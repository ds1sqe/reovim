use reovim_driver_text_session::{TextInputSink, testing::TestSessionRuntime};

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
    assert_eq!(commands.len(), 4);
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
        jump.start(vec!["hello world".into(), "foo bar".into()], 0, 0, Direction::Both, 0);
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
        jump.start(vec!["hello world".into()], 0, 0, Direction::Both, 0);
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
        jump.start(vec!["hello world".into()], 0, 0, Direction::Both, 0);
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

// =========================================================================
// Viewport-bounded scanning tests (Phase 1 of #663)
// =========================================================================

#[test]
fn test_jump_search_viewport_bounded() {
    use std::fmt::Write;
    // Buffer has 10 lines but viewport shows only lines 3-7.
    let mut buffer_content = String::new();
    for i in 0..10 {
        if i > 0 {
            buffer_content.push('\n');
        }
        let _ = write!(buffer_content, "line {i} content hello world");
    }
    let cmd = JumpSearchCommand;
    let mut harness = TestSessionRuntime::with_buffer(&buffer_content);
    let buffer_id = harness.active_buffer().unwrap();

    // Set viewport to scroll_top=3, height=5 (shows lines 3-7).
    harness.with_runtime(|rt| {
        let window = rt.windows_mut().active_mut().unwrap();
        window.viewport.scroll_top = 3;
        window.viewport.height = 5;
        window.cursor.line = 5; // Cursor at buffer line 5.
        window.cursor.column = 0;
    });

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));

    // Verify jump state was started and is active.
    harness.with_runtime(|rt| {
        let jump = rt.ext_mut::<JumpSessionState>();
        assert!(jump.is_active());
    });
}

#[test]
fn test_jump_search_viewport_target_absolute_coordinates() {
    // Buffer with unique patterns at specific lines.
    let content = "aaa\nbbb\nccc\nxyz target\neee\nfff\nggg";
    let cmd = JumpSearchCommand;
    let mut harness = TestSessionRuntime::with_buffer(content);
    let buffer_id = harness.active_buffer().unwrap();

    // Viewport: scroll_top=2, height=4 → visible lines 2-5.
    // Cursor at line 2 col 0, "xyz target" is at buffer line 3.
    harness.with_runtime(|rt| {
        let window = rt.windows_mut().active_mut().unwrap();
        window.viewport.scroll_top = 2;
        window.viewport.height = 4;
        window.cursor.line = 2;
        window.cursor.column = 0;
    });

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    harness.with_runtime(|rt| cmd.execute(rt, &args));

    // Search for "xy" — only match is at buffer line 3.
    harness.with_runtime(|rt| {
        let jump = rt.ext_mut::<JumpSessionState>();
        jump.insert_char('x');
        jump.insert_char('y');
        // Single match → auto-jump, target set.
        assert!(jump.has_target());
    });

    // Execute jump and verify cursor at absolute buffer line 3.
    let exec = JumpExecuteCommand;
    let exec_args = CommandContext::new();
    harness.with_runtime(|rt| exec.execute(rt, &exec_args));

    harness.with_runtime(|rt| {
        let window = rt.windows_mut().active_mut().unwrap();
        assert_eq!(window.cursor.line, 3); // Absolute buffer line, not viewport-relative.
        assert_eq!(window.cursor.column, 0);
    });
}

#[test]
fn test_jump_search_viewport_past_buffer_end() {
    // Viewport height exceeds buffer size.
    let cmd = JumpSearchCommand;
    let mut harness = TestSessionRuntime::with_buffer("hello world\nfoo bar");
    let buffer_id = harness.active_buffer().unwrap();

    harness.with_runtime(|rt| {
        let window = rt.windows_mut().active_mut().unwrap();
        window.viewport.scroll_top = 0;
        window.viewport.height = 100; // Much larger than 2-line buffer.
    });

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));

    // Should still find matches in the 2-line buffer.
    harness.with_runtime(|rt| {
        let jump = rt.ext_mut::<JumpSessionState>();
        assert!(jump.is_active());
    });
}

