use reovim_driver_session::{TextInputSink, testing::TestSessionRuntime};

use super::*;

#[test]
fn test_jump_search_command_id() {
    let cmd = JumpSearchCommand;
    assert_eq!(cmd.id(), ids::JUMP_SEARCH);
}

#[test]
fn test_jump_search_command_description() {
    let cmd = JumpSearchCommand;
    assert!(!cmd.description().is_empty());
}

#[test]
fn test_jump_execute_command_id() {
    let cmd = JumpExecuteCommand;
    assert_eq!(cmd.id(), ids::JUMP_EXECUTE);
}

#[test]
fn test_jump_execute_command_description() {
    let cmd = JumpExecuteCommand;
    assert!(!cmd.description().is_empty());
}

#[test]
fn test_start_find_char_jump_command_id() {
    let cmd = StartFindCharJumpCommand;
    assert_eq!(cmd.id(), ids::START_FIND_CHAR_JUMP);
}

#[test]
fn test_start_find_char_jump_command_description() {
    let cmd = StartFindCharJumpCommand;
    assert!(!cmd.description().is_empty());
}

#[test]
fn test_all_commands_count() {
    let commands = all_commands();
    assert_eq!(commands.len(), 3);
}

#[test]
fn test_all_commands_unique_ids() {
    let commands = all_commands();
    let ids: Vec<CommandId> = commands.iter().map(|c| c.id()).collect();
    assert_ne!(ids[0], ids[1]);
    assert_ne!(ids[0], ids[2]);
    assert_ne!(ids[1], ids[2]);
}

#[test]
fn test_jump_search_command_no_args() {
    let cmd = JumpSearchCommand;
    assert!(cmd.args().is_empty());
}

#[test]
fn test_jump_search_command_no_names() {
    let cmd = JumpSearchCommand;
    assert!(cmd.names().is_empty());
}

#[test]
fn test_jump_search_no_buffer() {
    let cmd = JumpSearchCommand;
    let mut harness = TestSessionRuntime::new();
    let args = CommandContext::new();
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));
}

#[test]
fn test_jump_search_no_window() {
    let cmd = JumpSearchCommand;
    let mut harness = TestSessionRuntime::new();
    let mut args = CommandContext::new();
    args.set_buffer_id(reovim_kernel::api::v1::BufferId::from_raw(0));
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));
}

#[test]
fn test_jump_search_starts_state_and_pushes_mode() {
    let cmd = JumpSearchCommand;
    let mut harness = TestSessionRuntime::with_buffer("hello world\nfoo bar");
    let buffer_id = harness.active_buffer().unwrap();
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));

    // Verify jump state was started with buffer lines.
    harness.with_runtime(|rt| {
        let jump = rt.ext_mut::<JumpSessionState>();
        assert!(jump.is_active());
    });

    // Verify mode was pushed.
    assert!(harness.changes().mode_changed);
}

#[test]
fn test_jump_execute_no_target() {
    let cmd = JumpExecuteCommand;
    let mut harness = TestSessionRuntime::with_buffer("hello world");
    let args = CommandContext::new();
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));
}

#[test]
fn test_jump_execute_moves_cursor() {
    let cmd = JumpExecuteCommand;
    let mut harness = TestSessionRuntime::with_buffer("hello world\nfoo bar");
    let args = CommandContext::new();

    // Set up a jump state with a resolved target via "wo" search.
    harness.with_runtime(|rt| {
        let jump = rt.ext_mut::<JumpSessionState>();
        jump.start(vec!["hello world".into(), "foo bar".into()], 0, 0, Direction::Both);
        jump.insert_char('w');
        jump.insert_char('o');
        assert!(jump.has_target());
    });

    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));

    // Verify cursor was moved.
    harness.with_runtime(|rt| {
        let window = rt.windows_mut().active_mut().expect("active window");
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 6);
    });
}

#[test]
fn test_jump_execute_no_window() {
    let cmd = JumpExecuteCommand;
    let mut harness = TestSessionRuntime::new();
    let args = CommandContext::new();

    // Set up a target without any window.
    harness.with_runtime(|rt| {
        let jump = rt.ext_mut::<JumpSessionState>();
        jump.start(vec!["hello world".into()], 0, 0, Direction::Both);
        jump.insert_char('w');
        jump.insert_char('o');
        assert!(jump.has_target());
    });

    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));
}

