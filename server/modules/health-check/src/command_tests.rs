use {
    super::*,
    reovim_driver_command::Command,
    reovim_driver_text_session::{BufferApi, testing::TestSessionRuntime},
    reovim_subsys_command_types::CommandContext,
};

// ========================================================================
// Command metadata
// ========================================================================

#[test]
fn test_command_id() {
    let cmd = CheckHealthCommand::new();
    assert_eq!(cmd.id().module().as_str(), "health-check");
    assert_eq!(cmd.id().name(), "checkhealth");
}

#[test]
fn test_command_description() {
    let cmd = CheckHealthCommand::new();
    assert!(!cmd.description().is_empty());
}

#[test]
fn test_command_names() {
    let cmd = CheckHealthCommand::new();
    assert_eq!(cmd.names(), &["checkhealth"]);
}

#[test]
fn test_command_no_args() {
    let cmd = CheckHealthCommand::new();
    assert!(cmd.args().is_empty());
}

#[test]
fn test_command_default() {
    fn create_default<T: Default>() -> T {
        T::default()
    }
    let cmd: CheckHealthCommand = create_default();
    assert_eq!(cmd.id().name(), "checkhealth");
}

// ========================================================================
// Command execution
// ========================================================================

#[test]
fn test_execute_creates_buffer() {
    let mut test = TestSessionRuntime::new();
    let args = CommandContext::new();

    let cmd = CheckHealthCommand::new();
    let result = test.with_runtime(|runtime| cmd.execute(runtime, &args));
    assert!(result.is_success());
}

#[test]
fn test_execute_sets_active_buffer() {
    let mut test = TestSessionRuntime::new();
    let args = CommandContext::new();

    let cmd = CheckHealthCommand::new();
    test.with_runtime(|runtime| {
        cmd.execute(runtime, &args);
        // After execution, active buffer should be set
        assert!(runtime.active_buffer().is_some());
    });
}

#[test]
fn test_execute_buffer_not_modified() {
    let mut test = TestSessionRuntime::new();
    let args = CommandContext::new();

    let cmd = CheckHealthCommand::new();
    test.with_runtime(|runtime| {
        cmd.execute(runtime, &args);
        let buf_id = runtime.active_buffer().unwrap();
        assert_eq!(runtime.is_buffer_modified(buf_id), Some(false));
    });
}

#[test]
fn test_execute_buffer_has_content() {
    let mut test = TestSessionRuntime::new();
    let args = CommandContext::new();

    let cmd = CheckHealthCommand::new();
    test.with_runtime(|runtime| {
        cmd.execute(runtime, &args);
        let buf_id = runtime.active_buffer().unwrap();
        let content = runtime.buffer_content(buf_id).unwrap();
        assert!(content.contains("=== System ==="));
        assert!(content.contains("reovim version"));
        assert!(content.contains("API version"));
    });
}

#[test]
fn test_execute_always_succeeds() {
    let mut test = TestSessionRuntime::new();
    let args = CommandContext::new();

    let cmd = CheckHealthCommand::new();
    let result = test.with_runtime(|runtime| cmd.execute(runtime, &args));
    assert!(result.is_success());
}