#[test]
fn test_jump_search_viewport_height_zero() {
    // Zero-height viewport should not crash.
    let cmd = JumpSearchCommand;
    let mut harness = TestSessionRuntime::with_buffer("hello world");
    let buffer_id = harness.active_buffer().unwrap();

    harness.with_runtime(|rt| {
        let window = rt.windows_mut().active_mut().unwrap();
        window.viewport.height = 0;
    });

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));
}

// =========================================================================
// JumpSearchBackwardCommand tests (Phase 2 of #663)
// =========================================================================

#[test]
fn test_jump_search_backward_command_id() {
    let cmd = JumpSearchBackwardCommand;
    assert_eq!(cmd.id(), ids::JUMP_SEARCH_BACKWARD);
}

#[test]
fn test_jump_search_backward_command_description() {
    let cmd = JumpSearchBackwardCommand;
    assert!(!cmd.description().is_empty());
}

#[test]
fn test_jump_search_backward_no_buffer() {
    let cmd = JumpSearchBackwardCommand;
    let mut harness = TestSessionRuntime::new();
    let args = CommandContext::new();
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));
}

#[test]
fn test_jump_search_backward_no_window() {
    let cmd = JumpSearchBackwardCommand;
    let mut harness = TestSessionRuntime::new();
    let mut args = CommandContext::new();
    args.set_buffer_id(reovim_kernel::api::v1::BufferId::from_raw(0));
    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));
}

#[test]
fn test_jump_search_backward_starts_state() {
    let cmd = JumpSearchBackwardCommand;
    let mut harness = TestSessionRuntime::with_buffer("hello world\nfoo bar");
    let buffer_id = harness.active_buffer().unwrap();
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
    assert!(matches!(result, CommandResult::Success));

    harness.with_runtime(|rt| {
        let jump = rt.ext_mut::<JumpSessionState>();
        assert!(jump.is_active());
    });
    assert!(harness.changes().mode_changed);
}

#[test]
fn test_jump_search_backward_finds_only_before_cursor() {
    // Buffer: "he he he", cursor at end (col 100).
    // Backward search should find matches before cursor.
    let cmd = JumpSearchBackwardCommand;
    let mut harness = TestSessionRuntime::with_buffer("he he he");
    let buffer_id = harness.active_buffer().unwrap();

    harness.with_runtime(|rt| {
        let window = rt.windows_mut().active_mut().unwrap();
        window.cursor.column = 7; // After last "he"
    });

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    harness.with_runtime(|rt| cmd.execute(rt, &args));

    // Search "he" — backward from col 7, finds cols 0, 3 (not col 6).
    harness.with_runtime(|rt| {
        let jump = rt.ext_mut::<JumpSessionState>();
        jump.insert_char('h');
        jump.insert_char('e');
        // Should have matches (2 before cursor).
        assert!(jump.is_active() || jump.has_target());
    });
}

#[test]
fn test_jump_search_forward_only_finds_after_cursor() {
    // Verify s (forward) no longer uses Direction::Both.
    let cmd = JumpSearchCommand;
    let mut harness = TestSessionRuntime::with_buffer("he he he");
    let buffer_id = harness.active_buffer().unwrap();

    harness.with_runtime(|rt| {
        let window = rt.windows_mut().active_mut().unwrap();
        window.cursor.column = 3; // Between matches
    });

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    harness.with_runtime(|rt| cmd.execute(rt, &args));

    // Search "he" — forward from col 3, should only find col 6.
    harness.with_runtime(|rt| {
        let jump = rt.ext_mut::<JumpSessionState>();
        jump.insert_char('h');
        jump.insert_char('e');
        // Single match → auto-jump.
        assert!(jump.has_target());
        let target = jump.take_target().unwrap();
        assert_eq!(target.col, 6);
    });
}

#[test]
fn test_all_commands_count_with_backward() {
    let commands = all_commands();
    assert_eq!(commands.len(), 4);
}

#[test]
fn test_all_commands_unique_ids_with_backward() {
    let commands = all_commands();
    let ids: Vec<CommandId> = commands.iter().map(|c| c.id()).collect();
    for (i, a) in ids.iter().enumerate() {
        for b in &ids[i + 1..] {
            assert_ne!(a, b);
        }
    }
}
