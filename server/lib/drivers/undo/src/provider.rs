//! Undo provider trait.

use {
    crate::UndoPersistError,
    reovim_driver_vfs::VfsDriver,
    reovim_kernel::api::v1::{BufferId, Edit, EditOrigin, Position, UndoResult, UndoTree},
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
mod tests {
    use {super::*, std::sync::Mutex};

    /// Minimal mock undo provider to test trait default methods.
    #[allow(clippy::struct_field_names)]
    struct MockUndo {
        undo_calls: Mutex<Vec<BufferId>>,
        redo_calls: Mutex<Vec<BufferId>>,
        record_calls: Mutex<Vec<BufferId>>,
    }

    impl MockUndo {
        fn new() -> Self {
            Self {
                undo_calls: Mutex::new(Vec::new()),
                redo_calls: Mutex::new(Vec::new()),
                record_calls: Mutex::new(Vec::new()),
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl UndoProvider for MockUndo {
        fn undo(&self, buffer_id: BufferId) -> Option<UndoResult> {
            self.undo_calls.lock().unwrap().push(buffer_id);
            None
        }
        fn redo(&self, buffer_id: BufferId) -> Option<UndoResult> {
            self.redo_calls.lock().unwrap().push(buffer_id);
            None
        }
        fn redo_branch(&self, _buffer_id: BufferId, _branch_idx: usize) -> Option<UndoResult> {
            None
        }
        fn record(
            &self,
            buffer_id: BufferId,
            _edits: Vec<Edit>,
            _cursor_before: Position,
            _cursor_after: Position,
        ) {
            self.record_calls.lock().unwrap().push(buffer_id);
        }
        fn has_history(&self, _buffer_id: BufferId) -> bool {
            false
        }
        fn remove(&self, _buffer_id: BufferId) {}
        fn buffer_count(&self) -> usize {
            0
        }
        fn get_tree(&self, _buffer_id: BufferId) -> Option<UndoTree> {
            None
        }
        fn begin_batch(&self, _buffer_id: BufferId, _cursor_before: Position) {}
        fn end_batch(&self, _buffer_id: BufferId, _cursor_after: Position) {}
        fn is_batching(&self, _buffer_id: BufferId) -> bool {
            false
        }
        fn persist(
            &self,
            _buffer_id: BufferId,
            _buffer_path: &str,
            _vfs: &dyn VfsDriver,
        ) -> Result<(), UndoPersistError> {
            Ok(())
        }
        fn load(
            &self,
            _buffer_id: BufferId,
            _buffer_path: &str,
            _vfs: &dyn VfsDriver,
        ) -> Result<bool, UndoPersistError> {
            Ok(false)
        }
    }

    #[test]
    fn client_origin_returns_client_variant() {
        let undo = MockUndo::new();
        let origin = undo.client_origin(42);
        assert_eq!(origin, EditOrigin::Client(42));
    }

    #[test]
    fn undo_for_client_falls_back_to_undo() {
        let undo = MockUndo::new();
        let bid = BufferId::from_raw(1);
        let _ = undo.undo_for_client(bid, 5);
        assert_eq!(undo.undo_calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn redo_for_client_falls_back_to_redo() {
        let undo = MockUndo::new();
        let bid = BufferId::from_raw(1);
        let _ = undo.redo_for_client(bid, 5);
        assert_eq!(undo.redo_calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn record_for_client_falls_back_to_record() {
        let undo = MockUndo::new();
        let bid = BufferId::from_raw(1);
        undo.record_for_client(bid, 5, vec![], Position::new(0, 0), Position::new(0, 0));
        assert_eq!(undo.record_calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn init_client_is_noop() {
        let undo = MockUndo::new();
        let bid = BufferId::from_raw(1);
        undo.init_client(bid, 5);
        // Should not panic
    }

    #[test]
    fn remove_client_is_noop() {
        let undo = MockUndo::new();
        let bid = BufferId::from_raw(1);
        undo.remove_client(bid, 5);
        // Should not panic
    }

    #[test]
    fn undo_returns_none() {
        let undo = MockUndo::new();
        let bid = BufferId::from_raw(1);
        assert!(undo.undo(bid).is_none());
    }

    #[test]
    fn redo_returns_none() {
        let undo = MockUndo::new();
        let bid = BufferId::from_raw(1);
        assert!(undo.redo(bid).is_none());
    }

    #[test]
    fn redo_branch_returns_none() {
        let undo = MockUndo::new();
        let bid = BufferId::from_raw(1);
        assert!(undo.redo_branch(bid, 0).is_none());
    }

    #[test]
    fn has_history_returns_false() {
        let undo = MockUndo::new();
        let bid = BufferId::from_raw(1);
        assert!(!undo.has_history(bid));
    }

    #[test]
    fn buffer_count_is_zero() {
        let undo = MockUndo::new();
        assert_eq!(undo.buffer_count(), 0);
    }

    #[test]
    fn get_tree_returns_none() {
        let undo = MockUndo::new();
        let bid = BufferId::from_raw(1);
        assert!(undo.get_tree(bid).is_none());
    }

    #[test]
    fn is_batching_returns_false() {
        let undo = MockUndo::new();
        let bid = BufferId::from_raw(1);
        assert!(!undo.is_batching(bid));
    }

    #[test]
    fn record_stores_buffer_id() {
        let undo = MockUndo::new();
        let bid1 = BufferId::from_raw(1);
        let bid2 = BufferId::from_raw(2);
        undo.record(bid1, vec![], Position::new(0, 0), Position::new(0, 1));
        undo.record(bid2, vec![], Position::new(0, 0), Position::new(0, 1));
        assert_eq!(undo.record_calls.lock().unwrap().len(), 2);
    }

    #[test]
    fn begin_and_end_batch_are_noop() {
        let undo = MockUndo::new();
        let bid = BufferId::from_raw(1);
        undo.begin_batch(bid, Position::new(0, 0));
        undo.end_batch(bid, Position::new(0, 5));
        // Should not panic
    }

    #[test]
    fn remove_is_noop() {
        let undo = MockUndo::new();
        let bid = BufferId::from_raw(1);
        undo.remove(bid);
        // Should not panic
    }

    #[test]
    fn provider_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MockUndo>();
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn provider_is_object_safe() {
        fn _accepts_ref(_: &dyn UndoProvider) {}
        fn _accepts_box(_: Box<dyn UndoProvider>) {}
    }

    #[test]
    fn load_graceful_returns_false_on_no_undo_file() {
        let undo = MockUndo::new();
        let bid = BufferId::from_raw(1);
        let vfs = reovim_driver_vfs::MockVfs::new();
        // MockUndo::load returns Ok(false), so load_graceful returns false
        let loaded = undo.load_graceful(bid, "/tmp/test.rs", &vfs);
        assert!(!loaded);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn load_graceful_returns_false_on_error() {
        struct FailingUndo;

        impl UndoProvider for FailingUndo {
            fn undo(&self, _buffer_id: BufferId) -> Option<UndoResult> {
                None
            }
            fn redo(&self, _buffer_id: BufferId) -> Option<UndoResult> {
                None
            }
            fn redo_branch(&self, _buffer_id: BufferId, _branch_idx: usize) -> Option<UndoResult> {
                None
            }
            fn record(
                &self,
                _buffer_id: BufferId,
                _edits: Vec<Edit>,
                _cursor_before: Position,
                _cursor_after: Position,
            ) {
            }
            fn has_history(&self, _buffer_id: BufferId) -> bool {
                false
            }
            fn remove(&self, _buffer_id: BufferId) {}
            fn buffer_count(&self) -> usize {
                0
            }
            fn get_tree(&self, _buffer_id: BufferId) -> Option<UndoTree> {
                None
            }
            fn begin_batch(&self, _buffer_id: BufferId, _cursor_before: Position) {}
            fn end_batch(&self, _buffer_id: BufferId, _cursor_after: Position) {}
            fn is_batching(&self, _buffer_id: BufferId) -> bool {
                false
            }
            fn persist(
                &self,
                _buffer_id: BufferId,
                _buffer_path: &str,
                _vfs: &dyn VfsDriver,
            ) -> Result<(), UndoPersistError> {
                Ok(())
            }
            fn load(
                &self,
                _buffer_id: BufferId,
                _buffer_path: &str,
                _vfs: &dyn VfsDriver,
            ) -> Result<bool, UndoPersistError> {
                Err(UndoPersistError::Io("disk error".into()))
            }
        }

        let undo = FailingUndo;
        let bid = BufferId::from_raw(1);
        let vfs = reovim_driver_vfs::MockVfs::new();
        let loaded = undo.load_graceful(bid, "/tmp/test.rs", &vfs);
        assert!(!loaded);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn load_graceful_returns_true_on_success() {
        struct SuccessUndo;

        impl UndoProvider for SuccessUndo {
            fn undo(&self, _buffer_id: BufferId) -> Option<UndoResult> {
                None
            }
            fn redo(&self, _buffer_id: BufferId) -> Option<UndoResult> {
                None
            }
            fn redo_branch(&self, _buffer_id: BufferId, _branch_idx: usize) -> Option<UndoResult> {
                None
            }
            fn record(
                &self,
                _buffer_id: BufferId,
                _edits: Vec<Edit>,
                _cursor_before: Position,
                _cursor_after: Position,
            ) {
            }
            fn has_history(&self, _buffer_id: BufferId) -> bool {
                false
            }
            fn remove(&self, _buffer_id: BufferId) {}
            fn buffer_count(&self) -> usize {
                0
            }
            fn get_tree(&self, _buffer_id: BufferId) -> Option<UndoTree> {
                None
            }
            fn begin_batch(&self, _buffer_id: BufferId, _cursor_before: Position) {}
            fn end_batch(&self, _buffer_id: BufferId, _cursor_after: Position) {}
            fn is_batching(&self, _buffer_id: BufferId) -> bool {
                false
            }
            fn persist(
                &self,
                _buffer_id: BufferId,
                _buffer_path: &str,
                _vfs: &dyn VfsDriver,
            ) -> Result<(), UndoPersistError> {
                Ok(())
            }
            fn load(
                &self,
                _buffer_id: BufferId,
                _buffer_path: &str,
                _vfs: &dyn VfsDriver,
            ) -> Result<bool, UndoPersistError> {
                Ok(true) // simulate successful load
            }
        }

        let undo = SuccessUndo;
        let bid = BufferId::from_raw(1);
        let vfs = reovim_driver_vfs::MockVfs::new();
        let loaded = undo.load_graceful(bid, "/tmp/test.rs", &vfs);
        assert!(loaded);
    }

    #[test]
    fn client_origin_different_ids() {
        let undo = MockUndo::new();
        assert_eq!(undo.client_origin(0), EditOrigin::Client(0));
        assert_eq!(undo.client_origin(1), EditOrigin::Client(1));
        assert_eq!(undo.client_origin(usize::MAX), EditOrigin::Client(usize::MAX));
    }

    #[test]
    fn undo_for_client_with_different_clients() {
        let undo = MockUndo::new();
        let bid = BufferId::from_raw(1);
        let _ = undo.undo_for_client(bid, 0);
        let _ = undo.undo_for_client(bid, 99);
        assert_eq!(undo.undo_calls.lock().unwrap().len(), 2);
    }

    #[test]
    fn redo_for_client_with_different_clients() {
        let undo = MockUndo::new();
        let bid = BufferId::from_raw(1);
        let _ = undo.redo_for_client(bid, 0);
        let _ = undo.redo_for_client(bid, 99);
        assert_eq!(undo.redo_calls.lock().unwrap().len(), 2);
    }

    #[test]
    fn record_for_client_with_different_clients() {
        let undo = MockUndo::new();
        let bid = BufferId::from_raw(1);
        undo.record_for_client(bid, 0, vec![], Position::new(0, 0), Position::new(0, 0));
        undo.record_for_client(bid, 99, vec![], Position::new(0, 0), Position::new(0, 0));
        assert_eq!(undo.record_calls.lock().unwrap().len(), 2);
    }

    #[test]
    fn persist_success() {
        let undo = MockUndo::new();
        let bid = BufferId::from_raw(1);
        let vfs = reovim_driver_vfs::MockVfs::new();
        assert!(undo.persist(bid, "/tmp/test.rs", &vfs).is_ok());
    }

    #[test]
    fn load_returns_false() {
        let undo = MockUndo::new();
        let bid = BufferId::from_raw(1);
        let vfs = reovim_driver_vfs::MockVfs::new();
        assert!(!undo.load(bid, "/tmp/test.rs", &vfs).unwrap());
    }

    // =========================================================================
    // Additional coverage: exercise FailingUndo methods individually
    // =========================================================================

    /// Failing undo provider (returns error from load).
    /// Extracted to module level so multiple tests can reuse it.
    struct FailingUndo;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl UndoProvider for FailingUndo {
        fn undo(&self, _buffer_id: BufferId) -> Option<UndoResult> {
            None
        }
        fn redo(&self, _buffer_id: BufferId) -> Option<UndoResult> {
            None
        }
        fn redo_branch(&self, _buffer_id: BufferId, _branch_idx: usize) -> Option<UndoResult> {
            None
        }
        fn record(
            &self,
            _buffer_id: BufferId,
            _edits: Vec<Edit>,
            _cursor_before: Position,
            _cursor_after: Position,
        ) {
        }
        fn has_history(&self, _buffer_id: BufferId) -> bool {
            false
        }
        fn remove(&self, _buffer_id: BufferId) {}
        fn buffer_count(&self) -> usize {
            0
        }
        fn get_tree(&self, _buffer_id: BufferId) -> Option<UndoTree> {
            None
        }
        fn begin_batch(&self, _buffer_id: BufferId, _cursor_before: Position) {}
        fn end_batch(&self, _buffer_id: BufferId, _cursor_after: Position) {}
        fn is_batching(&self, _buffer_id: BufferId) -> bool {
            false
        }
        fn persist(
            &self,
            _buffer_id: BufferId,
            _buffer_path: &str,
            _vfs: &dyn VfsDriver,
        ) -> Result<(), UndoPersistError> {
            Ok(())
        }
        fn load(
            &self,
            _buffer_id: BufferId,
            _buffer_path: &str,
            _vfs: &dyn VfsDriver,
        ) -> Result<bool, UndoPersistError> {
            Err(UndoPersistError::Io("disk error".into()))
        }
    }

    /// Success undo provider (returns Ok(true) from load).
    struct SuccessUndo;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl UndoProvider for SuccessUndo {
        fn undo(&self, _buffer_id: BufferId) -> Option<UndoResult> {
            None
        }
        fn redo(&self, _buffer_id: BufferId) -> Option<UndoResult> {
            None
        }
        fn redo_branch(&self, _buffer_id: BufferId, _branch_idx: usize) -> Option<UndoResult> {
            None
        }
        fn record(
            &self,
            _buffer_id: BufferId,
            _edits: Vec<Edit>,
            _cursor_before: Position,
            _cursor_after: Position,
        ) {
        }
        fn has_history(&self, _buffer_id: BufferId) -> bool {
            true
        }
        fn remove(&self, _buffer_id: BufferId) {}
        fn buffer_count(&self) -> usize {
            1
        }
        fn get_tree(&self, _buffer_id: BufferId) -> Option<UndoTree> {
            None
        }
        fn begin_batch(&self, _buffer_id: BufferId, _cursor_before: Position) {}
        fn end_batch(&self, _buffer_id: BufferId, _cursor_after: Position) {}
        fn is_batching(&self, _buffer_id: BufferId) -> bool {
            false
        }
        fn persist(
            &self,
            _buffer_id: BufferId,
            _buffer_path: &str,
            _vfs: &dyn VfsDriver,
        ) -> Result<(), UndoPersistError> {
            Ok(())
        }
        fn load(
            &self,
            _buffer_id: BufferId,
            _buffer_path: &str,
            _vfs: &dyn VfsDriver,
        ) -> Result<bool, UndoPersistError> {
            Ok(true)
        }
    }

    #[test]
    fn failing_undo_load_graceful() {
        let undo = FailingUndo;
        let bid = BufferId::from_raw(1);
        let vfs = reovim_driver_vfs::MockVfs::new();
        let loaded = undo.load_graceful(bid, "/tmp/test.rs", &vfs);
        assert!(!loaded);
    }

    #[test]
    fn failing_undo_undo_returns_none() {
        let undo = FailingUndo;
        let bid = BufferId::from_raw(1);
        assert!(undo.undo(bid).is_none());
    }

    #[test]
    fn failing_undo_redo_returns_none() {
        let undo = FailingUndo;
        let bid = BufferId::from_raw(1);
        assert!(undo.redo(bid).is_none());
    }

    #[test]
    fn failing_undo_redo_branch_returns_none() {
        let undo = FailingUndo;
        let bid = BufferId::from_raw(1);
        assert!(undo.redo_branch(bid, 0).is_none());
    }

    #[test]
    fn failing_undo_record_no_panic() {
        let undo = FailingUndo;
        let bid = BufferId::from_raw(1);
        undo.record(bid, vec![], Position::new(0, 0), Position::new(0, 1));
    }

    #[test]
    fn failing_undo_has_history_false() {
        let undo = FailingUndo;
        let bid = BufferId::from_raw(1);
        assert!(!undo.has_history(bid));
    }

    #[test]
    fn failing_undo_remove_no_panic() {
        let undo = FailingUndo;
        let bid = BufferId::from_raw(1);
        undo.remove(bid);
    }

    #[test]
    fn failing_undo_buffer_count_zero() {
        let undo = FailingUndo;
        assert_eq!(undo.buffer_count(), 0);
    }

    #[test]
    fn failing_undo_get_tree_none() {
        let undo = FailingUndo;
        let bid = BufferId::from_raw(1);
        assert!(undo.get_tree(bid).is_none());
    }

    #[test]
    fn failing_undo_batch_methods() {
        let undo = FailingUndo;
        let bid = BufferId::from_raw(1);
        undo.begin_batch(bid, Position::new(0, 0));
        undo.end_batch(bid, Position::new(0, 5));
        assert!(!undo.is_batching(bid));
    }

    #[test]
    fn failing_undo_persist_ok() {
        let undo = FailingUndo;
        let bid = BufferId::from_raw(1);
        let vfs = reovim_driver_vfs::MockVfs::new();
        assert!(undo.persist(bid, "/tmp/test.rs", &vfs).is_ok());
    }

    #[test]
    fn failing_undo_load_err() {
        let undo = FailingUndo;
        let bid = BufferId::from_raw(1);
        let vfs = reovim_driver_vfs::MockVfs::new();
        let result = undo.load(bid, "/tmp/test.rs", &vfs);
        assert!(result.is_err());
    }

    #[test]
    fn failing_undo_default_trait_methods() {
        let undo = FailingUndo;
        let bid = BufferId::from_raw(1);
        // Test default multi-client methods
        assert!(undo.undo_for_client(bid, 42).is_none());
        assert!(undo.redo_for_client(bid, 42).is_none());
        undo.record_for_client(bid, 42, vec![], Position::new(0, 0), Position::new(0, 0));
        undo.init_client(bid, 42);
        undo.remove_client(bid, 42);
        assert_eq!(undo.client_origin(42), EditOrigin::Client(42));
    }

    #[test]
    fn success_undo_load_graceful() {
        let undo = SuccessUndo;
        let bid = BufferId::from_raw(1);
        let vfs = reovim_driver_vfs::MockVfs::new();
        let loaded = undo.load_graceful(bid, "/tmp/test.rs", &vfs);
        assert!(loaded);
    }

    #[test]
    fn success_undo_undo_returns_none() {
        let undo = SuccessUndo;
        let bid = BufferId::from_raw(1);
        assert!(undo.undo(bid).is_none());
    }

    #[test]
    fn success_undo_redo_returns_none() {
        let undo = SuccessUndo;
        let bid = BufferId::from_raw(1);
        assert!(undo.redo(bid).is_none());
    }

    #[test]
    fn success_undo_redo_branch_returns_none() {
        let undo = SuccessUndo;
        let bid = BufferId::from_raw(1);
        assert!(undo.redo_branch(bid, 0).is_none());
    }

    #[test]
    fn success_undo_record_no_panic() {
        let undo = SuccessUndo;
        let bid = BufferId::from_raw(1);
        undo.record(bid, vec![], Position::new(0, 0), Position::new(0, 1));
    }

    #[test]
    fn success_undo_has_history_true() {
        let undo = SuccessUndo;
        let bid = BufferId::from_raw(1);
        assert!(undo.has_history(bid));
    }

    #[test]
    fn success_undo_remove_no_panic() {
        let undo = SuccessUndo;
        let bid = BufferId::from_raw(1);
        undo.remove(bid);
    }

    #[test]
    fn success_undo_buffer_count() {
        let undo = SuccessUndo;
        assert_eq!(undo.buffer_count(), 1);
    }

    #[test]
    fn success_undo_get_tree_none() {
        let undo = SuccessUndo;
        let bid = BufferId::from_raw(1);
        assert!(undo.get_tree(bid).is_none());
    }

    #[test]
    fn success_undo_batch_methods() {
        let undo = SuccessUndo;
        let bid = BufferId::from_raw(1);
        undo.begin_batch(bid, Position::new(0, 0));
        undo.end_batch(bid, Position::new(0, 5));
        assert!(!undo.is_batching(bid));
    }

    #[test]
    fn success_undo_persist_ok() {
        let undo = SuccessUndo;
        let bid = BufferId::from_raw(1);
        let vfs = reovim_driver_vfs::MockVfs::new();
        assert!(undo.persist(bid, "/tmp/test.rs", &vfs).is_ok());
    }

    #[test]
    fn success_undo_load_ok_true() {
        let undo = SuccessUndo;
        let bid = BufferId::from_raw(1);
        let vfs = reovim_driver_vfs::MockVfs::new();
        assert!(undo.load(bid, "/tmp/test.rs", &vfs).unwrap());
    }

    #[test]
    fn success_undo_default_trait_methods() {
        let undo = SuccessUndo;
        let bid = BufferId::from_raw(1);
        assert!(undo.undo_for_client(bid, 1).is_none());
        assert!(undo.redo_for_client(bid, 1).is_none());
        undo.record_for_client(bid, 1, vec![], Position::new(0, 0), Position::new(0, 0));
        undo.init_client(bid, 1);
        undo.remove_client(bid, 1);
        assert_eq!(undo.client_origin(1), EditOrigin::Client(1));
    }

    #[test]
    fn failing_undo_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<FailingUndo>();
    }

    #[test]
    fn success_undo_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SuccessUndo>();
    }
}
