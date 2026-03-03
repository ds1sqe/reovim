//! `CancelSnippet` command handler (#136).
//!
//! Cancels the active snippet session and returns to insert mode.

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{ExtensionApi, ModeApi, SessionRuntime, TransitionContext},
    reovim_kernel::api::v1::CommandId,
};

use crate::{ids, state::SnippetSessionState};

/// Cancel the active snippet and return to insert mode.
pub struct CancelSnippet;

impl Command for CancelSnippet {
    fn id(&self) -> CommandId {
        ids::CANCEL
    }

    fn description(&self) -> &'static str {
        "Cancel active snippet"
    }
}

impl CommandHandler for CancelSnippet {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<SnippetSessionState>();
        state.active = None;
        runtime.set_mode(ids::VIM_INSERT_MODE, TransitionContext::new());
        CommandResult::Success
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cancel_id() {
        assert_eq!(CancelSnippet.id(), ids::CANCEL);
    }

    #[test]
    fn test_cancel_description() {
        assert!(!CancelSnippet.description().is_empty());
    }
}
