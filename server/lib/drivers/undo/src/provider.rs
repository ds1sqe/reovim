//! Undo provider trait.

use {
    crate::UndoPersistError,
    reovim_domain_text::{Edit, EditOrigin, Position, UndoResult, UndoTree},
    reovim_driver_vfs::VfsDriver,
    reovim_kernel::api::v1::BufferId,
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
    // Batch Methods (Insert Mode Undo Grouping)
    // ========================================================================

    /// Begin a batch for accumulating edits.
    ///
    /// While a batch is active, `record()` calls accumulate edits instead of
    /// creating separate undo entries. Call `end_batch()` to commit all
    /// accumulated edits as a single undo entry.
    ///
    /// This is used for Vim-style insert mode undo: all characters typed
    /// during a single insert session become one undo entry.
    ///
    /// # Arguments
    ///
    /// * `buffer_id` - The buffer to start batching for
    /// * `cursor_before` - Cursor position at batch start (for undo restoration)
    fn begin_batch(&self, buffer_id: BufferId, cursor_before: Position);

    /// End the current batch and commit accumulated edits.
    ///
    /// All edits recorded since `begin_batch()` are committed as a single
    /// undo entry. If no edits were recorded, nothing is committed.
    ///
    /// # Arguments
    ///
    /// * `buffer_id` - The buffer to end batching for
    /// * `cursor_after` - Cursor position at batch end (for redo restoration)
    fn end_batch(&self, buffer_id: BufferId, cursor_after: Position);

    /// Check if a batch is currently active for a buffer.
    fn is_batching(&self, buffer_id: BufferId) -> bool;

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

    // ========================================================================
    // Multi-Client Undo Methods (#471)
    // ========================================================================

    /// Undo the last change made by a specific client.
    ///
    /// This navigates the undo tree backwards, skipping nodes that were
    /// created by other clients, until it finds a node with matching origin.
    ///
    /// # Arguments
    ///
    /// * `buffer_id` - The buffer to undo in
    /// * `client_id` - The client whose changes to undo
    ///
    /// # Returns
    ///
    /// The edits to apply and cursor position, or `None` if there are no
    /// changes by this client to undo.
    ///
    /// # Default Implementation
    ///
    /// Falls back to regular `undo()` - implementations should override
    /// for proper multi-client support.
    fn undo_for_client(&self, buffer_id: BufferId, client_id: usize) -> Option<UndoResult> {
        let _ = client_id;
        self.undo(buffer_id)
    }

    /// Redo the last undone change made by a specific client.
    ///
    /// This navigates the undo tree forward, skipping nodes that were
    /// created by other clients, until it finds a node with matching origin.
    ///
    /// # Arguments
    ///
    /// * `buffer_id` - The buffer to redo in
    /// * `client_id` - The client whose changes to redo
    ///
    /// # Returns
    ///
    /// The edits to apply and cursor position, or `None` if there are no
    /// changes by this client to redo.
    ///
    /// # Default Implementation
    ///
    /// Falls back to regular `redo()` - implementations should override
    /// for proper multi-client support.
    fn redo_for_client(&self, buffer_id: BufferId, client_id: usize) -> Option<UndoResult> {
        let _ = client_id;
        self.redo(buffer_id)
    }

    /// Record an edit transaction with client origin tagging.
    ///
    /// Like [`record`](Self::record), but tags the undo node with the
    /// `EditOrigin::Client(client_id)` so that `undo_for_client` can
    /// identify which client made this change.
    ///
    /// # Arguments
    ///
    /// * `buffer_id` - The buffer that was edited
    /// * `client_id` - The client that made this edit
    /// * `edits` - The edits that were made
    /// * `cursor_before` - Cursor position before the edits
    /// * `cursor_after` - Cursor position after the edits
    ///
    /// # Default Implementation
    ///
    /// Falls back to regular `record()` - implementations should override
    /// for proper origin tagging.
    fn record_for_client(
        &self,
        buffer_id: BufferId,
        client_id: usize,
        edits: Vec<Edit>,
        cursor_before: Position,
        cursor_after: Position,
    ) {
        let _ = client_id;
        self.record(buffer_id, edits, cursor_before, cursor_after);
    }

    /// Initialize per-client undo cursor tracking for a buffer.
    ///
    /// Called when a client attaches to a session or opens a buffer.
    /// Implementations should set the client's undo cursor to the current
    /// position in the undo tree.
    ///
    /// # Arguments
    ///
    /// * `buffer_id` - The buffer to initialize tracking for
    /// * `client_id` - The client to initialize
    ///
    /// # Default Implementation
    ///
    /// No-op - implementations should override for multi-client support.
    fn init_client(&self, buffer_id: BufferId, client_id: usize) {
        let _ = (buffer_id, client_id);
    }

    /// Remove per-client undo cursor tracking for a buffer.
    ///
    /// Called when a client disconnects or closes a buffer.
    /// Implementations should clean up any per-client state.
    ///
    /// # Arguments
    ///
    /// * `buffer_id` - The buffer to remove tracking from
    /// * `client_id` - The client to remove
    ///
    /// # Default Implementation
    ///
    /// No-op - implementations should override for multi-client support.
    fn remove_client(&self, buffer_id: BufferId, client_id: usize) {
        let _ = (buffer_id, client_id);
    }

    /// Get the edit origin for a client ID.
    ///
    /// Helper method to convert a `usize` client ID to `EditOrigin`.
    ///
    /// # Default Implementation
    ///
    /// Returns `EditOrigin::Client(client_id)`.
    fn client_origin(&self, client_id: usize) -> EditOrigin {
        EditOrigin::Client(client_id)
    }
}

#[cfg(test)]
#[path = "provider_tests.rs"]
mod tests;
