use super::*;

// ========================================================================
// DetachCommand tests
// ========================================================================

#[test]
fn test_detach_command_id() {
    let cmd = DetachCommand;
    assert_eq!(cmd.id().name(), "detach");
    assert_eq!(cmd.id().module().as_str(), "commands");
}

#[test]
fn test_detach_command_names() {
    let cmd = DetachCommand;
    assert_eq!(cmd.names(), &["detach"]);
    assert_eq!(cmd.names().len(), 1);
}

#[test]
fn test_detach_command_description() {
    let cmd = DetachCommand;
    let desc = cmd.description();
    assert!(!desc.is_empty());
    assert!(desc.contains("Detach"));
}

#[test]
fn test_detach_command_complete_returns_empty() {
    let cmd = DetachCommand;
    let completions = cmd.complete("anything");
    assert!(completions.is_empty());
}

// ========================================================================
// ServersCommand tests
// ========================================================================

#[test]
fn test_servers_command_id() {
    let cmd = ServersCommand;
    assert_eq!(cmd.id().name(), "servers");
    assert_eq!(cmd.id().module().as_str(), "commands");
}

#[test]
fn test_servers_command_names() {
    let cmd = ServersCommand;
    assert_eq!(cmd.names(), &["servers"]);
    assert_eq!(cmd.names().len(), 1);
}

#[test]
fn test_servers_command_description() {
    let cmd = ServersCommand;
    let desc = cmd.description();
    assert!(!desc.is_empty());
    assert!(desc.contains("server"));
}

#[test]
fn test_servers_command_complete_returns_empty() {
    let cmd = ServersCommand;
    let completions = cmd.complete("");
    assert!(completions.is_empty());
}

// ========================================================================
// KillServerCommand tests
// ========================================================================

#[test]
fn test_kill_server_command_id() {
    let cmd = KillServerCommand;
    assert_eq!(cmd.id().name(), "kill-server");
    assert_eq!(cmd.id().module().as_str(), "commands");
}

#[test]
fn test_kill_server_command_names() {
    let cmd = KillServerCommand;
    let names = cmd.names();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"kill-server"));
    assert!(names.contains(&"killserver"));
}

#[test]
fn test_kill_server_command_description() {
    let cmd = KillServerCommand;
    let desc = cmd.description();
    assert!(!desc.is_empty());
    assert!(desc.contains("Kill"));
}

#[test]
fn test_kill_server_command_complete_returns_empty() {
    let cmd = KillServerCommand;
    let completions = cmd.complete("anything");
    assert!(completions.is_empty());
}

// ========================================================================
// command_handlers() collection tests
// ========================================================================

#[test]
fn test_command_handlers_collection_count() {
    let cmds = command_handlers();
    assert_eq!(cmds.len(), 3);
}

#[test]
fn test_command_handlers_collection_ids() {
    let cmds = command_handlers();
    let ids: Vec<_> = cmds.iter().map(|c| c.id().name_owned()).collect();
    assert!(ids.iter().any(|n| n == "detach"));
    assert!(ids.iter().any(|n| n == "servers"));
    assert!(ids.iter().any(|n| n == "kill-server"));
}

#[test]
fn test_command_handlers_all_have_names() {
    let cmds = command_handlers();
    for cmd in &cmds {
        assert!(!cmd.names().is_empty(), "command '{}' has no names", cmd.id().name());
    }
}

#[test]
fn test_command_handlers_all_have_descriptions() {
    let cmds = command_handlers();
    for cmd in &cmds {
        assert!(
            !cmd.description().is_empty(),
            "command '{}' has no description",
            cmd.id().name()
        );
    }
}

// ========================================================================
// Trait object safety tests
// ========================================================================

#[test]
fn test_session_commands_as_trait_objects() {
    let cmds: Vec<Box<dyn CommandHandler>> = vec![
        Box::new(DetachCommand),
        Box::new(ServersCommand),
        Box::new(KillServerCommand),
    ];
    assert_eq!(cmds.len(), 3);
    for cmd in &cmds {
        assert!(!cmd.id().name().is_empty());
    }
}

// ========================================================================
// Execute tests
// ========================================================================

#[test]
fn test_detach_execute_signals_quit() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    harness.with_runtime(|runtime| {
        let cmd = DetachCommand;
        let ctx = CommandContext::new();
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());
        let signals = runtime.take_signals();
        assert_eq!(signals.len(), 1);
        assert!(matches!(signals[0], RuntimeSignal::Quit));
    });
}

#[test]
fn test_servers_execute_returns_success() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    harness.with_runtime(|runtime| {
        let cmd = ServersCommand;
        let ctx = CommandContext::new();
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());
    });
}

#[test]
fn test_kill_server_execute_signals_quit() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    harness.with_runtime(|runtime| {
        let cmd = KillServerCommand;
        let ctx = CommandContext::new();
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());
        let signals = runtime.take_signals();
        assert_eq!(signals.len(), 1);
        assert!(matches!(signals[0], RuntimeSignal::Quit));
    });
}
