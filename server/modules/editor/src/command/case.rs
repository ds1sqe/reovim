//! Case transformation commands.
//!
//! Provides `ToggleCase` (~) which toggles the case of characters under the
//! cursor and advances the cursor position.

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{BufferApi, ChangeTracker, SessionRuntime},
    reovim_kernel::api::v1::{CommandId, Position},
};

use crate::ids;

/// Toggle case of character under cursor (~).
///
/// Toggles uppercase to lowercase and vice versa for the character at the
/// cursor position, then advances the cursor by one column. With a count,
/// toggles that many characters. Stops at the end of the line.
#[derive(Debug, Clone, Copy, Default)]
pub struct ToggleCase;

impl Command for ToggleCase {
    fn id(&self) -> CommandId {
        ids::TOGGLE_CASE
    }

    fn description(&self) -> &'static str {
        "Toggle case of character under cursor"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of characters to toggle",
        )]
    }
}

impl CommandHandler for ToggleCase {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let pos = Position::new(window.cursor.line, window.cursor.column);

        let Some(line) = runtime.buffer_line(buffer_id, pos.line) else {
            return CommandResult::Success;
        };

        let line_len = line.len();
        if line_len == 0 || pos.column >= line_len {
            return CommandResult::Success;
        }

        let count = args.count().unwrap_or(1).max(1);
        let chars_to_toggle = count.min(line_len - pos.column);

        // Extract the substring to toggle
        let slice = &line[pos.column..pos.column + chars_to_toggle];
        let toggled: String = slice
            .chars()
            .map(|c| {
                if c.is_uppercase() {
                    c.to_lowercase().next().unwrap_or(c)
                } else if c.is_lowercase() {
                    c.to_uppercase().next().unwrap_or(c)
                } else {
                    c
                }
            })
            .collect();

        // Only modify if something actually changed
        if toggled != slice {
            let end = Position::new(pos.line, pos.column + chars_to_toggle);
            runtime.delete_range(buffer_id, pos, end);
            runtime.insert_text(buffer_id, pos, &toggled);
        }

        // Advance cursor to end of toggled region (or end of line)
        let new_col = (pos.column + chars_to_toggle).min(line_len.saturating_sub(1));
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.cursor.column = new_col;
        }

        runtime.record_cursor_move(buffer_id);

        CommandResult::Success
    }
}
