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

use reovim_driver_text_session::{TextInputSink, testing::TestSessionRuntime};

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
    let exec_cmd = crate::jump::command::JumpExecuteCommand;
    let exec_args = CommandContext::new();
    harness.with_runtime(|rt| exec_cmd.execute(rt, &exec_args));

    harness.with_runtime(|rt| {
        let window = rt.windows().active().unwrap();
        assert_eq!(window.cursor.column, 4);
    });
}

// ========================================================================
// Bug reproduction: ; repeat re-enters label mode after label-selected f
// (#663)
// ========================================================================

/// After `fX` with multi-match labels -> select label -> cursor at first X,
/// a subsequent call to `EnhancedFindCharCommand` (what `;` does internally)
/// should advance to the next X — not re-enter label mode.
///
/// Current bug: the command has no way to distinguish an initial `f` from a
/// `;` repeat, so it always checks for multi-match and shows labels again.
#[test]
fn test_repeat_after_label_selection_should_advance_not_relabel() {
    let cmd = EnhancedFindCharCommand;
    // Buffer: "aXaXaXa" — X at cols 1, 3, 5
    let mut harness = TestSessionRuntime::with_buffer("aXaXaXa");
    let buffer_id = harness.active_buffer().unwrap();

    // Step 1: Initial fX — 3 matches, activates labels
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("find_char", ArgValue::Char('X'));

    harness.with_runtime(|rt| cmd.execute(rt, &args));

    harness.with_runtime(|rt| {
        let jump = rt.ext_mut::<JumpSessionState>();
        assert!(jump.is_active(), "should show labels for 3 X matches");
        assert_eq!(jump.get_matches().unwrap().len(), 3);
    });

    // Step 2: Select first label -> cursor moves to col 1 (first X)
    harness.with_runtime(|rt| {
        let jump = rt.ext_mut::<JumpSessionState>();
        jump.insert_char('s'); // first home-row label
        assert!(jump.has_target());
    });

    let exec_cmd = crate::jump::command::JumpExecuteCommand;
    harness.with_runtime(|rt| exec_cmd.execute(rt, &CommandContext::new()));

    harness.with_runtime(|rt| {
        let window = rt.windows().active().unwrap();
        assert_eq!(window.cursor.column, 1, "should be at first X");
    });

    // Step 3: Simulate ; repeat — call EnhancedFindCharCommand again from col 1.
    // From col 1, there are 2 X's ahead (col 3, 5) — still multi-match.
    // RepeatFindSame sets is_repeat=true so labels are bypassed.
    let mut repeat_args = CommandContext::new();
    repeat_args.set_buffer_id(buffer_id);
    repeat_args.set("find_char", ArgValue::Char('X'));
    repeat_args.set("is_repeat", ArgValue::Bool(true));

    harness.with_runtime(|rt| cmd.execute(rt, &repeat_args));
    harness.with_runtime(|rt| {
        let jump = rt.ext_mut::<JumpSessionState>();
        assert!(
            !jump.is_active(),
            "repeat should advance to next match, not re-enter label mode"
        );

        let window = rt.windows().active().unwrap();
        assert_eq!(window.cursor.column, 3, "repeat should advance cursor to next X at col 3");
    });
}

/// After `FX` backward with labels -> select label -> cursor at last X,
/// a subsequent backward call (what `;` does after `FX`) should advance
/// backward to the previous X — not re-enter label mode.
#[test]
fn test_reverse_repeat_after_label_selection_should_advance_backward() {
    let cmd = EnhancedFindCharCommand;
    // Buffer: "aXaXaXa" — X at cols 1, 3, 5
    let mut harness = TestSessionRuntime::with_buffer("aXaXaXa");
    let buffer_id = harness.active_buffer().unwrap();

    // Position cursor at end (col 6)
    harness.with_runtime(|rt| {
        let window = rt.windows_mut().active_mut().unwrap();
        window.cursor.column = 6;
    });

    // Step 1: FX backward — 3 matches, activates labels
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("find_char", ArgValue::Char('X'));
    args.set("find_direction", ArgValue::String("backward".to_string()));

    harness.with_runtime(|rt| cmd.execute(rt, &args));

    harness.with_runtime(|rt| {
        let jump = rt.ext_mut::<JumpSessionState>();
        assert!(jump.is_active(), "should show labels for 3 backward matches");
    });

    // Step 2: Select nearest label -> cursor moves to col 5 (last X)
    harness.with_runtime(|rt| {
        let jump = rt.ext_mut::<JumpSessionState>();
        jump.insert_char('s'); // nearest label
        assert!(jump.has_target());
    });

    let exec_cmd = crate::jump::command::JumpExecuteCommand;
    harness.with_runtime(|rt| exec_cmd.execute(rt, &CommandContext::new()));

    harness.with_runtime(|rt| {
        let window = rt.windows().active().unwrap();
        assert_eq!(window.cursor.column, 5, "should be at last X");
    });

    // Step 3: Simulate ; (same direction = backward) from col 5.
    // From col 5, backward X's at col 3, 1 — still multi-match.
    // RepeatFindSame sets is_repeat=true so labels are bypassed.
    let mut repeat_args = CommandContext::new();
    repeat_args.set_buffer_id(buffer_id);
    repeat_args.set("find_char", ArgValue::Char('X'));
    repeat_args.set("find_direction", ArgValue::String("backward".to_string()));
    repeat_args.set("is_repeat", ArgValue::Bool(true));

    harness.with_runtime(|rt| cmd.execute(rt, &repeat_args));
    harness.with_runtime(|rt| {
        let jump = rt.ext_mut::<JumpSessionState>();
        assert!(!jump.is_active(), "backward repeat should advance, not re-enter label mode");

        let window = rt.windows().active().unwrap();
        assert_eq!(
            window.cursor.column, 3,
            "backward repeat should move cursor to previous X at col 3"
        );
    });
}

