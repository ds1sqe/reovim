//! Jump next/prev command handlers (#136).
//!
//! Navigate between tab stops in the active snippet.

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{
        ChangeTracker, ExtensionApi, ModeApi, SessionRuntime, TransitionContext,
    },
    reovim_kernel::api::v1::CommandId,
};

use crate::{ids, state::SnippetSessionState};

/// Jump to the next tab stop in the active snippet.
pub struct JumpNext;

impl Command for JumpNext {
    fn id(&self) -> CommandId {
        ids::JUMP_NEXT
    }

    fn description(&self) -> &'static str {
        "Jump to next snippet tab stop"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for JumpNext {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("no active buffer");
        };

        let state = runtime.ext_mut::<SnippetSessionState>();
        let Some(active) = &mut state.active else {
            return CommandResult::Success; // no active snippet
        };

        if let Some(ts) = active.next() {
            let start = ts.start;
            // Move cursor to the tab stop position
            if let Some(w) = runtime.windows_mut().active_mut() {
                w.cursor.line = start.line;
                w.cursor.column = start.column;
            }
            runtime.record_cursor_move(buffer_id);
        } else {
            // All tab stops visited: exit snippet mode
            let state = runtime.ext_mut::<SnippetSessionState>();
            state.active = None;
            runtime.set_mode(ids::VIM_INSERT_MODE, TransitionContext::new());
        }

        CommandResult::Success
    }
}

/// Jump to the previous tab stop in the active snippet.
pub struct JumpPrev;

impl Command for JumpPrev {
    fn id(&self) -> CommandId {
        ids::JUMP_PREV
    }

    fn description(&self) -> &'static str {
        "Jump to previous snippet tab stop"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for JumpPrev {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("no active buffer");
        };

        let state = runtime.ext_mut::<SnippetSessionState>();
        let Some(active) = &mut state.active else {
            return CommandResult::Success; // no active snippet
        };

        if let Some(ts) = active.prev() {
            let start = ts.start;
            if let Some(w) = runtime.windows_mut().active_mut() {
                w.cursor.line = start.line;
                w.cursor.column = start.column;
            }
            runtime.record_cursor_move(buffer_id);
        }
        // If None (at first): no-op

        CommandResult::Success
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jump_next_id() {
        assert_eq!(JumpNext.id(), ids::JUMP_NEXT);
    }

    #[test]
    fn test_jump_next_description() {
        assert!(!JumpNext.description().is_empty());
    }

    #[test]
    fn test_jump_prev_id() {
        assert_eq!(JumpPrev.id(), ids::JUMP_PREV);
    }

    #[test]
    fn test_jump_prev_description() {
        assert!(!JumpPrev.description().is_empty());
    }
}
