use super::*;

#[test]
fn test_edit_command_id() {
    let cmd = EditCommand;
    assert_eq!(cmd.id().name(), "edit");
    assert_eq!(cmd.id().module().as_str(), "commands");
}

#[test]
fn test_edit_command_names() {
    let cmd = EditCommand;
    let names = cmd.names();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"e"));
    assert!(names.contains(&"edit"));
}

#[test]
fn test_edit_command_description() {
    let cmd = EditCommand;
    let desc = cmd.description();
    assert!(!desc.is_empty());
    assert!(desc.contains("Edit"));
}

#[test]
fn test_edit_command_args() {
    let cmd = EditCommand;
    let args = cmd.args();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].name, "file");
    assert_eq!(args[0].kind, ArgKind::Rest);
    assert!(!args[0].required);
}

#[test]
fn test_edit_command_complete_returns_empty() {
    let cmd = EditCommand;
    let completions = cmd.complete("some_path");
    assert!(completions.is_empty());
}

#[test]
fn test_edit_command_debug() {
    let cmd = EditCommand;
    let debug = format!("{cmd:?}");
    assert!(debug.contains("EditCommand"));
}

#[test]
fn test_edit_command_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<EditCommand>();
}

#[test]
fn test_edit_command_clone_copy() {
    let cmd = EditCommand;
    let copy = cmd;
    assert_eq!(cmd.id(), copy.id());
}

// ========================================================================
// Execute tests (using TestSessionRuntime + MockVfs)
// ========================================================================

#[test]
fn test_edit_execute_no_filename() {
    use reovim_driver_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    harness.with_runtime(|runtime| {
        let cmd = EditCommand;
        let ctx = CommandContext::new();
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
    });
}

#[test]
fn test_edit_execute_no_buffer_id() {
    use reovim_driver_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    harness.with_runtime(|runtime| {
        let cmd = EditCommand;
        let mut ctx = CommandContext::new();
        ctx.set("file", reovim_driver_command::ArgValue::String("test.rs".to_string()));
        // No buffer_id set
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
    });
}

#[test]
fn test_edit_execute_no_vfs() {
    use reovim_driver_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let buffer_id = harness.active_buffer().unwrap();
    harness.with_runtime(|runtime| {
        let cmd = EditCommand;
        let mut ctx = CommandContext::new();
        ctx.set("file", reovim_driver_command::ArgValue::String("test.rs".to_string()));
        ctx.set_buffer_id(buffer_id);
        // No VFS set
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
    });
}

#[test]
fn test_edit_execute_file_not_found() {
    use {
        reovim_driver_session::testing::TestSessionRuntime, reovim_driver_vfs::MockVfs,
        std::sync::Arc,
    };

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let buffer_id = harness.active_buffer().unwrap();
    let mock_vfs = Arc::new(MockVfs::new());
    // No files added → read will fail

    harness.with_runtime(|runtime| {
        let cmd = EditCommand;
        let mut ctx = CommandContext::new();
        ctx.set("file", reovim_driver_command::ArgValue::String("nonexistent.rs".to_string()));
        ctx.set_buffer_id(buffer_id);
        ctx.set_vfs(Arc::clone(&mock_vfs) as Arc<dyn reovim_driver_vfs::VfsDriver>);
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
    });
}

#[test]
fn test_edit_execute_success() {
    use {
        reovim_driver_session::testing::TestSessionRuntime, reovim_driver_vfs::MockVfs,
        std::sync::Arc,
    };

    let mut harness = TestSessionRuntime::with_buffer("original");
    let buffer_id = harness.active_buffer().unwrap();
    let mock_vfs = Arc::new(MockVfs::new());
    mock_vfs.add_file_str("/tmp/test.rs", "fn main() {}");

    harness.with_runtime(|runtime| {
        let cmd = EditCommand;
        let mut ctx = CommandContext::new();
        ctx.set("file", reovim_driver_command::ArgValue::String("/tmp/test.rs".to_string()));
        ctx.set_buffer_id(buffer_id);
        ctx.set_vfs(Arc::clone(&mock_vfs) as Arc<dyn reovim_driver_vfs::VfsDriver>);
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());
    });
}

#[test]
fn test_edit_execute_invalid_utf8() {
    use {
        reovim_driver_session::testing::TestSessionRuntime, reovim_driver_vfs::MockVfs,
        std::sync::Arc,
    };

    let mut harness = TestSessionRuntime::with_buffer("original");
    let buffer_id = harness.active_buffer().unwrap();
    let mock_vfs = Arc::new(MockVfs::new());
    mock_vfs.add_file("/tmp/binary.bin", [0xFF, 0xFE, 0x80, 0x90]);

    harness.with_runtime(|runtime| {
        let cmd = EditCommand;
        let mut ctx = CommandContext::new();
        ctx.set("file", reovim_driver_command::ArgValue::String("/tmp/binary.bin".to_string()));
        ctx.set_buffer_id(buffer_id);
        ctx.set_vfs(Arc::clone(&mock_vfs) as Arc<dyn reovim_driver_vfs::VfsDriver>);
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
    });
}
