//! Undo/redo operations API.
//!
//! This module provides the [`UndoApi`] trait for undo and redo operations.
//! Commands use this to perform undo/redo on the active buffer.
//!
//! # Design
//!
//! Following Unix philosophy: this trait does ONE thing well - undo management.
//! The actual undo tree is stored in the `UndoProvider` service, accessed via
//! `ServiceRegistry`.
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_session::api::UndoApi;
//!
//! fn undo_last_change<S: UndoApi>(session: &mut S) {
//!     if let Some(buffer) = session.active_buffer() {
//!         if let Some(result) = session.undo(buffer) {
//!             // Apply inverse edits to buffer
//!             // Move cursor to result.cursor
//!         }
//!     }
//! }
//! ```

use reovim_kernel::api::v1::{BufferId, Edit, Position, UndoResult};

/// Undo/redo operations API.
///
/// Provides access to undo/redo functionality for resolvers and commands.
/// Implementations access the `UndoProvider` service from the kernel's
/// service registry.
pub trait UndoApi: Send {
    /// Undo the last change for a buffer.
    ///
    /// Returns the edits to apply (already inverted) and cursor position
    /// to restore. Returns `None` if there's nothing to undo.
    ///
    /// # Note
    ///
    /// The returned edits should be applied to the buffer. The cursor
    /// position should be restored after applying edits.
    fn undo(&mut self, buffer: BufferId) -> Option<UndoResult>;

    /// Redo the last undone change for a buffer.
    ///
    /// Returns the edits to apply and cursor position to restore.
    /// Returns `None` if there's nothing to redo.
    fn redo(&mut self, buffer: BufferId) -> Option<UndoResult>;

    /// Record edits for undo history.
    ///
    /// This should be called after making edits to a buffer so they can
    /// be undone later. The edits are stored with cursor positions for
    /// proper restoration.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The buffer that was edited
    /// * `edits` - The edits that were made
    /// * `cursor_before` - Cursor position before the edits
    /// * `cursor_after` - Cursor position after the edits
    fn record_edit(
        &mut self,
        buffer: BufferId,
        edits: Vec<Edit>,
        cursor_before: Position,
        cursor_after: Position,
    );

    /// Check if undo is available for a buffer.
    fn can_undo(&self, buffer: BufferId) -> bool;

    /// Check if redo is available for a buffer.
    fn can_redo(&self, buffer: BufferId) -> bool;
}
