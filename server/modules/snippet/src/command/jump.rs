//! Jump next/prev command handlers (#136).
//!
//! Navigate between tab stops in the active snippet.

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{
        BufferApi, ChangeTracker, ExtensionApi, ModeApi, Selection, SessionRuntime,
        TransitionContext,
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
        execute_jump(runtime, args, Direction::Next)
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
        execute_jump(runtime, args, Direction::Prev)
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

/// Shared jump implementation for both `JumpNext` and `JumpPrev`.
fn execute_jump(
    runtime: &mut SessionRuntime<'_>,
    args: &CommandContext,
    direction: Direction,
) -> CommandResult {
    let Some(buffer_id) = args.buffer_id() else {
        return CommandResult::error("no active buffer");
    };

    let cursor = runtime
        .windows()
        .active()
        .map_or(Position::origin(), |w| Position::new(w.cursor.line, w.cursor.column));

    // Pre-read departing text from buffer (before mutable snippet borrow).
    let departing_text = read_departing_text(runtime, buffer_id, cursor);

    // Take the active snippet out of state to avoid borrow conflicts.
    let mut active = {
        let state = runtime.ext_mut::<SnippetSessionState>();
        match state.active.take() {
            Some(a) => a,
            None => return CommandResult::Success,
        }
    };

    // Reconcile any user typing at the current tab stop.
    active.reconcile_typing(cursor);

    // Collect mirror data before navigating.
    let departing_index = active.current_index();
    let departing_id = active.current().map(|ts| ts.id);
    let mirrors: Vec<MirrorInfo> = departing_id
        .map(|id| {
            active
                .mirror_indices(id, departing_index)
                .into_iter()
                .map(|i| {
                    let ts = &active.tab_stops()[i];
                    MirrorInfo {
                        index: i,
                        old_start: ts.start,
                        old_end: ts.end,
                        old_placeholder: ts.placeholder.clone(),
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    // Navigate to next/prev tab stop.
    let ts = match direction {
        Direction::Next => active.next(),
        Direction::Prev => active.prev(),
    };

    match ts {
        Some(ts) => {
            let mut dest_start = ts.start;
            let mut dest_end = ts.end;

            // Apply mirrors for the departing tab stop.
            for mirror in &mirrors {
                apply_single_mirror(
                    runtime,
                    buffer_id,
                    mirror,
                    &departing_text,
                    &mut active,
                    &mut dest_start,
                    &mut dest_end,
                );
            }

            // Put snippet back before setting selection.
            let state = runtime.ext_mut::<SnippetSessionState>();
            state.active = Some(active);

            // Select placeholder at destination tab stop.
            select_placeholder_and_move(runtime, buffer_id, dest_start, dest_end);
        }
        None => {
            match direction {
                Direction::Next => {
                    // All tab stops exhausted — exit snippet mode.
                    // Don't put snippet back; it's consumed.
                    if let Some(w) = runtime.windows_mut().active_mut() {
                        w.selection = None;
                    }
                    runtime.set_mode(ids::VIM_INSERT_MODE, TransitionContext::new());
                }
                Direction::Prev => {
                    // Already at first stop — put snippet back, do nothing.
                    let state = runtime.ext_mut::<SnippetSessionState>();
                    state.active = Some(active);
                }
            }
        }
    }

    CommandResult::Success
}

/// Pre-read the text at the current tab stop (from start to cursor).
fn read_departing_text(
    runtime: &SessionRuntime<'_>,
    buffer_id: reovim_kernel::api::v1::BufferId,
    cursor: Position,
) -> String {
    let state = runtime.ext::<SnippetSessionState>();
    state
        .and_then(|s| s.active.as_ref())
        .and_then(|a| a.current())
        .map(|ts| {
            runtime
                .buffer_text_range(buffer_id, ts.start, cursor)
                .unwrap_or_default()
        })
        .unwrap_or_default()
}

/// Information about a mirror tab stop that needs updating.
struct MirrorInfo {
    index: usize,
    old_start: Position,
    old_end: Position,
    old_placeholder: String,
}

/// Apply a single mirror edit: delete old text, insert new text, update positions.
fn apply_single_mirror(
    runtime: &mut SessionRuntime<'_>,
    buffer_id: reovim_kernel::api::v1::BufferId,
    mirror: &MirrorInfo,
    new_text: &str,
    active: &mut crate::engine::ActiveSnippet,
    dest_start: &mut Position,
    dest_end: &mut Position,
) {
    // Delete the old mirror text.
    if mirror.old_start != mirror.old_end {
        let edit = Edit::delete(mirror.old_start, &mirror.old_placeholder);
        runtime.delete_range(buffer_id, mirror.old_start, mirror.old_end);
        active.update_positions(&edit);
        *dest_start = reovim_kernel::api::v1::transform_position(*dest_start, &edit);
        *dest_end = reovim_kernel::api::v1::transform_position(*dest_end, &edit);
    }

    // Insert the new text at the mirror position.
    let insert_pos = active.tab_stops()[mirror.index].start;
    if !new_text.is_empty() {
        let edit = Edit::insert(insert_pos, new_text);
        runtime.insert_text(buffer_id, insert_pos, new_text);
        active.update_positions(&edit);
        *dest_start = reovim_kernel::api::v1::transform_position(*dest_start, &edit);
        *dest_end = reovim_kernel::api::v1::transform_position(*dest_end, &edit);
    }

    // Compute the end position of the newly-inserted text.
    let mut mirror_end = insert_pos;
    for ch in new_text.chars() {
        if ch == '\n' {
            mirror_end = Position::new(mirror_end.line + 1, 0);
        } else {
            mirror_end = Position::new(mirror_end.line, mirror_end.column + 1);
        }
    }
    active.update_tab_stop(mirror.index, insert_pos, mirror_end, new_text.to_string());
}

/// Select placeholder text (if any) and position cursor at the tab stop.
fn select_placeholder_and_move(
    runtime: &mut SessionRuntime<'_>,
    buffer_id: reovim_kernel::api::v1::BufferId,
    start: Position,
    end: Position,
) {
    if let Some(w) = runtime.windows_mut().active_mut() {
        w.cursor.line = start.line;
        w.cursor.column = start.column;
        if start == end {
            w.selection = None;
        } else {
            w.selection = Some(Selection::character(start, end));
        }
    }
    runtime.record_cursor_move(buffer_id);
    if start != end {
        runtime.record_selection_change(buffer_id);
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
