//! Visual mode exit commands.
//!
//! Provides commands to exit visual mode and return to normal mode.

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{
            BufferApi, SessionRuntime, TransitionContext,
        api::{ChangeTracker, ModeApi},
    },
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
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Clear selection on the active window
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.selection = None;
        }

        // #474: Notify other clients that selection was cleared
        if let Some(buffer_id) = runtime.active_buffer() {
            runtime.record_selection_change(buffer_id);
        }

        // Change to normal mode
        runtime.set_mode(VimMode::NORMAL_ID, TransitionContext::new());

        CommandResult::Success
    }
}
