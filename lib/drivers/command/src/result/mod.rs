//! Command execution results and action intents.
//!
//! # Policy-Adjacent Types
//!
//! Some types in this module (`SearchDirection`, `SearchAction`) are closer to
//! policy than pure mechanism. They remain here because:
//! 1. No dedicated search module exists yet
//! 2. They are callback intents (commands declare WHAT, runner decides HOW)
//! 3. Moving them would create circular dependencies
//!
//! Future refactoring may extract these to dedicated modules.

mod block_insert;
mod edit;
mod mode;
mod search;
mod undo;
mod window;

pub use {
    block_insert::BlockInsertAction,
    edit::EditAction,
    mode::ModeAction,
    search::{SearchAction, SearchDirection},
    undo::{UndoAction, UndotreeAction},
    window::WindowAction,
};

use {
    crate::char_wait::{CharWaitContext, FindType},
    reovim_kernel::api::v1::{BufferId, Edit, Position},
};

// ============================================================================
// CommandResult
// ============================================================================

/// Result of command execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandResult {
    /// Command executed successfully.
    Success,
    /// Command failed with an error message.
    Error(String),
    /// Command requests editor to quit.
    Quit,
    /// Command requests editor to quit without saving.
    ForceQuit,
    /// Command requests an undo/redo action to be performed by the runner.
    ///
    /// This follows the callback pattern where commands declare WHAT they want
    /// (intent), and the runner decides HOW to execute it (policy).
    UndoAction(UndoAction),
    /// Command requests an undotree visualization action.
    ///
    /// This follows the same callback pattern as `UndoAction`, where commands
    /// declare intent and the runner handles execution.
    UndotreeAction(UndotreeAction),
    /// Command requests a window management action.
    ///
    /// Window commands (split, focus, close) return this to indicate what
    /// window operation should be performed. The runner handles the actual
    /// window state management, layout updates, and focus changes.
    WindowAction(WindowAction),
    /// Command requests a mode change action.
    ///
    /// Mode commands (enter-window-mode, enter-visual, etc.) return this to
    /// indicate what mode operation should be performed. The runner handles
    /// the actual mode stack manipulation.
    ModeAction(ModeAction),
    /// Command reports edits it made to a buffer.
    ///
    /// The runner records these edits in the undo registry for later
    /// undo/redo operations.
    EditAction(EditAction),
    /// Command needs a character argument before it can complete.
    ///
    /// Find-char commands (f, F, t, T) and replace-char command (r) return
    /// this to indicate they need the next keypress as a character argument.
    /// The runner sets pending-char state and waits for the character input.
    WaitingForChar(CharWaitContext),
    /// Repeat the last find-char motion in the same direction (;).
    ///
    /// The runner executes `last_find.repeat_motion()` if `last_find` is set.
    RepeatFindSame,
    /// Repeat the last find-char motion in the opposite direction (,).
    ///
    /// The runner executes `last_find.reverse_motion()` if `last_find` is set.
    RepeatFindReverse,
    /// Command requests a search action.
    ///
    /// Search commands (/, ?, n, N, *, #, :noh) return this to indicate
    /// what search operation should be performed. The runner handles
    /// input mode, pattern storage, and search execution.
    SearchAction(SearchAction),
    /// Repeat the last repeatable command (.).
    ///
    /// The runner replays the last repeatable command from `repeat_state`.
    /// This includes text-modifying commands and any accumulated insert text.
    RepeatAction,
    /// Text object command returns a range for the pending operator.
    ///
    /// Text object commands (iw, aw, i", a", etc.) return this in operator-pending
    /// mode to provide the range for the operator (d, y, c) to act upon.
    /// The runner executes the pending operator with this range.
    OperatorRange {
        /// Start position of the range (inclusive).
        start: Position,
        /// End position of the range (exclusive).
        end: Position,
        /// Whether this is a linewise range (affects paste behavior).
        is_linewise: bool,
    },
    /// Command requests visual selection restoration (gv).
    ///
    /// The `reselect-last` command returns this to request restoration of
    /// the last visual selection. The runner handles the actual restoration
    /// by looking up the saved selection and entering the appropriate visual mode.
    ReselectVisual,
    /// Command requests a visual-block insert operation.
    ///
    /// Visual-block commands (I, A in visual-block mode) return this to
    /// indicate a block insert operation should be performed. The runner
    /// handles entering insert mode and replicating the inserted text
    /// across all lines of the block when insert mode exits.
    BlockInsertAction(BlockInsertAction),
}

impl CommandResult {
    /// Check if the result is success.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }

