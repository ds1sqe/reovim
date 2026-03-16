use super::*;

// ========================================================================
// WriteCommand tests
// ========================================================================

#[test]
fn test_write_command_id() {
    let cmd = WriteCommand;
    assert_eq!(cmd.id().name(), "write");
    assert_eq!(cmd.id().module().as_str(), "commands");
}

#[test]
fn test_write_command_names() {
    let cmd = WriteCommand;
    let names = cmd.names();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"w"));
    assert!(names.contains(&"write"));
}

#[test]
fn test_write_command_description() {
    let cmd = WriteCommand;
    let desc = cmd.description();
    assert!(!desc.is_empty());
    assert!(desc.contains("Write"));
}

#[test]
fn test_write_command_args() {
    let cmd = WriteCommand;
    let args = cmd.args();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].name, "file");
    assert_eq!(args[0].kind, reovim_driver_command::ArgKind::Rest);
    assert!(!args[0].required);
}

#[test]
fn test_write_command_complete_returns_empty() {
    let cmd = WriteCommand;
    let completions = cmd.complete("some_partial");
    assert!(completions.is_empty());
}

#[test]
fn test_write_command_debug() {
    let cmd = WriteCommand;
    let debug = format!("{cmd:?}");
    assert!(debug.contains("WriteCommand"));
}

#[test]
fn test_write_command_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<WriteCommand>();
}

#[test]
fn test_write_cmd_id_const() {
    assert_eq!(WRITE_CMD_ID.name(), "write");
    assert_eq!(WRITE_CMD_ID.module().as_str(), "commands");
}

// ========================================================================
// WriteQuitCommand tests
// ========================================================================

#[test]
fn test_write_quit_command_id() {
    let cmd = WriteQuitCommand;
    assert_eq!(cmd.id().name(), "write-quit");
    assert_eq!(cmd.id().module().as_str(), "commands");
}

#[test]
fn test_write_quit_command_names() {
    let cmd = WriteQuitCommand;
    let names = cmd.names();
    assert_eq!(names.len(), 1);
    assert!(names.contains(&"wq"));
}

#[test]
fn test_write_quit_command_description() {
    let cmd = WriteQuitCommand;
    let desc = cmd.description();
    assert!(!desc.is_empty());
    assert!(desc.contains("Write"));
    assert!(desc.contains("quit"));
}

#[test]
fn test_write_quit_command_complete_returns_empty() {
    let cmd = WriteQuitCommand;
    let completions = cmd.complete("anything");
    assert!(completions.is_empty());
}

#[test]
fn test_write_quit_command_debug() {
    let cmd = WriteQuitCommand;
    let debug = format!("{cmd:?}");
    assert!(debug.contains("WriteQuitCommand"));
}

#[test]
fn test_write_quit_command_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<WriteQuitCommand>();
}

// ========================================================================
// WriteCommand execution tests
// ========================================================================

#[test]
fn test_write_no_buffer_returns_error() {
    use reovim_driver_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    harness.with_runtime(|runtime| {
        let cmd = WriteCommand;
        // No buffer_id set on context
        let ctx = CommandContext::new();
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
        if let CommandResult::Error(msg) = &result {
            assert!(msg.contains("no buffer"));
        }
    });
}

#[test]
fn test_write_no_filename_returns_error() {
    use {
        reovim_driver_session::testing::TestSessionRuntime, reovim_driver_vfs::MockVfs,
        std::sync::Arc,
    };

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let buffer_id = harness.active_buffer().unwrap();
    // Buffer has no file path, no explicit file arg → error
    harness.with_runtime(|runtime| {
        let cmd = WriteCommand;
        let mut ctx = CommandContext::new();
        ctx.set_buffer_id(buffer_id);
        let mock_vfs = Arc::new(MockVfs::new());
        ctx.set_vfs(mock_vfs as Arc<dyn reovim_driver_vfs::VfsDriver>);
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
        if let CommandResult::Error(msg) = &result {
            assert!(msg.contains("No file name"));
        }
    });
}

#[test]
fn test_write_vfs_not_available_returns_error() {
    use reovim_driver_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let buffer_id = harness.active_buffer().unwrap();
    // Set file path on buffer but don't set VFS
    harness
        .kernel()
        .buffers
        .get(buffer_id)
        .unwrap()
        .write()
        .set_file_path(Some("/tmp/test.txt".to_string()));

    harness.with_runtime(|runtime| {
        let cmd = WriteCommand;
        let mut ctx = CommandContext::new();
        ctx.set_buffer_id(buffer_id);
        // No VFS set
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
        if let CommandResult::Error(msg) = &result {
            assert!(msg.contains("VFS not available"));
        }
    });
}

