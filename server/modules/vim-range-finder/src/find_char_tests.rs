use super::*;

// ========================================================================
// find_all_char_matches tests
// ========================================================================

#[test]
fn test_find_all_forward_multiple() {
    let matches = find_all_char_matches("ababa", 0, 0, 'a', true);
    assert_eq!(matches, vec![(0, 2), (0, 4)]);
}

#[test]
fn test_find_all_forward_single() {
    let matches = find_all_char_matches("abcde", 0, 0, 'c', true);
    assert_eq!(matches, vec![(0, 2)]);
}

#[test]
fn test_find_all_forward_none() {
    let matches = find_all_char_matches("abcde", 0, 0, 'z', true);
    assert!(matches.is_empty());
}

#[test]
fn test_find_all_backward_multiple() {
    let matches = find_all_char_matches("ababa", 4, 0, 'a', false);
    assert_eq!(matches, vec![(0, 2), (0, 0)]);
}

#[test]
fn test_find_all_backward_none() {
    let matches = find_all_char_matches("abcde", 0, 0, 'a', false);
    assert!(matches.is_empty());
}

#[test]
fn test_find_all_preserves_line() {
    let matches = find_all_char_matches("aa", 0, 5, 'a', true);
    assert_eq!(matches, vec![(5, 1)]);
}

#[test]
fn test_find_all_empty_line() {
    let matches = find_all_char_matches("", 0, 0, 'a', true);
    assert!(matches.is_empty());
}

// ========================================================================
// Command metadata tests
// ========================================================================

#[test]
fn test_command_id() {
    let cmd = EnhancedFindCharCommand;
    assert_eq!(cmd.id().module().as_str(), "vim");
    assert_eq!(cmd.id().name(), "execute-find-char");
}

#[test]
fn test_command_description() {
    let cmd = EnhancedFindCharCommand;
    assert!(!cmd.description().is_empty());
}

#[test]
fn test_command_priority_is_override() {
    let cmd = EnhancedFindCharCommand;
    assert_eq!(cmd.priority(), CommandPriority::Override);
}

// ========================================================================
// Execute tests using TestSessionRuntime
// ========================================================================

use reovim_driver_session::{TextInputSink, testing::TestSessionRuntime};

#[test]
fn test_execute_no_find_char_arg() {
    let cmd = EnhancedFindCharCommand;
    let mut harness = TestSessionRuntime::new();
    let args = CommandContext::new();
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Error(_)));
}

#[test]
fn test_execute_no_buffer() {
    let cmd = EnhancedFindCharCommand;
    let mut harness = TestSessionRuntime::new();
    let mut args = CommandContext::new();
    args.set("find_char", ArgValue::Char('x'));
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Error(_)));
}

#[test]
fn test_execute_forward_single_match() {
    let cmd = EnhancedFindCharCommand;
    let mut harness = TestSessionRuntime::with_buffer("hello world");
    let buffer_id = harness.active_buffer().unwrap();
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("find_char", ArgValue::Char('w'));

    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert_eq!(result, CommandResult::Success);

    harness.with_runtime(|rt| {
        let window = rt.windows().active().unwrap();
        assert_eq!(window.cursor.column, 6);
    });
}

#[test]
fn test_execute_forward_multi_match_activates_jump() {
    let cmd = EnhancedFindCharCommand;
    let mut harness = TestSessionRuntime::with_buffer("ababa");
    let buffer_id = harness.active_buffer().unwrap();
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("find_char", ArgValue::Char('a'));

    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert_eq!(result, CommandResult::Success);

    // Jump state should be active with 2 matches
    harness.with_runtime(|rt| {
        let jump = rt.ext_mut::<JumpSessionState>();
        assert!(jump.is_active());
        let m = jump.get_matches().unwrap();
        assert_eq!(m.len(), 2);
    });

    // Mode should have been pushed
    assert!(harness.changes().mode_changed);
}

#[test]
fn test_execute_multi_match_with_count_uses_motion() {
    let cmd = EnhancedFindCharCommand;
    let mut harness = TestSessionRuntime::with_buffer("ababa");
    let buffer_id = harness.active_buffer().unwrap();
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("find_char", ArgValue::Char('a'));
    args.set("count", ArgValue::Count(2));

    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert_eq!(result, CommandResult::Success);

    // With count=2, should use direct motion to second 'a' at col 4
    harness.with_runtime(|rt| {
        let window = rt.windows().active().unwrap();
        assert_eq!(window.cursor.column, 4);
    });
}

