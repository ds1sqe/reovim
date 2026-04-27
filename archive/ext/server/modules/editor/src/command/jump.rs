//! Jump list commands.
//!
//! Provides jump list navigation:
//! - `JumpBackward` (Ctrl-O) - jump to older position
//! - `JumpForward` (Ctrl-I) - jump to newer position

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_text_session::{SessionRuntime, api::BufferApi},
    reovim_kernel::api::v1::CommandId,
};

use crate::ids;

/// Jump to older position in jump list (Ctrl-O).
///
/// Navigates backward through the jump list. If the target entry is in
/// a different buffer, switches the active buffer automatically.
#[derive(Debug, Clone, Copy, Default)]
pub struct JumpBackward;

impl Command for JumpBackward {
    fn id(&self) -> CommandId {
        ids::JUMP_BACKWARD
    }

    fn description(&self) -> &'static str {
        "Jump to older position"
    }

    fn args(&self) -> Vec<reovim_driver_command::ArgSpec> {
        vec![]
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for JumpBackward {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let Some(entry) = runtime.jumplist_mut().backward().cloned() else {
            return CommandResult::Success;
        };

        // Switch buffer if needed
        if let Some(current_buf) = runtime.active_buffer()
            && entry.buffer != current_buf
        {
            runtime.set_active_buffer(Some(entry.buffer));
            if let Some(window) = runtime.windows_mut().active_mut() {
                window.buffer_id = Some(entry.buffer);
            }
        }

        // Move cursor
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.cursor.line = entry.position.line;
            window.cursor.column = entry.position.column;
        }

        CommandResult::Success
    }
}

/// Jump to newer position in jump list (Ctrl-I).
///
/// Navigates forward through the jump list. If the target entry is in
/// a different buffer, switches the active buffer automatically.
#[derive(Debug, Clone, Copy, Default)]
pub struct JumpForward;

impl Command for JumpForward {
    fn id(&self) -> CommandId {
        ids::JUMP_FORWARD
    }

    fn description(&self) -> &'static str {
        "Jump to newer position"
    }

    fn args(&self) -> Vec<reovim_driver_command::ArgSpec> {
        vec![]
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for JumpForward {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let Some(entry) = runtime.jumplist_mut().forward().cloned() else {
            return CommandResult::Success;
        };

        // Switch buffer if needed
        if let Some(current_buf) = runtime.active_buffer()
            && entry.buffer != current_buf
        {
            runtime.set_active_buffer(Some(entry.buffer));
            if let Some(window) = runtime.windows_mut().active_mut() {
                window.buffer_id = Some(entry.buffer);
            }
        }

        // Move cursor
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.cursor.line = entry.position.line;
            window.cursor.column = entry.position.column;
        }

        CommandResult::Success
    }
}
