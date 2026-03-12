use super::*;

#[test]
fn test_quit_command_id() {
    let cmd = QuitCommand;
    assert_eq!(cmd.id().name(), "quit");
    assert_eq!(cmd.id().module().as_str(), "commands");
}

#[test]
fn test_quit_command_names() {
    let cmd = QuitCommand;
    let names = cmd.names();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"q"));
    assert!(names.contains(&"quit"));
}

#[test]
fn test_quit_command_description() {
    let cmd = QuitCommand;
    let desc = cmd.description();
    assert!(!desc.is_empty());
    assert!(desc.contains("Quit"));
    assert!(desc.contains("q!"));
}

#[test]
fn test_quit_command_complete_returns_empty() {
    let cmd = QuitCommand;
    let completions = cmd.complete("anything");
    assert!(completions.is_empty());
}

#[test]
fn test_quit_command_debug() {
    let cmd = QuitCommand;
    let debug = format!("{cmd:?}");
    assert!(debug.contains("QuitCommand"));
}

#[test]
fn test_quit_command_clone() {
    let cmd = QuitCommand;
    let cloned = cmd;
    assert_eq!(cmd.id(), cloned.id());
}

#[test]
fn test_quit_command_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<QuitCommand>();
}

// === Execution tests (unsaved buffer check) ===

#[test]
fn test_quit_unmodified_buffer_signals_quit() {
    use reovim_driver_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    harness.with_runtime(|runtime| {
        let cmd = QuitCommand;
        let ctx = CommandContext::new();
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());
        let signals = runtime.take_signals();
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0], RuntimeSignal::Quit);
    });
}

#[test]
fn test_quit_modified_buffer_returns_error() {
    use reovim_driver_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    // Mark the buffer as modified
    let buffer_id = harness.active_buffer().unwrap();
    harness
        .kernel()
        .buffers
        .get(buffer_id)
        .unwrap()
        .write()
        .set_modified(true);

    harness.with_runtime(|runtime| {
        let cmd = QuitCommand;
        let ctx = CommandContext::new();
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
        if let CommandResult::Error(msg) = &result {
            assert!(msg.contains("No write since last change"));
        }
        let signals = runtime.take_signals();
        assert!(signals.is_empty(), "should not signal quit when buffer is modified");
    });
}

#[test]
fn test_quit_bang_ignores_modified_buffer() {
    use {
        reovim_driver_command_types::ArgValue,
        reovim_driver_session::testing::TestSessionRuntime,
    };

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let buffer_id = harness.active_buffer().unwrap();
    harness
        .kernel()
        .buffers
        .get(buffer_id)
        .unwrap()
        .write()
        .set_modified(true);

    harness.with_runtime(|runtime| {
        let cmd = QuitCommand;
        let mut ctx = CommandContext::new();
        ctx.set("bang", ArgValue::Bang(true));
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());
        let signals = runtime.take_signals();
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0], RuntimeSignal::Quit);
    });
}

#[test]
fn test_quit_no_active_buffer_signals_quit() {
    use reovim_driver_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    harness.with_runtime(|runtime| {
        let cmd = QuitCommand;
        let ctx = CommandContext::new();
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());
        let signals = runtime.take_signals();
        assert_eq!(signals.len(), 1);
    });
}