/// After multi-match `fX` with counted bypass (`2fX` -> no labels, cursor at
/// 2nd X), a subsequent repeat should advance to the 3rd X. This is the
/// baseline that works — included for regression and contrast with the
/// label-selection path.
#[test]
fn test_repeat_after_counted_find_works_correctly() {
    let cmd = EnhancedFindCharCommand;
    // Buffer: "aXaXaXa" — X at cols 1, 3, 5
    let mut harness = TestSessionRuntime::with_buffer("aXaXaXa");
    let buffer_id = harness.active_buffer().unwrap();

    // Step 1: 2fX — counted, bypasses labels, jumps to 2nd X (col 3)
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("find_char", ArgValue::Char('X'));
    args.set("count", ArgValue::Count(2));

    harness.with_runtime(|rt| cmd.execute(rt, &args));

    harness.with_runtime(|rt| {
        let jump = rt.ext_mut::<JumpSessionState>();
        assert!(!jump.is_active(), "counted find should not show labels");

        let window = rt.windows().active().unwrap();
        assert_eq!(window.cursor.column, 3, "2fX should land at 2nd X");
    });

    // Step 2: Simulate ; repeat (count=1) from col 3.
    // From col 3, there is 1 X ahead at col 5 — single match, should auto-jump.
    let mut repeat_args = CommandContext::new();
    repeat_args.set_buffer_id(buffer_id);
    repeat_args.set("find_char", ArgValue::Char('X'));

    harness.with_runtime(|rt| cmd.execute(rt, &repeat_args));

    harness.with_runtime(|rt| {
        let jump = rt.ext_mut::<JumpSessionState>();
        assert!(!jump.is_active(), "single match ahead should auto-jump");

        let window = rt.windows().active().unwrap();
        assert_eq!(window.cursor.column, 5, "repeat should advance to 3rd X");
    });
}

/// Edge case: `is_repeat=true` with `count > 1` should still bypass labels
/// and use the standard motion engine. Both guards trigger independently.
#[test]
fn test_repeat_with_count_bypasses_labels() {
    let cmd = EnhancedFindCharCommand;
    let mut harness = TestSessionRuntime::with_buffer("aXaXaXa");
    let buffer_id = harness.active_buffer().unwrap();

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("find_char", ArgValue::Char('X'));
    args.set("count", ArgValue::Count(2));
    args.set("is_repeat", ArgValue::Bool(true));

    harness.with_runtime(|rt| cmd.execute(rt, &args));

    harness.with_runtime(|rt| {
        let jump = rt.ext_mut::<JumpSessionState>();
        assert!(!jump.is_active(), "count + is_repeat should not show labels");

        let window = rt.windows().active().unwrap();
        assert_eq!(window.cursor.column, 3, "2fX repeat should land at 2nd X");
    });
}

/// Edge case: `is_repeat=true` with till mode (`find_inclusive=false`) should
/// bypass labels and stop one position before the target character.
#[test]
fn test_repeat_with_till_mode_bypasses_labels() {
    let cmd = EnhancedFindCharCommand;
    // Buffer: "aXbXc" — X at cols 1, 3
    let mut harness = TestSessionRuntime::with_buffer("aXbXc");
    let buffer_id = harness.active_buffer().unwrap();

    // First: tX with labels -> select first label -> cursor at col 0 (before X)
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("find_char", ArgValue::Char('X'));
    args.set("find_inclusive", ArgValue::Bool(false)); // till mode

    harness.with_runtime(|rt| cmd.execute(rt, &args));

    // 2 matches -> labels shown
    harness.with_runtime(|rt| {
        let jump = rt.ext_mut::<JumpSessionState>();
        assert!(jump.is_active(), "till mode with 2 matches should show labels");
        // Select first label
        jump.insert_char('s');
        assert!(jump.has_target());
    });

    let exec_cmd = crate::jump::command::JumpExecuteCommand;
    harness.with_runtime(|rt| exec_cmd.execute(rt, &CommandContext::new()));

    // Jump lands ON the target (label selection bypasses till offset)
    let first_col = harness.with_runtime(|rt| rt.windows().active().unwrap().cursor.column);

    // ; repeat with till mode should bypass labels and land before next X
    let mut repeat_args = CommandContext::new();
    repeat_args.set_buffer_id(buffer_id);
    repeat_args.set("find_char", ArgValue::Char('X'));
    repeat_args.set("find_inclusive", ArgValue::Bool(false));
    repeat_args.set("is_repeat", ArgValue::Bool(true));

    harness.with_runtime(|rt| cmd.execute(rt, &repeat_args));

    harness.with_runtime(|rt| {
        let jump = rt.ext_mut::<JumpSessionState>();
        assert!(!jump.is_active(), "till repeat should not show labels");

        let window = rt.windows().active().unwrap();
        // Should have advanced past the first X position
        assert!(
            window.cursor.column > first_col,
            "till repeat should advance cursor past first match"
        );
    });
}
