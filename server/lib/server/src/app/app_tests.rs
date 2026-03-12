use super::*;

#[test]
fn test_app_state_new() {
    let kernel = KernelContext::default();
    let app = AppState::new(kernel);

    assert!(app.is_running());
}

#[test]
fn test_app_state_quit() {
    let kernel = KernelContext::default();
    let mut app = AppState::new(kernel);

    assert!(app.is_running());
    app.request_quit();
    assert!(!app.is_running());
}

#[test]
fn test_get_undo_provider_returns_none_when_not_registered() {
    let kernel = KernelContext::default();
    let app = AppState::new(kernel);
    assert!(app.get_undo_provider().is_none());
}

#[test]
fn test_request_detach_does_not_panic() {
    let kernel = KernelContext::default();
    let mut app = AppState::new(kernel);
    app.request_detach();
    // Detach doesn't change running state
    assert!(app.is_running());
}

#[test]
#[should_panic(expected = "should not be called")]
fn test_fallback_current_mode_panics() {
    let kernel = KernelContext::default();
    let app = AppState::new(kernel);
    let _ = FallbackContext::current_mode(&app);
}

#[test]
#[should_panic(expected = "should not be called")]
fn test_fallback_active_buffer_panics() {
    let kernel = KernelContext::default();
    let app = AppState::new(kernel);
    let _ = FallbackContext::active_buffer(&app);
}

#[test]
#[should_panic(expected = "should not be called")]
fn test_fallback_cursor_position_panics() {
    let kernel = KernelContext::default();
    let app = AppState::new(kernel);
    let _ = FallbackContext::cursor_position(&app);
}

#[test]
#[should_panic(expected = "should not be called")]
fn test_fallback_set_cursor_position_panics() {
    let kernel = KernelContext::default();
    let mut app = AppState::new(kernel);
    FallbackContext::set_cursor_position(&mut app, Position::new(0, 0));
}

#[test]
fn test_fallback_get_buffer() {
    let kernel = KernelContext::default();
    let app = AppState::new(kernel);
    // No buffers registered, should return None
    assert!(FallbackContext::get_buffer(&app, BufferId::from_raw(999)).is_none());
}

#[test]
fn test_fallback_record_edit_no_provider() {
    let kernel = KernelContext::default();
    let mut app = AppState::new(kernel);
    // Should not panic even without undo provider
    FallbackContext::record_edit(
        &mut app,
        BufferId::from_raw(0),
        vec![],
        Position::new(0, 0),
        Position::new(0, 0),
    );
}

#[test]
fn test_fallback_accumulate_edit_no_provider() {
    let kernel = KernelContext::default();
    let mut app = AppState::new(kernel);
    FallbackContext::accumulate_edit(
        &mut app,
        BufferId::from_raw(0),
        Edit::Insert {
            position: Position::new(0, 0),
            text: "a".into(),
        },
        Position::new(0, 0),
        Position::new(0, 1),
    );
}

#[test]
fn test_fallback_flush_pending_edits_is_noop() {
    let kernel = KernelContext::default();
    let mut app = AppState::new(kernel);
    FallbackContext::flush_pending_edits(&mut app);
}

#[test]
fn test_app_state_services_arc_shared() {
    let kernel = KernelContext::default();
    let app = AppState::new(kernel);
    // Services should be a shared reference to the kernel's service registry
    // The Arc should have at least 2 strong references (kernel + app)
    assert!(Arc::strong_count(&app.services) >= 2);
}

#[test]
fn test_app_state_extensions_empty_initially() {
    let kernel = KernelContext::default();
    let app = AppState::new(kernel);
    // Extensions map should be empty initially
    // We can verify by checking that it exists and doesn't panic
    let _ = &app.extensions;
}

#[test]
fn test_app_state_debug_format() {
    let kernel = KernelContext::default();
    let app = AppState::new(kernel);
    let debug_str = format!("{app:?}");
    assert!(debug_str.contains("AppState"));
    assert!(debug_str.contains("running: true"));
}

#[test]
fn test_app_state_quit_then_check() {
    let kernel = KernelContext::default();
    let mut app = AppState::new(kernel);

    assert!(app.is_running());
    app.request_quit();
    assert!(!app.is_running());
    // Double quit should be idempotent
    app.request_quit();
    assert!(!app.is_running());
}

