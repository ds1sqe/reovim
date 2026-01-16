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

use {
    crate::char_wait::{CharWaitContext, FindType},
    reovim_kernel::api::v1::{BufferId, Edit, Position},
};

// ============================================================================
// Search Types (Policy-Adjacent)
// ============================================================================

/// Search direction for / and ? commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchDirection {
    /// Search forward from cursor (/)
    #[default]
    Forward,
    /// Search backward from cursor (?)
    Backward,
}

/// Search action intent returned by commands.
///
/// Commands return this to request search operations. The runner handles
/// the actual search execution, input mode management, and pattern storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchAction {
    /// Enter search input mode (/ or ?)
    EnterSearchMode { direction: SearchDirection },
    /// Go to next match in the same direction (n)
    Next,
    /// Go to previous match / reverse direction (N)
    Previous,
    /// Search word under cursor (* or #)
    WordUnderCursor { direction: SearchDirection },
    /// Clear search highlighting (:noh)
    ClearHighlight,
}

// ============================================================================
// Undo/Redo Action Types
// ============================================================================

/// Undo/redo action intent returned by commands.
///
/// Commands return this to request undo/redo operations. The runner
/// handles the actual undo tree manipulation, maintaining separation
/// of concerns between command (policy) and runner (mechanism).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UndoAction {
    /// Request to undo the specified number of changes.
    Undo { count: usize },
    /// Request to redo the specified number of changes.
    Redo { count: usize },
}

/// Undotree visualization action intent returned by commands.
///
/// Commands return this to request undotree operations. The runner
/// handles the actual panel creation, navigation, and tree traversal,
/// maintaining separation of concerns between command (policy) and
/// runner (mechanism).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UndotreeAction {
    /// Toggle undotree panel for the specified buffer.
    Toggle { buffer_id: usize },
    /// Close undotree panel.
    Close,
    /// Navigate to a specific node in the undotree.
    GotoNode { node_index: usize },
    /// Go to the currently selected node.
    GotoSelected,
    /// Move selection up (toward parent).
    MoveUp,
    /// Move selection down (toward child).
    MoveDown,
}

// ============================================================================
// Edit Action Type
// ============================================================================

/// Edit action intent returned by commands that modify buffer content.
///
/// Commands return this to report edits they made. The runner records
/// these edits in the undo registry for later undo/redo operations.
///
/// # Design Philosophy
///
/// This follows the callback pattern where commands declare WHAT they did
/// (the edit), and the runner decides HOW to handle it (record in undo tree).
///
/// # Example
///
/// ```ignore
/// fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
///     let buffer_id = args.buffer_id().unwrap();
///     let buffer = ctx.buffers.get(buffer_id).unwrap();
///
///     let cursor_before = buffer.position();
///     let edit = buffer.insert("hello");
///     let cursor_after = buffer.position();
///
///     CommandResult::EditAction(EditAction::new(
///         buffer_id, vec![edit], cursor_before, cursor_after
///     ))
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditAction {
    /// The buffer that was edited.
    pub buffer_id: BufferId,
    /// The edits that were made (in order applied).
    pub edits: Vec<Edit>,
    /// Cursor position before the edits were applied.
    pub cursor_before: Position,
    /// Cursor position after the edits were applied.
    pub cursor_after: Position,
}

impl EditAction {
    /// Create a new edit action.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec cannot be const-constructed
    pub fn new(
        buffer_id: BufferId,
        edits: Vec<Edit>,
        cursor_before: Position,
        cursor_after: Position,
    ) -> Self {
        Self {
            buffer_id,
            edits,
            cursor_before,
            cursor_after,
        }
    }

    /// Create an edit action from a single edit.
    #[must_use]
    pub fn single(
        buffer_id: BufferId,
        edit: Edit,
        cursor_before: Position,
        cursor_after: Position,
    ) -> Self {
        Self::new(buffer_id, vec![edit], cursor_before, cursor_after)
    }

    /// Check if this action has no edits (no-op).
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec::is_empty is not const stable
    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }
}

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
}
