//! Paste commands.
//!
//! Provides paste commands:
//! - `PasteAfter` (p)
//! - `PasteBefore` (P)

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{BufferApi, RegisterApi, SessionRuntime},
    reovim_kernel::api::v1::{CommandId, Position},
};

use crate::ids;

/// Paste from register after cursor (p).
///
/// - Linewise paste: insert below current line
/// - Characterwise paste: insert after cursor position
#[derive(Debug, Clone, Copy, Default)]
pub struct PasteAfter;

impl Command for PasteAfter {
    fn id(&self) -> CommandId {
        ids::PASTE_AFTER
    }

    fn description(&self) -> &'static str {
        "Paste after cursor"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![
            ArgSpec::optional("count", ArgKind::Count, "Number of times to paste"),
            ArgSpec::optional("register", ArgKind::Register, "Source register"),
        ]
    }
}

impl CommandHandler for PasteAfter {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = runtime.kernel().buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let count = args.count().unwrap_or(1);
        let register = args.register();

        // Get register content via RegisterApi
        let content = runtime.get_register(register);

        let Some(content) = content else {
            return CommandResult::Success; // Empty register
        };

        if content.is_empty() {
            return CommandResult::Success; // Nothing to paste
        }

        // Get position via BufferApi
        let Some(pos) = runtime.buffer_position(buffer_id) else {
            return CommandResult::error("Failed to get buffer position");
        };
        let cursor_before = pos;

        if content.is_linewise() {
            // Paste below current line
            // Get line count via BufferApi
            let line_count = runtime.buffer_line_count(buffer_id).unwrap_or(0);

            // Build paste text (repeated count times, strip trailing newline for clean insert)
            let paste_text = content.text.repeat(count);
            let paste_text = paste_text.trim_end_matches('\n');

            if line_count == 0 {
                // Empty buffer: just insert the content
                let mut buffer = buffer_arc.write();
                let _edit = buffer.insert(paste_text);
                drop(buffer);
                runtime.set_buffer_position(buffer_id, Position::new(0, 0));
                // TODO(#394): Return edit action via different mechanism (escape hatch until API supports this)
                let _ = cursor_before; // Suppress unused warning
                return CommandResult::Success;
            }

            // Get line length via BufferApi
            let line_len = runtime.buffer_line_len(buffer_id, pos.line).unwrap_or(0);

            // Move to end of current line
            runtime.set_buffer_position(buffer_id, Position::new(pos.line, line_len));

            // Insert newline then content
            let insert_text = format!("\n{paste_text}");
            let mut buffer = buffer_arc.write();
            let _edit = buffer.insert(&insert_text);
            drop(buffer);

            // Position cursor on first character of first pasted line
            let new_line = pos.line + 1;
            runtime.set_buffer_position(buffer_id, Position::new(new_line, 0));
        } else {
            // Characterwise: paste after cursor
            // Get line length via BufferApi
            let line_len = runtime.buffer_line_len(buffer_id, pos.line).unwrap_or(0);
            let insert_col = if line_len == 0 {
                0
            } else {
                (pos.column + 1).min(line_len)
            };

            runtime.set_buffer_position(buffer_id, Position::new(pos.line, insert_col));

            let paste_text = content.text.repeat(count);
            let mut buffer = buffer_arc.write();
            let _edit = buffer.insert(&paste_text);
            let final_pos = buffer.position();
            drop(buffer);

            // Position cursor at end of pasted text (Vim behavior: last character)
            let cursor_after = if final_pos.column > 0 {
                Position::new(final_pos.line, final_pos.column - 1)
            } else {
                final_pos
            };
            runtime.set_buffer_position(buffer_id, cursor_after);
        }

        // TODO(#394): Return edit action via different mechanism (escape hatch until API supports this)
        let _ = cursor_before; // Suppress unused warning
        CommandResult::Success
    }
}

/// Paste from register before cursor (P).
///
/// - Linewise paste: insert above current line
/// - Characterwise paste: insert at cursor position
#[derive(Debug, Clone, Copy, Default)]
pub struct PasteBefore;

impl Command for PasteBefore {
    fn id(&self) -> CommandId {
        ids::PASTE_BEFORE
    }

    fn description(&self) -> &'static str {
        "Paste before cursor"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![
            ArgSpec::optional("count", ArgKind::Count, "Number of times to paste"),
            ArgSpec::optional("register", ArgKind::Register, "Source register"),
        ]
    }
}

impl CommandHandler for PasteBefore {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = runtime.kernel().buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let count = args.count().unwrap_or(1);
        let register = args.register();

        // Get register content via RegisterApi
        let content = runtime.get_register(register);

        let Some(content) = content else {
            return CommandResult::Success; // Empty register
        };

        if content.is_empty() {
            return CommandResult::Success; // Nothing to paste
        }

        // Get position via BufferApi
        let Some(pos) = runtime.buffer_position(buffer_id) else {
            return CommandResult::error("Failed to get buffer position");
        };
        let cursor_before = pos;

        // Build paste text (repeated count times)
        let paste_text = content.text.repeat(count);

        if content.is_linewise() {
            // Paste above current line
            // Strip trailing newline for clean insert
            let paste_text = paste_text.trim_end_matches('\n');

            // Move to start of current line
            runtime.set_buffer_position(buffer_id, Position::new(pos.line, 0));

            // Insert content then newline
            let insert_text = format!("{paste_text}\n");
            let mut buffer = buffer_arc.write();
            let _edit = buffer.insert(&insert_text);
            drop(buffer);

            // Position cursor on first character of first pasted line
            runtime.set_buffer_position(buffer_id, Position::new(pos.line, 0));
        } else {
            // Characterwise: paste at cursor position (before)
            // Cursor stays at current position, content inserted there
            let mut buffer = buffer_arc.write();
            let _edit = buffer.insert(&paste_text);
            let final_pos = buffer.position();
            drop(buffer);

            // Position cursor at end of pasted text (Vim behavior: last character)
            let cursor_after = if final_pos.column > 0 {
                Position::new(final_pos.line, final_pos.column - 1)
            } else {
                final_pos
            };
            runtime.set_buffer_position(buffer_id, cursor_after);
        }

        // TODO(#394): Return edit action via different mechanism (escape hatch until API supports this)
        let _ = cursor_before; // Suppress unused warning
        CommandResult::Success
    }
}
