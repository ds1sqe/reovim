//! Jump next/prev command handlers (#136).
//!
//! Navigate between tab stops in the active snippet.

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{
        BufferApi, ChangeTracker, ExtensionApi, ModeApi, SessionRuntime, TransitionContext,
    },
    reovim_kernel::api::v1::{CommandId, Edit, Position},
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

        // Read cursor position before taking the ext_mut borrow.
        let cursor = runtime
            .windows()
            .active()
            .map_or(Position::origin(), |w| Position::new(w.cursor.line, w.cursor.column));

        // Extract tab stop info while holding ext_mut borrow.
        // Reconciles any user typing, then navigates and pre-adjusts
        // positions for the upcoming placeholder deletion.
        let jump_info = {
            let state = runtime.ext_mut::<SnippetSessionState>();
            let Some(active) = &mut state.active else {
                return CommandResult::Success;
            };
            extract_jump_info(active, Direction::Next, cursor)
        };

        match jump_info {
            JumpAction::GoTo { start, end } => {
                delete_placeholder_and_move(runtime, buffer_id, start, end);
            }
            JumpAction::Done => {
                let state = runtime.ext_mut::<SnippetSessionState>();
                state.active = None;
                runtime.set_mode(ids::VIM_INSERT_MODE, TransitionContext::new());
            }
            JumpAction::NoOp => {}
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

        let cursor = runtime
            .windows()
            .active()
            .map_or(Position::origin(), |w| Position::new(w.cursor.line, w.cursor.column));

        let jump_info = {
            let state = runtime.ext_mut::<SnippetSessionState>();
            let Some(active) = &mut state.active else {
                return CommandResult::Success;
            };
            extract_jump_info(active, Direction::Prev, cursor)
        };

        if let JumpAction::GoTo { start, end } = jump_info {
            delete_placeholder_and_move(runtime, buffer_id, start, end);
        }
        // NoOp/Done: no-op for prev

        CommandResult::Success
    }
}

// ============================================================================
// Shared helpers
// ============================================================================

#[derive(Clone, Copy)]
enum Direction {
    Next,
    Prev,
}

enum JumpAction {
    /// Jump to a tab stop. If `start != end`, the placeholder must be deleted.
    GoTo { start: Position, end: Position },
    /// All tab stops exhausted (next only).
    Done,
    /// No movement (prev at first stop).
    NoOp,
}

/// Navigate to the next/prev tab stop, extract its range, and pre-adjust
/// all remaining positions if a placeholder deletion is needed.
///
/// `cursor` is the current cursor position — used to reconcile any text
/// the user typed at the departing tab stop before `update_positions`
/// was aware of it.
fn extract_jump_info(
    active: &mut crate::engine::ActiveSnippet,
    direction: Direction,
    cursor: Position,
) -> JumpAction {
    // Reconcile any text typed at the current tab stop so that
    // subsequent tab stop positions reflect the actual buffer state.
    active.reconcile_typing(cursor);

    let ts = match direction {
        Direction::Next => active.next(),
        Direction::Prev => active.prev(),
    };

    let Some(ts) = ts else {
        return match direction {
            Direction::Next => JumpAction::Done,
            Direction::Prev => JumpAction::NoOp,
        };
    };

    let start = ts.start;
    let end = ts.end;
    let placeholder = ts.placeholder.clone();
    // NLL: ts borrow is dropped here

    // If placeholder text exists, update all tab stop positions to account
    // for the deletion that will happen after we release the ext_mut borrow.
    if start != end {
        let edit = Edit::delete(start, &placeholder);
        active.update_positions(&edit);
    }

    JumpAction::GoTo { start, end }
}

/// Delete placeholder text (if any) and position cursor at the tab stop.
fn delete_placeholder_and_move(
    runtime: &mut SessionRuntime<'_>,
    buffer_id: reovim_kernel::api::v1::BufferId,
    start: Position,
    end: Position,
) {
    if start != end {
        runtime.delete_range(buffer_id, start, end);
    }
    if let Some(w) = runtime.windows_mut().active_mut() {
        w.cursor.line = start.line;
        w.cursor.column = start.column;
    }
    runtime.record_cursor_move(buffer_id);
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