    /// Check if the result is an error.
    #[must_use]
    pub const fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }

    /// Check if the result requests quit.
    #[must_use]
    pub const fn is_quit(&self) -> bool {
        matches!(self, Self::Quit | Self::ForceQuit)
    }

    /// Check if the result is an undo/redo action.
    #[must_use]
    pub const fn is_undo_action(&self) -> bool {
        matches!(self, Self::UndoAction(_))
    }

    /// Check if the result is an undotree action.
    #[must_use]
    pub const fn is_undotree_action(&self) -> bool {
        matches!(self, Self::UndotreeAction(_))
    }

    /// Check if the result is a window action.
    #[must_use]
    pub const fn is_window_action(&self) -> bool {
        matches!(self, Self::WindowAction(_))
    }

    /// Check if the result is a mode action.
    #[must_use]
    pub const fn is_mode_action(&self) -> bool {
        matches!(self, Self::ModeAction(_))
    }

    /// Check if the result is an edit action.
    #[must_use]
    pub const fn is_edit_action(&self) -> bool {
        matches!(self, Self::EditAction(_))
    }

    /// Check if the result is waiting for a character argument.
    #[must_use]
    pub const fn is_waiting_for_char(&self) -> bool {
        matches!(self, Self::WaitingForChar(_))
    }

    /// Check if the result is a search action.
    #[must_use]
    pub const fn is_search_action(&self) -> bool {
        matches!(self, Self::SearchAction(_))
    }

    /// Create an error result.
    #[must_use]
    pub fn error(msg: impl Into<String>) -> Self {
        Self::Error(msg.into())
    }

    /// Create an edit action result.
    #[must_use]
    pub fn edit_action(
        buffer_id: BufferId,
        edit: Edit,
        cursor_before: Position,
        cursor_after: Position,
    ) -> Self {
        Self::EditAction(EditAction::single(buffer_id, edit, cursor_before, cursor_after))
    }

    /// Create an edit action result from multiple edits.
    #[must_use]
    pub fn edit_actions(
        buffer_id: BufferId,
        edits: Vec<Edit>,
        cursor_before: Position,
        cursor_after: Position,
    ) -> Self {
        Self::EditAction(EditAction::new(buffer_id, edits, cursor_before, cursor_after))
    }

    /// Create a waiting-for-char result for find-char operations.
    #[must_use]
    pub const fn waiting_for_char(find_type: FindType, start_position: Position) -> Self {
        Self::WaitingForChar(CharWaitContext::find_char(find_type, start_position))
    }

    /// Create a waiting-for-char result for replace-char operation.
    #[must_use]
    pub const fn waiting_for_replace_char(count: usize) -> Self {
        Self::WaitingForChar(CharWaitContext::replace_char(count))
    }

    /// Check if the result is a repeat action.
    #[must_use]
    pub const fn is_repeat_action(&self) -> bool {
        matches!(self, Self::RepeatAction)
    }

    /// Check if the result is an operator range.
    #[must_use]
    pub const fn is_operator_range(&self) -> bool {
        matches!(self, Self::OperatorRange { .. })
    }

    /// Check if the result is a reselect visual action.
    #[must_use]
    pub const fn is_reselect_visual(&self) -> bool {
        matches!(self, Self::ReselectVisual)
    }

    /// Check if the result is a block insert action.
    #[must_use]
    pub const fn is_block_insert_action(&self) -> bool {
        matches!(self, Self::BlockInsertAction(_))
    }

    /// Create an operator range result for text object commands.
    #[must_use]
    pub const fn operator_range(start: Position, end: Position, is_linewise: bool) -> Self {
        Self::OperatorRange {
            start,
            end,
            is_linewise,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_result_success() {
        let result = CommandResult::Success;
        assert!(result.is_success());
        assert!(!result.is_error());
        assert!(!result.is_quit());
    }

    #[test]
    fn test_command_result_error() {
        let result = CommandResult::error("Something went wrong");
        assert!(!result.is_success());
        assert!(result.is_error());
        assert!(!result.is_quit());
    }

    #[test]
    fn test_command_result_quit() {
        assert!(CommandResult::Quit.is_quit());
        assert!(CommandResult::ForceQuit.is_quit());
    }

    #[test]
    fn test_command_result_undo_action() {
        let undo = CommandResult::UndoAction(UndoAction::Undo { count: 1 });
        assert!(undo.is_undo_action());
        assert!(!undo.is_success());
        assert!(!undo.is_error());
        assert!(!undo.is_quit());

        let redo = CommandResult::UndoAction(UndoAction::Redo { count: 3 });
        assert!(redo.is_undo_action());
    }

    #[test]
    fn test_undo_action_variants() {
        let undo = UndoAction::Undo { count: 5 };
        let redo = UndoAction::Redo { count: 2 };

        assert_eq!(undo, UndoAction::Undo { count: 5 });
        assert_eq!(redo, UndoAction::Redo { count: 2 });
        assert_ne!(undo, redo);
    }

    #[test]
    fn test_undotree_action_toggle() {
        let action = UndotreeAction::Toggle { buffer_id: 42 };
        assert_eq!(action, UndotreeAction::Toggle { buffer_id: 42 });

        let result = CommandResult::UndotreeAction(action);
        assert!(result.is_undotree_action());
        assert!(!result.is_success());
        assert!(!result.is_error());
        assert!(!result.is_quit());
        assert!(!result.is_undo_action());
    }

    #[test]
    fn test_undotree_action_close() {
        let action = UndotreeAction::Close;
        let result = CommandResult::UndotreeAction(action);
        assert!(result.is_undotree_action());
    }

    #[test]
    fn test_undotree_action_goto_node() {
        let action = UndotreeAction::GotoNode { node_index: 5 };
        assert_eq!(action, UndotreeAction::GotoNode { node_index: 5 });
    }

    #[test]
    fn test_undotree_action_navigation() {
        // Test all navigation variants
        let goto_selected = UndotreeAction::GotoSelected;
        let move_up = UndotreeAction::MoveUp;
        let move_down = UndotreeAction::MoveDown;

        // They should all be distinct
        assert_ne!(goto_selected, move_up);
        assert_ne!(move_up, move_down);
        assert_ne!(goto_selected, move_down);
    }

    #[test]
    fn test_undotree_action_all_variants() {
        // Verify all variants can be constructed and compared
        let variants = [
            UndotreeAction::Toggle { buffer_id: 0 },
            UndotreeAction::Close,
            UndotreeAction::GotoNode { node_index: 0 },
            UndotreeAction::GotoSelected,
            UndotreeAction::MoveUp,
            UndotreeAction::MoveDown,
        ];

        // Each variant wrapped in CommandResult should be an undotree action
        for action in variants {
            let result = CommandResult::UndotreeAction(action);
            assert!(result.is_undotree_action());
        }
    }

    #[test]
    fn test_edit_action_new() {
        let buffer_id = BufferId::from_raw(1);
        let edit = Edit::insert(Position::new(0, 0), "hello");
        let before = Position::new(0, 0);
        let after = Position::new(0, 5);

        let action = EditAction::new(buffer_id, vec![edit], before, after);

        assert_eq!(action.buffer_id, buffer_id);
        assert_eq!(action.edits.len(), 1);
        assert_eq!(action.cursor_before, before);
        assert_eq!(action.cursor_after, after);
    }

    #[test]
    fn test_command_result_edit_action() {
        let buffer_id = BufferId::from_raw(1);
        let edit = Edit::insert(Position::new(0, 0), "hello");
        let before = Position::new(0, 0);
        let after = Position::new(0, 5);

        let result = CommandResult::edit_action(buffer_id, edit, before, after);

        assert!(result.is_edit_action());
        assert!(!result.is_success());
        assert!(!result.is_error());
        assert!(!result.is_quit());
    }

    #[test]
    fn test_command_result_is_edit_action() {
        let buffer_id = BufferId::from_raw(1);
        let edit = Edit::insert(Position::new(0, 0), "x");

        let edit_result =
            CommandResult::edit_action(buffer_id, edit, Position::new(0, 0), Position::new(0, 1));
        let success_result = CommandResult::Success;
        let error_result = CommandResult::error("fail");

        assert!(edit_result.is_edit_action());
        assert!(!success_result.is_edit_action());
        assert!(!error_result.is_edit_action());
    }

    #[test]
    fn test_edit_action_empty_edits() {
        let buffer_id = BufferId::from_raw(1);
        let before = Position::new(0, 0);
        let after = Position::new(0, 0);

        // Empty edits vec is valid (no-op edit)
        let action = EditAction::new(buffer_id, vec![], before, after);

        assert!(action.is_empty());
        assert_eq!(action.edits.len(), 0);
    }

    #[test]
    fn test_edit_action_single() {
        let buffer_id = BufferId::from_raw(1);
        let edit = Edit::insert(Position::new(0, 0), "hello");
        let before = Position::new(0, 0);
        let after = Position::new(0, 5);

        let action = EditAction::single(buffer_id, edit.clone(), before, after);

        assert_eq!(action.edits.len(), 1);
        assert_eq!(action.edits[0], edit);
    }

    #[test]
    fn test_command_result_waiting_for_char() {
        let result = CommandResult::waiting_for_char(FindType::TillBackward, Position::new(0, 10));

        assert!(result.is_waiting_for_char());
        assert!(!result.is_success());
        assert!(!result.is_error());
        assert!(!result.is_quit());
        assert!(!result.is_undo_action());
        assert!(!result.is_edit_action());
    }

    #[test]
    fn test_command_result_waiting_for_replace_char() {
        let result = CommandResult::waiting_for_replace_char(2);

        assert!(result.is_waiting_for_char());
        if let CommandResult::WaitingForChar(ctx) = result {
            assert_eq!(ctx.op_type, crate::char_wait::CharWaitOp::ReplaceChar);
            assert_eq!(ctx.count, Some(2));
        } else {
            panic!("Expected WaitingForChar");
        }
    }

    #[test]
    fn test_command_result_is_waiting_for_char() {
        let wait_result =
            CommandResult::waiting_for_char(FindType::FindForward, Position::new(0, 0));
        let success_result = CommandResult::Success;
        let error_result = CommandResult::error("fail");

        assert!(wait_result.is_waiting_for_char());
        assert!(!success_result.is_waiting_for_char());
        assert!(!error_result.is_waiting_for_char());
    }

    #[test]
    fn test_command_result_repeat_action() {
        let result = CommandResult::RepeatAction;
        assert!(result.is_repeat_action());
        assert!(!result.is_success());
        assert!(!result.is_error());
    }

    #[test]
    fn test_window_action_split() {
        let h_split = WindowAction::SplitHorizontal;
        let v_split = WindowAction::SplitVertical;

        assert_ne!(h_split, v_split);

        let result = CommandResult::WindowAction(h_split);
        assert!(result.is_window_action());
        assert!(!result.is_success());
        assert!(!result.is_error());
        assert!(!result.is_quit());
        assert!(!result.is_undo_action());
    }

    #[test]
    fn test_window_action_close() {
        let close = WindowAction::CloseWindow;
        let close_others = WindowAction::CloseOthers;

        assert_ne!(close, close_others);

        let result = CommandResult::WindowAction(close);
        assert!(result.is_window_action());
    }

    #[test]
    fn test_window_action_focus_direction() {
        use reovim_driver_display::NavigateDirection;

        let left = WindowAction::FocusDirection(NavigateDirection::Left);
        let right = WindowAction::FocusDirection(NavigateDirection::Right);
        let up = WindowAction::FocusDirection(NavigateDirection::Up);
        let down = WindowAction::FocusDirection(NavigateDirection::Down);

        assert_ne!(left, right);
        assert_ne!(up, down);

        let result = CommandResult::WindowAction(left);
        assert!(result.is_window_action());
    }

    #[test]
    fn test_window_action_cycle() {
        let forward = WindowAction::CycleForward;
        let backward = WindowAction::CycleBackward;

        assert_ne!(forward, backward);

        let result = CommandResult::WindowAction(forward);
        assert!(result.is_window_action());
    }

    #[test]
    fn test_window_action_resize() {
        let variants = [
            WindowAction::ResizeHeightIncrease,
            WindowAction::ResizeHeightDecrease,
            WindowAction::ResizeWidthIncrease,
            WindowAction::ResizeWidthDecrease,
            WindowAction::ResizeEqual,
        ];

        // Each variant should be distinct
        for i in 0..variants.len() {
            for j in i + 1..variants.len() {
                assert_ne!(variants[i], variants[j]);
            }
        }

        // Each should be a valid window action
        for action in variants {
            let result = CommandResult::WindowAction(action);
            assert!(result.is_window_action());
        }
    }

    #[test]
    fn test_window_action_all_variants() {
        use reovim_driver_display::NavigateDirection;

        // Verify all variants can be constructed and wrapped
        let variants = [
            WindowAction::SplitHorizontal,
            WindowAction::SplitVertical,
            WindowAction::CloseWindow,
            WindowAction::CloseOthers,
            WindowAction::FocusDirection(NavigateDirection::Left),
            WindowAction::CycleForward,
            WindowAction::CycleBackward,
            WindowAction::ResizeHeightIncrease,
            WindowAction::ResizeHeightDecrease,
            WindowAction::ResizeWidthIncrease,
            WindowAction::ResizeWidthDecrease,
            WindowAction::ResizeEqual,
        ];

        // Each variant wrapped in CommandResult should be a window action
        for action in variants {
            let result = CommandResult::WindowAction(action);
            assert!(result.is_window_action());
        }
    }

    // ========================================================================
    // ModeAction Tests
    // ========================================================================

    #[test]
    fn test_mode_action_push() {
        let action = ModeAction::Push("window".to_string());
        let result = CommandResult::ModeAction(action.clone());

        assert!(result.is_mode_action());
        assert!(!result.is_window_action());
        assert!(!result.is_success());

        // Verify the action value
        if let ModeAction::Push(mode_name) = action {
            assert_eq!(mode_name, "window");
        } else {
            panic!("Expected ModeAction::Push");
        }
    }

    #[test]
    fn test_mode_action_pop() {
        let action = ModeAction::Pop;
        let result = CommandResult::ModeAction(action);

        assert!(result.is_mode_action());
        assert!(!result.is_window_action());
        assert!(!result.is_success());
    }

    #[test]
    fn test_mode_action_set() {
        let action = ModeAction::Set("insert".to_string());
        let result = CommandResult::ModeAction(action.clone());

        assert!(result.is_mode_action());
        assert!(!result.is_window_action());
        assert!(!result.is_success());

        // Verify the action value
        if let ModeAction::Set(mode_name) = action {
            assert_eq!(mode_name, "insert");
        } else {
            panic!("Expected ModeAction::Set");
        }
    }

    #[test]
    fn test_mode_action_equality() {
        let push1 = ModeAction::Push("window".to_string());
        let push2 = ModeAction::Push("window".to_string());
        let push3 = ModeAction::Push("insert".to_string());
        let pop = ModeAction::Pop;
        let set = ModeAction::Set("normal".to_string());

        // Same values should be equal
        assert_eq!(push1, push2);

        // Different variants/values should not be equal
        assert_ne!(push1, push3);
        assert_ne!(push1, pop);
        assert_ne!(push1, set);
        assert_ne!(pop, set);
    }

    #[test]
    fn test_mode_action_all_variants() {
        // Verify all variants can be constructed and wrapped
        let variants = [
            ModeAction::Push("window".to_string()),
            ModeAction::Pop,
            ModeAction::Set("normal".to_string()),
        ];

        // Each variant wrapped in CommandResult should be a mode action
        for action in variants {
            let result = CommandResult::ModeAction(action);
            assert!(result.is_mode_action());
            assert!(!result.is_window_action());
        }
    }

    // ========================================================================
    // BlockInsertAction Tests
    // ========================================================================

    #[test]
    fn test_block_insert_action_insert_start() {
        let action = BlockInsertAction::InsertStart {
            start_line: 2,
            end_line: 5,
            column: 10,
        };
        let result = CommandResult::BlockInsertAction(action);

        assert!(result.is_block_insert_action());
        assert!(!result.is_success());
        assert!(!result.is_error());
        assert!(!result.is_mode_action());
    }

    #[test]
    fn test_block_insert_action_insert_end() {
        let action = BlockInsertAction::InsertEnd {
            start_line: 0,
            end_line: 3,
            column: 15,
        };
        let result = CommandResult::BlockInsertAction(action);

        assert!(result.is_block_insert_action());
        assert!(!result.is_success());
        assert!(!result.is_error());
    }

    #[test]
    fn test_block_insert_action_equality() {
        let start1 = BlockInsertAction::InsertStart {
            start_line: 2,
            end_line: 5,
            column: 10,
        };
        let start2 = BlockInsertAction::InsertStart {
            start_line: 2,
            end_line: 5,
            column: 10,
        };
        let start3 = BlockInsertAction::InsertStart {
            start_line: 0,
            end_line: 5,
            column: 10,
        };
        let end1 = BlockInsertAction::InsertEnd {
            start_line: 2,
            end_line: 5,
            column: 10,
        };

        // Same values should be equal
        assert_eq!(start1, start2);

        // Different values should not be equal
        assert_ne!(start1, start3);
        assert_ne!(start1, end1);
    }

    #[test]
    fn test_block_insert_action_all_variants() {
        let variants = [
            BlockInsertAction::InsertStart {
                start_line: 0,
                end_line: 10,
                column: 5,
            },
            BlockInsertAction::InsertEnd {
                start_line: 0,
                end_line: 10,
                column: 20,
            },
        ];

        // Each variant wrapped in CommandResult should be a block insert action
        for action in variants {
            let result = CommandResult::BlockInsertAction(action);
            assert!(result.is_block_insert_action());
            assert!(!result.is_mode_action());
            assert!(!result.is_window_action());
        }
    }
}