// =========================================================================
// StartFindCharJumpCommand execute tests
// =========================================================================

#[test]
fn test_start_find_char_jump_no_args() {
    let cmd = StartFindCharJumpCommand;
    let mut harness = TestSessionRuntime::with_buffer("hello");
    let args = CommandContext::new();
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Error(_)));
}

#[test]
fn test_start_find_char_jump_invalid_json() {
    let cmd = StartFindCharJumpCommand;
    let mut harness = TestSessionRuntime::with_buffer("hello");
    let mut args = CommandContext::new();
    args.set(
        "find_char_matches",
        reovim_driver_command::ArgValue::String("not json".to_string()),
    );
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Error(_)));
}

#[test]
fn test_start_find_char_jump_too_few_matches() {
    let cmd = StartFindCharJumpCommand;
    let mut harness = TestSessionRuntime::with_buffer("hello");
    let mut args = CommandContext::new();
    args.set(
        "find_char_matches",
        reovim_driver_command::ArgValue::String(r#"[{"line":0,"col":5}]"#.to_string()),
    );
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Error(_)));
}

#[test]
fn test_start_find_char_jump_activates_state() {
    let cmd = StartFindCharJumpCommand;
    let mut harness = TestSessionRuntime::with_buffer("aabaa");
    let mut args = CommandContext::new();
    args.set(
        "find_char_matches",
        reovim_driver_command::ArgValue::String(
            r#"[{"line":0,"col":0},{"line":0,"col":2},{"line":0,"col":4}]"#.to_string(),
        ),
    );

    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));

    // Verify jump state was activated with 3 matches.
    harness.with_runtime(|rt| {
        let jump = rt.ext_mut::<JumpSessionState>();
        assert!(jump.is_active());
        let m = jump.get_matches().expect("should have matches");
        assert_eq!(m.len(), 3);
    });

    // Verify mode was pushed.
    assert!(harness.changes().mode_changed);
}

#[test]
fn test_start_find_char_jump_label_selection_moves_cursor() {
    let cmd = StartFindCharJumpCommand;
    let mut harness = TestSessionRuntime::with_buffer("aabaa");
    let mut args = CommandContext::new();
    args.set(
        "find_char_matches",
        reovim_driver_command::ArgValue::String(
            r#"[{"line":0,"col":0},{"line":0,"col":2},{"line":0,"col":4}]"#.to_string(),
        ),
    );

    harness.with_runtime(|rt| cmd.execute(rt, &args));

    // Select second label ("f") to jump to col 2
    harness.with_runtime(|rt| {
        let jump = rt.ext_mut::<JumpSessionState>();
        jump.insert_char('f');
        assert!(!jump.is_active());
        assert!(jump.has_target());
    });

    // Execute the jump
    let exec_cmd = JumpExecuteCommand;
    let exec_args = CommandContext::new();
    harness.with_runtime(|rt| exec_cmd.execute(rt, &exec_args));

    // Verify cursor position
    harness.with_runtime(|rt| {
        let window = rt.windows_mut().active_mut().expect("active window");
        assert_eq!(window.cursor.column, 2);
    });
}

// =========================================================================
// JumpExecuteCommand additional tests
// =========================================================================

#[test]
fn test_jump_execute_consumes_target() {
    let cmd = JumpExecuteCommand;
    let mut harness = TestSessionRuntime::with_buffer("hello world");
    let args = CommandContext::new();

    // Set up a target.
    harness.with_runtime(|rt| {
        let jump = rt.ext_mut::<JumpSessionState>();
        jump.start(vec!["hello world".into()], 0, 0, Direction::Both);
        jump.insert_char('w');
        jump.insert_char('o');
        assert!(jump.has_target());
    });

    // Execute once.
    harness.with_runtime(|rt| cmd.execute(rt, &args));

    // Target should be consumed (take_target returns None).
    harness.with_runtime(|rt| {
        let jump = rt.ext_mut::<JumpSessionState>();
        assert!(!jump.has_target());
    });
}
