//! `CancelSnippet` command handler (#136).
//!
//! Cancels the active snippet session and returns to insert mode.

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{ExtensionApi, ModeApi, SessionRuntime, TransitionContext},
    reovim_kernel::api::v1::{CommandId, ModeId},
};

use crate::{ids, state::SnippetSessionState};

/// Cancel the active snippet and return to the configured return mode.
pub struct CancelSnippet {
    return_mode: ModeId,
}

impl CancelSnippet {
    /// Create a new cancel command with the given return mode.
    #[must_use]
    pub const fn new(return_mode: ModeId) -> Self {
        Self { return_mode }
    }
}

impl Command for CancelSnippet {
    fn id(&self) -> CommandId {
        ids::CANCEL
    }

    fn description(&self) -> &'static str {
        "Cancel active snippet"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for CancelSnippet {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<SnippetSessionState>();
        state.active = None;
        // Clear any placeholder selection
        if let Some(w) = runtime.windows_mut().active_mut() {
            w.selection = None;
        }
        runtime.set_mode(self.return_mode.clone(), TransitionContext::new());
        CommandResult::Success
    }
}

#[cfg(test)]
#[path = "cancel_tests.rs"]
mod tests;
