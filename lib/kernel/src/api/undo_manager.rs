//! Undo/redo management trait.
//!
//! Provides the mechanism for undo/redo operations on buffers.
//! The kernel provides the trait (mechanism), modules decide when to checkpoint (policy).
//!
//! # Design Principle
//!
//! - **Mechanism (Kernel)**: `UndoManager` trait, `UndoTree` data structure, Edit application
//! - **Policy (Module)**: When to create checkpoints, undo grouping rules, :undolevels
//!
//! # Example
//!
//! ```ignore
//! use reovim_kernel::api::v1::*;
//!
//! fn undo_last_edit(undo: &dyn UndoManager, buffer_id: BufferId) -> Result<(), UndoError> {
//!     if undo.can_undo(buffer_id) {
//!         if let Some(edit) = undo.undo(buffer_id)? {
//!             // Apply the inverted edit to the buffer
//!         }
//!     }
//!     Ok(())
//! }
//! ```

use crate::{
    block::UndoResult,
    mm::{BufferId, Position},
};

// ============================================================================
// Error Types
// ============================================================================

/// Errors from undo operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UndoError {
    /// Buffer not found.
    BufferNotFound(BufferId),
    /// Nothing to undo.
    NothingToUndo,
    /// Nothing to redo.
    NothingToRedo,
    /// Undo limit reached.
    LimitReached,
}

impl std::fmt::Display for UndoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BufferNotFound(id) => write!(f, "buffer not found: {id:?}"),
            Self::NothingToUndo => write!(f, "nothing to undo"),
            Self::NothingToRedo => write!(f, "nothing to redo"),
            Self::LimitReached => write!(f, "undo limit reached"),
        }
    }
}

impl std::error::Error for UndoError {}

// ============================================================================
// UndoManager Trait
// ============================================================================

/// Undo manager for buffer history.
///
/// Provides the mechanism for undo/redo operations.
///
/// - **Mechanism (Kernel)**: Track edit history, apply undo/redo
/// - **Policy (Module)**: When to call `checkpoint()`, grouping rules
///
/// # Implementors
///
/// The kernel provides a default implementation that wraps `UndoTree`.
/// Modules can customize behavior by controlling when checkpoints are created.
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::api::v1::*;
///
/// // Module decides when to checkpoint (policy)
/// fn handle_command(undo: &dyn UndoManager, buffer_id: BufferId, count: usize) {
///     // Checkpoint before multi-command operation
///     let _ = undo.checkpoint(buffer_id, Position::new(0, 0));
///
///     for _ in 0..count {
///         // ... apply edits ...
///     }
///
///     // All edits since checkpoint will undo together
/// }
/// ```
pub trait UndoManager: Send + Sync {
    /// Create a checkpoint for the current buffer state.
    ///
    /// Edits made after this checkpoint will be grouped together
    /// and undone as a single unit.
    ///
    /// # Arguments
    ///
    /// * `buffer_id` - The buffer to checkpoint
    /// * `cursor_pos` - Current cursor position (for restoration)
    ///
    /// # Errors
    ///
    /// Returns `UndoError` if the buffer is not found.
    fn checkpoint(&self, buffer_id: BufferId, cursor_pos: Position) -> Result<(), UndoError>;

    /// Undo the last edit group.
    ///
    /// Returns the `UndoResult` containing edits to apply (already inverted)
    /// and the cursor position to restore.
    ///
    /// Returns `Ok(None)` if nothing to undo.
    ///
    /// # Errors
    ///
    /// Returns `UndoError` if the buffer is not found.
    fn undo(&self, buffer_id: BufferId) -> Result<Option<UndoResult>, UndoError>;

    /// Redo a previously undone edit group.
    ///
    /// Returns the `UndoResult` containing edits to reapply
    /// and the cursor position to restore.
    ///
    /// Returns `Ok(None)` if nothing to redo.
    ///
    /// # Errors
    ///
    /// Returns `UndoError` if the buffer is not found.
    fn redo(&self, buffer_id: BufferId) -> Result<Option<UndoResult>, UndoError>;

    /// Clear undo history for a buffer.
    ///
    /// Called when buffer content is replaced entirely (e.g., :edit!).
    fn clear(&self, buffer_id: BufferId);

    /// Check if undo is available.
    fn can_undo(&self, buffer_id: BufferId) -> bool;

    /// Check if redo is available.
    fn can_redo(&self, buffer_id: BufferId) -> bool;

    /// Get the number of undo levels available.
    fn undo_count(&self, buffer_id: BufferId) -> usize {
        // Default implementation - implementors can override
        let _ = buffer_id;
        0
    }

    /// Get the number of redo levels available.
    fn redo_count(&self, buffer_id: BufferId) -> usize {
        // Default implementation - implementors can override
        let _ = buffer_id;
        0
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_undo_error_display() {
        let err = UndoError::NothingToUndo;
        assert_eq!(err.to_string(), "nothing to undo");

        let err = UndoError::BufferNotFound(BufferId::new());
        assert!(err.to_string().contains("buffer not found"));
    }
}
