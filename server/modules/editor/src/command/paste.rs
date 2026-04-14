//! Paste commands.
//!
//! Provides paste commands:
//! - `PasteAfter` (p)
//! - `PasteBefore` (P)

use {
    reovim_domain_text::Position,
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_text_session::{SessionRuntime, api::BufferApi},
    reovim_kernel::api::v1::CommandId,
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
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let count = args.count().unwrap_or(1);
        let register = args.register();

        // Get register content with clipboard fallback for +/* (#515)
        let content = runtime.get_register_with_clipboard(register);

        let Some(content) = content else {
            return CommandResult::Success; // Empty register
        };

        if content.is_empty() {
            return CommandResult::Success; // Nothing to paste
        }

        // Get cursor from per-client Window (#471)
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let pos = Position::new(window.cursor.line, window.cursor.column);

        if content.is_linewise() {
            // Paste below current line
            // Get line count via BufferApi
            let line_count = runtime.buffer_line_count(buffer_id).unwrap_or(0);

            // Build paste text (repeated count times, strip trailing newline for clean insert)
            let paste_text = content.text.repeat(count);
            let paste_text = paste_text.trim_end_matches('\n');

            if line_count == 0 {
                // Empty buffer: just insert the content at origin
                runtime.insert_text(buffer_id, Position::new(0, 0), paste_text);
                // Update cursor via per-client Window (#471)
                if let Some(window) = runtime.windows_mut().active_mut() {
                    window.cursor = Position::new(0, 0).into();
                }
                return CommandResult::Success;
            }

            // Get line length via BufferApi
            let line_len = runtime.buffer_line_len(buffer_id, pos.line).unwrap_or(0);

            // Insert newline then content at end of current line
            let insert_pos = Position::new(pos.line, line_len);
            let insert_text = format!("\n{paste_text}");
            runtime.insert_text(buffer_id, insert_pos, &insert_text);

            // Position cursor on first character of first pasted line
            let new_line = pos.line + 1;
            // Update cursor via per-client Window (#471)
            if let Some(window) = runtime.windows_mut().active_mut() {
                window.cursor = Position::new(new_line, 0).into();
            }
        } else {
            // Characterwise: paste after cursor
            // Get line length via BufferApi
            let line_len = runtime.buffer_line_len(buffer_id, pos.line).unwrap_or(0);
            let insert_col = if line_len == 0 {
                0
            } else {
                (pos.column + 1).min(line_len)
            };

            let insert_pos = Position::new(pos.line, insert_col);
            let paste_text = content.text.repeat(count);
            runtime.insert_text(buffer_id, insert_pos, &paste_text);

            // Calculate final cursor position (at end of pasted text - 1 for Vim behavior)
            // Count lines in pasted text
            let lines_in_paste: Vec<&str> = paste_text.lines().collect();
            let cursor_after = if lines_in_paste.len() > 1 {
                // Multi-line paste: cursor at end of last line
                let last_line_len = lines_in_paste.last().map_or(0, |l| l.chars().count());
                let final_line = insert_pos.line + lines_in_paste.len() - 1;
                let col = if last_line_len > 0 {
                    last_line_len - 1
                } else {
                    0
                };
                Position::new(final_line, col)
            } else {
                // Single-line paste: cursor at end of pasted text - 1
                let text_len = paste_text.chars().count();
                let final_col = insert_col + text_len;
                let col = if final_col > 0 { final_col - 1 } else { 0 };
                Position::new(pos.line, col)
            };
            // Update cursor via per-client Window (#471)
            if let Some(window) = runtime.windows_mut().active_mut() {
                window.cursor = cursor_after.into();
            }
        }

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
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let count = args.count().unwrap_or(1);
        let register = args.register();

        // Get register content with clipboard fallback for +/* (#515)
        let content = runtime.get_register_with_clipboard(register);

        let Some(content) = content else {
            return CommandResult::Success; // Empty register
        };

        if content.is_empty() {
            return CommandResult::Success; // Nothing to paste
        }

        // Get cursor from per-client Window (#471)
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let pos = Position::new(window.cursor.line, window.cursor.column);

        // Build paste text (repeated count times)
        let paste_text = content.text.repeat(count);

        if content.is_linewise() {
            // Paste above current line
            // Strip trailing newline for clean insert
            let paste_text = paste_text.trim_end_matches('\n');

            // Insert content then newline at start of current line
            let insert_pos = Position::new(pos.line, 0);
            let insert_text = format!("{paste_text}\n");
            runtime.insert_text(buffer_id, insert_pos, &insert_text);

            // Position cursor on first character of first pasted line
            // Update cursor via per-client Window (#471)
            if let Some(window) = runtime.windows_mut().active_mut() {
                window.cursor = Position::new(pos.line, 0).into();
            }
        } else {
            // Characterwise: paste at cursor position (before)
            runtime.insert_text(buffer_id, pos, &paste_text);

            // Calculate final cursor position (at end of pasted text - 1 for Vim behavior)
            let lines_in_paste: Vec<&str> = paste_text.lines().collect();
            let cursor_after = if lines_in_paste.len() > 1 {
                // Multi-line paste: cursor at end of last line
                let last_line_len = lines_in_paste.last().map_or(0, |l| l.chars().count());
                let final_line = pos.line + lines_in_paste.len() - 1;
                let col = if last_line_len > 0 {
                    last_line_len - 1
                } else {
                    0
                };
                Position::new(final_line, col)
            } else {
                // Single-line paste: cursor at end of pasted text - 1
                let text_len = paste_text.chars().count();
                let final_col = pos.column + text_len;
                let col = if final_col > 0 { final_col - 1 } else { 0 };
                Position::new(pos.line, col)
            };
            // Update cursor via per-client Window (#471)
            if let Some(window) = runtime.windows_mut().active_mut() {
                window.cursor = cursor_after.into();
            }
        }

        CommandResult::Success
    }
}
