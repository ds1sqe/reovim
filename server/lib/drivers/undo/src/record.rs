//! Undo record type for test mocks.

use reovim_kernel::api::v1::BufferId;
use reovim_types_text::{Edit, Position};

/// A recorded undo entry capturing the parameters of [`super::UndoProvider::record()`].
///
/// Used in test mocks to store recorded edits for verification.
#[derive(Debug, Clone)]
pub struct UndoRecord {
    /// Buffer the edits belong to.
    pub buffer_id: BufferId,
    /// The edits that were recorded.
    pub edits: Vec<Edit>,
    /// Cursor position before the edits.
    pub cursor_before: Position,
    /// Cursor position after the edits.
    pub cursor_after: Position,
}
