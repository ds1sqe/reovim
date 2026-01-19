//! Paste commands.
//!
//! Provides paste commands:
//! - `PasteAfter` (p)
//! - `PasteBefore` (P)

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_kernel::api::v1::{CommandId, KernelContext, Position},
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
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let count = args.count().unwrap_or(1);
        let register = args.register();

        // Get register content (use specified or unnamed)
        let content = {
            let registers = ctx.registers.read();
            registers.get_by_name(register).cloned()
        };

        let Some(content) = content else {
            return CommandResult::Success; // Empty register
        };

        if content.is_empty() {
            return CommandResult::Success; // Nothing to paste
        }

        let mut buffer = buffer_arc.write();
        let pos = buffer.position();
        let cursor_before = pos;

        if content.is_linewise() {
            // Paste below current line
            let line_count = buffer.line_count();

            // Build paste text (repeated count times, strip trailing newline for clean insert)
            let paste_text = content.text.repeat(count);
            let paste_text = paste_text.trim_end_matches('\n');

            if line_count == 0 {
                // Empty buffer: just insert the content
                let _edit = buffer.insert(paste_text);
                buffer.set_position(Position::new(0, 0));
                let _cursor_after = buffer.position();
                drop(buffer);
                // TODO: Return edit action via different mechanism when SessionContext is available
                let _ = cursor_before; // Suppress unused warning
                return CommandResult::Success;
            }

            // Move to end of current line
            let line_len = buffer.line_len(pos.line).unwrap_or(0);
            buffer.set_position(Position::new(pos.line, line_len));

            // Insert newline then content
            let insert_text = format!("\n{paste_text}");
            let _edit = buffer.insert(&insert_text);

            // Position cursor on first character of first pasted line
            let new_line = pos.line + 1;
            buffer.set_position(Position::new(new_line, 0));
        } else {
            // Characterwise: paste after cursor
            let line_len = buffer.line_len(pos.line).unwrap_or(0);
            let insert_col = if line_len == 0 {
                0
            } else {
                (pos.column + 1).min(line_len)
            };

            buffer.set_position(Position::new(pos.line, insert_col));

            let paste_text = content.text.repeat(count);
            let _edit = buffer.insert(&paste_text);

            // Position cursor at end of pasted text (Vim behavior: last character)
            let final_pos = buffer.position();
            let cursor_after = if final_pos.column > 0 {
                Position::new(final_pos.line, final_pos.column - 1)
            } else {
                final_pos
            };
            buffer.set_position(cursor_after);
        }
        drop(buffer);

        // TODO: Return edit action via different mechanism when SessionContext is available
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
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let count = args.count().unwrap_or(1);
        let register = args.register();

        // Get register content (use specified or unnamed)
        let content = {
            let registers = ctx.registers.read();
            registers.get_by_name(register).cloned()
        };

        let Some(content) = content else {
            return CommandResult::Success; // Empty register
        };

        if content.is_empty() {
            return CommandResult::Success; // Nothing to paste
        }

        let mut buffer = buffer_arc.write();
        let pos = buffer.position();
        let cursor_before = pos;

        // Build paste text (repeated count times)
        let paste_text = content.text.repeat(count);

        if content.is_linewise() {
            // Paste above current line
            // Strip trailing newline for clean insert
            let paste_text = paste_text.trim_end_matches('\n');

            // Move to start of current line
            buffer.set_position(Position::new(pos.line, 0));

            // Insert content then newline
            let insert_text = format!("{paste_text}\n");
            let _edit = buffer.insert(&insert_text);

            // Position cursor on first character of first pasted line
            buffer.set_position(Position::new(pos.line, 0));
        } else {
            // Characterwise: paste at cursor position (before)
            // Cursor stays at current position, content inserted there
            let _edit = buffer.insert(&paste_text);

            // Position cursor at end of pasted text (Vim behavior: last character)
            let final_pos = buffer.position();
            let cursor_after = if final_pos.column > 0 {
                Position::new(final_pos.line, final_pos.column - 1)
            } else {
                final_pos
            };
            buffer.set_position(cursor_after);
        }
        drop(buffer);

        // TODO: Return edit action via different mechanism when SessionContext is available
        let _ = cursor_before; // Suppress unused warning
        CommandResult::Success
    }
}
