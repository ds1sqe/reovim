//! Edit action types for command results.
//!
//! Provides types for reporting buffer edits made by commands.

use reovim_kernel::api::v1::{BufferId, Edit, Position};

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
