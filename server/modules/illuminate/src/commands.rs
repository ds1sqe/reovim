//! Navigation commands for moving between highlighted references.

use {
    reovim_driver_command::{Command, CommandHandler},
    reovim_driver_command_types::{CommandContext, CommandResult},
    reovim_driver_session::{ChangeTracker, ExtensionApi, SessionRuntime, WindowApi},
};

use crate::{ids, state::IlluminateState};

/// Navigate to the next highlighted reference.
///
/// Wraps around from the last reference to the first.
pub struct NextReferenceCommand;

impl Command for NextReferenceCommand {
    fn id(&self) -> reovim_kernel::api::v1::CommandId {
        ids::NEXT_REFERENCE
    }

    fn description(&self) -> &'static str {
        "Next reference"
    }
}

impl CommandHandler for NextReferenceCommand {
    #[allow(clippy::cast_possible_truncation)]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let Some(state) = runtime.ext::<IlluminateState>() else {
            return CommandResult::Success;
        };
        if !state.active {
            return CommandResult::Success;
        }

        let Some(cursor) = runtime.cursor_position() else {
            return CommandResult::Success;
        };

        // next_range_index returns None when ranges is empty
        let Some(idx) = state.next_range_index(cursor.line as u32, cursor.column as u32) else {
            return CommandResult::Success;
        };

        let target_line = state.ranges[idx].start_line as usize;
        let target_col = state.ranges[idx].start_col as usize;
        let buffer_id = state.buffer_id;

        if let Some(window) = runtime.windows_mut().active_mut() {
            window.cursor.line = target_line;
            window.cursor.column = target_col;
        }
        runtime.record_cursor_move(buffer_id);

        CommandResult::Success
    }
}

/// Navigate to the previous highlighted reference.
///
/// Wraps around from the first reference to the last.
pub struct PrevReferenceCommand;

impl Command for PrevReferenceCommand {
    fn id(&self) -> reovim_kernel::api::v1::CommandId {
        ids::PREV_REFERENCE
    }

    fn description(&self) -> &'static str {
        "Previous reference"
    }
}

impl CommandHandler for PrevReferenceCommand {
    #[allow(clippy::cast_possible_truncation)]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let Some(state) = runtime.ext::<IlluminateState>() else {
            return CommandResult::Success;
        };
        if !state.active {
            return CommandResult::Success;
        }

        let Some(cursor) = runtime.cursor_position() else {
            return CommandResult::Success;
        };

        // prev_range_index returns None when ranges is empty
        let Some(idx) = state.prev_range_index(cursor.line as u32, cursor.column as u32) else {
            return CommandResult::Success;
        };

        let target_line = state.ranges[idx].start_line as usize;
        let target_col = state.ranges[idx].start_col as usize;
        let buffer_id = state.buffer_id;

        if let Some(window) = runtime.windows_mut().active_mut() {
            window.cursor.line = target_line;
            window.cursor.column = target_col;
        }
        runtime.record_cursor_move(buffer_id);

        CommandResult::Success
    }
}

/// All illuminate command handlers.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(NextReferenceCommand),
        Box::new(PrevReferenceCommand),
    ]
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