#[test]
fn test_execute_not_found() {
    let cmd = EnhancedFindCharCommand;
    let mut harness = TestSessionRuntime::with_buffer("hello");
    let buffer_id = harness.active_buffer().unwrap();
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("find_char", ArgValue::Char('z'));

    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert_eq!(result, CommandResult::Success);

    // Cursor doesn't move
    harness.with_runtime(|rt| {
        let window = rt.windows().active().unwrap();
        assert_eq!(window.cursor.column, 0);
    });
}

#[test]
fn test_execute_backward() {
    let cmd = EnhancedFindCharCommand;
    // Use string with single 'h' to test basic backward motion
    let mut harness = TestSessionRuntime::with_buffer("hello world");
    let buffer_id = harness.active_buffer().unwrap();

    harness.with_runtime(|rt| {
        let window = rt.windows_mut().active_mut().unwrap();
        window.cursor.column = 5;
    });

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("find_char", ArgValue::Char('h'));
    args.set("find_direction", ArgValue::String("backward".to_string()));

    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert_eq!(result, CommandResult::Success);

    harness.with_runtime(|rt| {
        let window = rt.windows().active().unwrap();
        assert_eq!(window.cursor.column, 0); // 'h' at col 0
    });
}

#[test]
fn test_execute_till_mode() {
    let cmd = EnhancedFindCharCommand;
    // Use 'w' (single match) to test till mode
    let mut harness = TestSessionRuntime::with_buffer("hello world");
    let buffer_id = harness.active_buffer().unwrap();
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("find_char", ArgValue::Char('w'));
    args.set("find_inclusive", ArgValue::Bool(false));

    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert_eq!(result, CommandResult::Success);

    harness.with_runtime(|rt| {
        let window = rt.windows().active().unwrap();
        assert_eq!(window.cursor.column, 5); // One before 'w' at col 6
    });
}

#[test]
fn test_execute_empty_buffer() {
    let cmd = EnhancedFindCharCommand;
    let mut harness = TestSessionRuntime::with_buffer("");
    let buffer_id = harness.active_buffer().unwrap();
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("find_char", ArgValue::Char('x'));

    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert_eq!(result, CommandResult::Success);

    harness.with_runtime(|rt| {
        let window = rt.windows().active().unwrap();
        assert_eq!(window.cursor.column, 0);
    });
}

#[test]
fn test_execute_no_active_window() {
    let cmd = EnhancedFindCharCommand;
    let mut harness = TestSessionRuntime::new();
    let mut args = CommandContext::new();
    args.set_buffer_id(reovim_kernel::api::v1::BufferId::new());
    args.set("find_char", ArgValue::Char('x'));
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Error(_)));
}

#[test]
fn test_execute_buffer_not_found() {
    let cmd = EnhancedFindCharCommand;
    let mut harness = TestSessionRuntime::with_buffer("hello");
    let mut args = CommandContext::new();
    // Use a fresh BufferId that doesn't exist in the kernel
    args.set_buffer_id(reovim_kernel::api::v1::BufferId::new());
    args.set("find_char", ArgValue::Char('h'));
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Error(_)));
}

#[test]
fn test_execute_buffer_not_found_counted() {
    let cmd = EnhancedFindCharCommand;
    let mut harness = TestSessionRuntime::with_buffer("hello");
    let mut args = CommandContext::new();
    args.set_buffer_id(reovim_kernel::api::v1::BufferId::new());
    args.set("find_char", ArgValue::Char('h'));
    args.set("count", ArgValue::Count(2));
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Error(_)));
}

#[test]
fn test_execute_multi_match_label_selection_moves_cursor() {
    let cmd = EnhancedFindCharCommand;
    let mut harness = TestSessionRuntime::with_buffer("ababa");
    let buffer_id = harness.active_buffer().unwrap();
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("find_char", ArgValue::Char('a'));

    // Start enhanced find-char, should activate jump labels
    harness.with_runtime(|rt| cmd.execute(rt, &args));

    // Select second label ('f') to jump to col 4
    harness.with_runtime(|rt| {
        let jump = rt.ext_mut::<JumpSessionState>();
        jump.insert_char('f');
        assert!(jump.has_target());
    });

    // Execute jump
    let exec_cmd = reovim_module_range_finder::jump::command::JumpExecuteCommand;
    let exec_args = CommandContext::new();
    harness.with_runtime(|rt| exec_cmd.execute(rt, &exec_args));

    harness.with_runtime(|rt| {
        let window = rt.windows().active().unwrap();
        assert_eq!(window.cursor.column, 4);
    });
}