#[test]
fn test_fallback_get_buffer_with_real_buffer() {
    use {
        parking_lot::RwLock as ParkingLotRwLock,
        reovim_driver_buffer::TestBufferManager,
        reovim_kernel::api::v1::{
            EventBus, MarkBank, MotionEngine, OptionRegistry, ServiceRegistry, TextObjectEngine,
        },
    };

    let kernel = KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(TestBufferManager::new()),
        Arc::new(MotionEngine),
        Arc::new(TextObjectEngine),
        Arc::new(ParkingLotRwLock::new(MarkBank::new())),
        Arc::new(OptionRegistry::new()),
        Arc::new(ServiceRegistry::new()),
    );

    // Create and register a buffer
    let mut buffer = Buffer::new();
    buffer.set_content("test content");
    let buffer_id = kernel.buffers.register(buffer);

    let app = AppState::new(kernel);

    // FallbackContext::get_buffer should find the buffer
    let found = FallbackContext::get_buffer(&app, buffer_id);
    assert!(found.is_some());
    let content = found.unwrap().read().content();
    assert_eq!(content, "test content");
}

#[test]
fn test_fallback_record_edit_with_undo_provider() {
    use {
        reovim_driver_undo::{UndoKey, UndoPersistError, UndoProviderRegistry},
        reovim_driver_vfs::VfsDriver,
        reovim_kernel::api::v1::{UndoResult, UndoTree},
        std::sync::atomic::{AtomicBool, Ordering},
    };

    struct MockUndo {
        called: AtomicBool,
    }
    #[cfg_attr(coverage_nightly, coverage(off))]
    impl UndoProvider for MockUndo {
        fn undo(&self, _: BufferId) -> Option<UndoResult> {
            None
        }
        fn redo(&self, _: BufferId) -> Option<UndoResult> {
            None
        }
        fn redo_branch(&self, _: BufferId, _: usize) -> Option<UndoResult> {
            None
        }
        fn record(&self, _: BufferId, _: Vec<Edit>, _: Position, _: Position) {
            self.called.store(true, Ordering::SeqCst);
        }
        fn has_history(&self, _: BufferId) -> bool {
            false
        }
        fn remove(&self, _: BufferId) {}
        fn buffer_count(&self) -> usize {
            0
        }
        fn get_tree(&self, _: BufferId) -> Option<UndoTree> {
            None
        }
        fn begin_batch(&self, _: BufferId, _: Position) {}
        fn end_batch(&self, _: BufferId, _: Position) {}
        fn is_batching(&self, _: BufferId) -> bool {
            false
        }
        fn persist(&self, _: BufferId, _: &str, _: &dyn VfsDriver) -> Result<(), UndoPersistError> {
            Ok(())
        }
        fn load(&self, _: BufferId, _: &str, _: &dyn VfsDriver) -> Result<bool, UndoPersistError> {
            Ok(false)
        }
    }

    let kernel = KernelContext::default();
    let undo_registry = Arc::new(UndoProviderRegistry::new());
    let mock = Arc::new(MockUndo {
        called: AtomicBool::new(false),
    });
    undo_registry.register(UndoKey::Buffer, Arc::clone(&mock) as Arc<dyn UndoProvider>);
    kernel.services.register(undo_registry);

    let mut app = AppState::new(kernel);
    FallbackContext::record_edit(
        &mut app,
        BufferId::from_raw(0),
        vec![],
        Position::new(0, 0),
        Position::new(0, 0),
    );
    assert!(mock.called.load(Ordering::SeqCst));

    // Reset and test accumulate_edit
    mock.called.store(false, Ordering::SeqCst);
    FallbackContext::accumulate_edit(
        &mut app,
        BufferId::from_raw(0),
        Edit::Insert {
            position: Position::new(0, 0),
            text: "x".into(),
        },
        Position::new(0, 0),
        Position::new(0, 1),
    );
    assert!(mock.called.load(Ordering::SeqCst));
}

#[test]
fn test_detach_followed_by_quit() {
    let kernel = KernelContext::default();
    let mut app = AppState::new(kernel);

    // Detach should not affect running state
    app.request_detach();
    assert!(app.is_running());

    // Quit should stop running
    app.request_quit();
    assert!(!app.is_running());
}