#[test]
fn test_write_vfs_error_returns_error() {
    use {
        reovim_driver_session::testing::TestSessionRuntime,
        reovim_driver_vfs::{MockErrorKind, MockVfs},
        std::sync::Arc,
    };

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let buffer_id = harness.active_buffer().unwrap();
    harness
        .kernel()
        .buffers
        .get(buffer_id)
        .unwrap()
        .write()
        .set_file_path(Some("/tmp/test.txt".to_string()));

    let mock_vfs = Arc::new(MockVfs::new());
    mock_vfs.set_error("/tmp/test.txt", MockErrorKind::PermissionDenied);

    harness.with_runtime(|runtime| {
        let cmd = WriteCommand;
        let mut ctx = CommandContext::new();
        ctx.set_buffer_id(buffer_id);
        ctx.set_vfs(Arc::clone(&mock_vfs) as Arc<dyn reovim_driver_vfs::VfsDriver>);
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
        if let CommandResult::Error(msg) = &result {
            assert!(msg.contains("Write failed"));
        }
    });
}

#[test]
fn test_write_success_with_existing_path() {
    use {
        reovim_driver_session::testing::TestSessionRuntime, reovim_driver_vfs::MockVfs,
        std::sync::Arc,
    };

    let mut harness = TestSessionRuntime::with_buffer("hello world");
    let buffer_id = harness.active_buffer().unwrap();
    harness
        .kernel()
        .buffers
        .get(buffer_id)
        .unwrap()
        .write()
        .set_file_path(Some("/tmp/test.txt".to_string()));
    // Mark buffer as modified
    harness
        .kernel()
        .buffers
        .get(buffer_id)
        .unwrap()
        .write()
        .set_modified(true);

    let mock_vfs = Arc::new(MockVfs::new());

    harness.with_runtime(|runtime| {
        let cmd = WriteCommand;
        let mut ctx = CommandContext::new();
        ctx.set_buffer_id(buffer_id);
        ctx.set_vfs(Arc::clone(&mock_vfs) as Arc<dyn reovim_driver_vfs::VfsDriver>);
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());

        // Verify VFS was called
        let calls = mock_vfs.write_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, std::path::PathBuf::from("/tmp/test.txt"));
        assert_eq!(String::from_utf8_lossy(&calls[0].1), "hello world");

        // Modified flag should be cleared
        let is_modified = runtime
            .kernel()
            .buffers
            .get(buffer_id)
            .unwrap()
            .read()
            .is_modified();
        assert!(!is_modified);
    });
}

#[test]
fn test_write_with_explicit_file_renames_buffer() {
    use {
        reovim_driver_command_types::ArgValue, reovim_driver_session::testing::TestSessionRuntime,
        reovim_driver_vfs::MockVfs, std::sync::Arc,
    };

    let mut harness = TestSessionRuntime::with_buffer("save as content");
    let buffer_id = harness.active_buffer().unwrap();

    let mock_vfs = Arc::new(MockVfs::new());

    harness.with_runtime(|runtime| {
        let cmd = WriteCommand;
        let mut ctx = CommandContext::new();
        ctx.set_buffer_id(buffer_id);
        ctx.set("file", ArgValue::String("/tmp/new_file.txt".to_string()));
        ctx.set_vfs(Arc::clone(&mock_vfs) as Arc<dyn reovim_driver_vfs::VfsDriver>);
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());

        // Verify VFS wrote to the new path
        let calls = mock_vfs.write_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, std::path::PathBuf::from("/tmp/new_file.txt"));

        // Buffer should be renamed
        let path = runtime
            .kernel()
            .buffers
            .get(buffer_id)
            .unwrap()
            .read()
            .file_path()
            .map(String::from);
        assert_eq!(path.as_deref(), Some("/tmp/new_file.txt"));
    });
}

