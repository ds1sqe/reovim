//! Undo provider trait.

use {
    crate::UndoPersistError,
    reovim_driver_vfs::VfsDriver,
    reovim_kernel::api::v1::{BufferId, Edit, Position, UndoResult, UndoTree},
};

/// Undo provider interface for per-buffer undo/redo operations.
///
/// Implementations should use interior mutability (e.g., `RwLock`) for thread safety
/// since trait methods take `&self`.
///
/// # Design Philosophy
///
/// - **Mechanism focus**: Defines WHAT operations are available
/// - **Thread-safe**: All methods take `&self` with internal locking
/// - **Per-buffer**: Each buffer has independent undo history
/// - **Persistence-aware**: Implementations handle their own disk storage
///
/// # Example
///
/// ```ignore
/// use reovim_driver_undo::UndoProvider;
///
/// // After making an edit
/// provider.record(buffer_id, edits, cursor_before, cursor_after);
///
/// // To undo
/// if let Some(result) = provider.undo(buffer_id) {
///     apply_edits(&mut buffer, &result.edits);
///     buffer.set_cursor(result.cursor);
/// }
///
/// // Persist to disk
/// provider.persist(buffer_id, "/path/to/file.rs", &vfs)?;
/// ```
pub trait UndoProvider: Send + Sync {
    /// Undo the last change for a buffer.
    ///
    /// Returns the edits to apply and cursor position, or `None` if
    /// there's nothing to undo (at the root of the undo tree).
    fn undo(&self, buffer_id: BufferId) -> Option<UndoResult>;

    /// Redo the last undone change for a buffer.
    ///
    /// Returns the edits to apply and cursor position, or `None` if
    /// there's nothing to redo.
    fn redo(&self, buffer_id: BufferId) -> Option<UndoResult>;

    /// Redo following a specific branch for a buffer.
    ///
    /// Like `redo()` but follows a specific branch index instead of
    /// the default/active branch. Used for undotree navigation.
    fn redo_branch(&self, buffer_id: BufferId, branch_idx: usize) -> Option<UndoResult>;

    /// Record an edit transaction for a buffer.
    ///
    /// This should be called after making edits to a buffer so they can
    /// be undone later.
    fn record(
        &self,
        buffer_id: BufferId,
        edits: Vec<Edit>,
        cursor_before: Position,
        cursor_after: Position,
    );

    /// Check if a buffer has any undo history.
    fn has_history(&self, buffer_id: BufferId) -> bool;

    /// Remove the undo tree for a buffer.
    ///
    /// Called when a buffer is closed to free memory.
    fn remove(&self, buffer_id: BufferId);

    /// Get the number of buffers with undo history.
    fn buffer_count(&self) -> usize;

    /// Get a read-only reference to the undo tree for a buffer.
    ///
    /// Returns `None` if no undo history exists for the buffer.
    /// Used for visualization features like `:undotree` panel.
    ///
    /// Note: The returned reference is only valid for the duration of the call.
    /// For thread-safe access, implementations may return a clone.
    fn get_tree(&self, buffer_id: BufferId) -> Option<UndoTree>;

    // ========================================================================
    // Persistence Methods (Epic #417 Part 2)
    // ========================================================================

    /// Persist the undo tree for a buffer to disk.
    ///
    /// Does nothing if the buffer has no undo history.
    ///
    /// # Arguments
    ///
    /// * `buffer_id` - The buffer whose undo tree to persist
    /// * `buffer_path` - The file path associated with the buffer (for undo file naming)
    /// * `vfs` - VFS driver for file operations
    ///
    /// # Errors
    ///
    /// Returns an error if persistence fails (I/O error, serialization error).
    fn persist(
        &self,
        buffer_id: BufferId,
        buffer_path: &str,
        vfs: &dyn VfsDriver,
    ) -> Result<(), UndoPersistError>;

    /// Load the undo tree for a buffer from disk.
    ///
    /// If an undo file exists, it replaces any existing undo history for the buffer.
    /// If no undo file exists, the buffer's undo history is unchanged.
    ///
    /// # Arguments
    ///
    /// * `buffer_id` - The buffer to load undo history for
    /// * `buffer_path` - The file path associated with the buffer
    /// * `vfs` - VFS driver for file operations
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Undo history was loaded from disk
    /// * `Ok(false)` - No undo file exists, history unchanged
    ///
    /// # Errors
    ///
    /// Returns an error if the undo file exists but is corrupt or unreadable.
    fn load(
        &self,
        buffer_id: BufferId,
        buffer_path: &str,
        vfs: &dyn VfsDriver,
    ) -> Result<bool, UndoPersistError>;

    /// Load undo history from disk, falling back to empty on error.
    ///
    /// This is a convenience wrapper around [`load`](Self::load) that logs
    /// errors but doesn't propagate them. Use this for graceful degradation.
    ///
    /// # Returns
    ///
    /// `true` if undo history was loaded, `false` otherwise.
    fn load_graceful(&self, buffer_id: BufferId, buffer_path: &str, vfs: &dyn VfsDriver) -> bool {
        match self.load(buffer_id, buffer_path, vfs) {
            Ok(loaded) => loaded,
            Err(e) => {
                tracing::warn!(
                    "Failed to load undo history for '{}': {}, starting fresh",
                    buffer_path,
                    e
                );
                false
            }
        }
    }
}
