//! Visual mode exit commands.
//!
//! Provides commands to exit visual mode and return to normal mode.

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{SessionRuntime, TransitionContext, api::ModeApi},
    reovim_kernel::api::v1::CommandId,
};

use crate::{ids, modes::VimMode};

/// Exit visual mode and return to normal mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExitVisualMode;

impl Command for ExitVisualMode {
    fn id(&self) -> CommandId {
        ids::EXIT_VISUAL
    }

    fn description(&self) -> &'static str {
        "Exit visual mode and return to normal mode"
    }
}

impl CommandHandler for ExitVisualMode {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let kernel = runtime.kernel();

        // Clear selection if we have a buffer
        if let Some(buffer_id) = args.buffer_id()
            && let Some(buffer_arc) = kernel.buffers.get(buffer_id)
        {
            let mut buffer = buffer_arc.write();
            buffer.selection_mut().clear();
        }

        // Change to normal mode
        runtime.set_mode(VimMode::NORMAL_ID, TransitionContext::new());

        CommandResult::Success
    }
}
