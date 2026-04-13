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
    let vfs = reovim_subsys_vfs::MockVfs::new();
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
    let vfs = reovim_subsys_vfs::MockVfs::new();
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
    let vfs = reovim_subsys_vfs::MockVfs::new();
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
    let vfs = reovim_subsys_vfs::MockVfs::new();
    assert!(undo.persist(bid, "/tmp/test.rs", &vfs).is_ok());
}

#[test]
fn load_returns_false() {
    let undo = MockUndo::new();
    let bid = BufferId::from_raw(1);
    let vfs = reovim_subsys_vfs::MockVfs::new();
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
    let vfs = reovim_subsys_vfs::MockVfs::new();
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
    let vfs = reovim_subsys_vfs::MockVfs::new();
    assert!(undo.persist(bid, "/tmp/test.rs", &vfs).is_ok());
}

#[test]
fn failing_undo_load_err() {
    let undo = FailingUndo;
    let bid = BufferId::from_raw(1);
    let vfs = reovim_subsys_vfs::MockVfs::new();
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
    let vfs = reovim_subsys_vfs::MockVfs::new();
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
    let vfs = reovim_subsys_vfs::MockVfs::new();
    assert!(undo.persist(bid, "/tmp/test.rs", &vfs).is_ok());
}

#[test]
fn success_undo_load_ok_true() {
    let undo = SuccessUndo;
    let bid = BufferId::from_raw(1);
    let vfs = reovim_subsys_vfs::MockVfs::new();
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