#[test]
fn test_write_emits_buffer_saved_event() {
    use {
        reovim_driver_session::testing::TestSessionRuntime,
        reovim_driver_vfs::MockVfs,
        reovim_kernel::api::v1::events::kernel::BufferSaved,
        std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };

    let mut harness = TestSessionRuntime::with_buffer("event test");
    let buffer_id = harness.active_buffer().unwrap();
    harness
        .kernel()
        .buffers
        .get(buffer_id)
        .unwrap()
        .write()
        .set_file_path(Some("/tmp/saved.txt".to_string()));

    let mock_vfs = Arc::new(MockVfs::new());

    // Subscribe to BufferSaved events
    let event_received = Arc::new(AtomicBool::new(false));
    let event_flag = Arc::clone(&event_received);
    let _sub = harness
        .kernel()
        .event_bus
        .subscribe::<BufferSaved, _>(0, move |_event| {
            event_flag.store(true, Ordering::SeqCst);
            reovim_kernel::api::v1::EventResult::Handled
        });

    harness.with_runtime(|runtime| {
        let cmd = WriteCommand;
        let mut ctx = CommandContext::new();
        ctx.set_buffer_id(buffer_id);
        ctx.set_vfs(Arc::clone(&mock_vfs) as Arc<dyn reovim_driver_vfs::VfsDriver>);
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());
    });

    assert!(event_received.load(Ordering::SeqCst), "BufferSaved event should be emitted");
}

#[test]
fn test_write_emits_buffer_will_save_event() {
    use {
        reovim_driver_session::testing::TestSessionRuntime,
        reovim_driver_vfs::MockVfs,
        reovim_kernel::api::v1::events::kernel::BufferWillSave,
        std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };

    let mut harness = TestSessionRuntime::with_buffer("format me");
    let buffer_id = harness.active_buffer().unwrap();
    harness
        .kernel()
        .buffers
        .get(buffer_id)
        .unwrap()
        .write()
        .set_file_path(Some("/tmp/will-save.txt".to_string()));

    let mock_vfs = Arc::new(MockVfs::new());

    let event_received = Arc::new(AtomicBool::new(false));
    let event_flag = Arc::clone(&event_received);
    let _sub = harness
        .kernel()
        .event_bus
        .subscribe::<BufferWillSave, _>(0, move |_event| {
            event_flag.store(true, Ordering::SeqCst);
            reovim_kernel::api::v1::EventResult::Handled
        });

    harness.with_runtime(|runtime| {
        let cmd = WriteCommand;
        let mut ctx = CommandContext::new();
        ctx.set_buffer_id(buffer_id);
        ctx.set_vfs(Arc::clone(&mock_vfs) as Arc<dyn reovim_driver_vfs::VfsDriver>);
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());
    });

    assert!(
        event_received.load(Ordering::SeqCst),
        "BufferWillSave event should be emitted before write"
    );
}

#[test]
fn test_write_will_save_fires_before_saved() {
    use {
        reovim_driver_session::testing::TestSessionRuntime,
        reovim_driver_vfs::MockVfs,
        reovim_kernel::api::v1::events::kernel::{BufferSaved, BufferWillSave},
        std::sync::{Arc, Mutex},
    };

    let mut harness = TestSessionRuntime::with_buffer("order test");
    let buffer_id = harness.active_buffer().unwrap();
    harness
        .kernel()
        .buffers
        .get(buffer_id)
        .unwrap()
        .write()
        .set_file_path(Some("/tmp/order.txt".to_string()));

    let mock_vfs = Arc::new(MockVfs::new());

    // Track event ordering
    let order = Arc::new(Mutex::new(Vec::<&'static str>::new()));

    let order_will = Arc::clone(&order);
    let _sub1 = harness
        .kernel()
        .event_bus
        .subscribe::<BufferWillSave, _>(0, move |_| {
            order_will.lock().unwrap().push("will_save");
            reovim_kernel::api::v1::EventResult::Handled
        });

    let order_saved = Arc::clone(&order);
    let _sub2 = harness
        .kernel()
        .event_bus
        .subscribe::<BufferSaved, _>(0, move |_| {
            order_saved.lock().unwrap().push("saved");
            reovim_kernel::api::v1::EventResult::Handled
        });

    harness.with_runtime(|runtime| {
        let cmd = WriteCommand;
        let mut ctx = CommandContext::new();
        ctx.set_buffer_id(buffer_id);
        ctx.set_vfs(Arc::clone(&mock_vfs) as Arc<dyn reovim_driver_vfs::VfsDriver>);
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());
    });

    let events = order.lock().unwrap();
    assert_eq!(&*events, &["will_save", "saved"]);
    drop(events);
}

// ========================================================================
// WriteQuitCommand execution tests
// ========================================================================

#[test]
fn test_write_quit_propagates_write_error() {
    use reovim_driver_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    harness.with_runtime(|runtime| {
        let cmd = WriteQuitCommand;
        // StubExecutor returns None → execute_command returns "command not found"
        let ctx = CommandContext::new();
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
        // Should NOT signal quit when write fails
        let signals = runtime.take_signals();
        assert!(signals.is_empty());
    });
}
