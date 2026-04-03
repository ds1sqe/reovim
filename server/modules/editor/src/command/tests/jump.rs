//! Tests for jump list command handlers.

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler},
    reovim_driver_session::{JumpEntry, testing::TestSessionRuntime},
    reovim_types_text::Position,
};

use crate::command::{JumpBackward, JumpForward};

// =============================================================================
// JumpBackward
// =============================================================================

#[test]
fn test_jump_backward_empty_jumplist() {
    let mut test = TestSessionRuntime::with_buffer("hello\nworld\n");
    let cmd = JumpBackward;
    let args = CommandContext::new();

    test.with_runtime(|runtime| {
        let result = cmd.execute(runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);
    });

    // Cursor shouldn't move
    test.assert_cursor(0, 0);
}

#[test]
fn test_jump_backward_basic() {
    let mut test = TestSessionRuntime::with_buffer("line 0\nline 1\nline 2\nline 3\nline 4\n");
    let cmd = JumpBackward;
    let buf_id = test.active_buffer().unwrap();

    // Push some entries to jumplist
    test.with_runtime(|runtime| {
        runtime
            .jumplist_mut()
            .push(JumpEntry::new(buf_id, Position::new(0, 0)));
        runtime
            .jumplist_mut()
            .push(JumpEntry::new(buf_id, Position::new(3, 2)));
    });

    // Move cursor to line 4 (simulating a jump)
    test.with_runtime(|runtime| {
        if let Some(w) = runtime.windows_mut().active_mut() {
            w.cursor.line = 4;
            w.cursor.column = 0;
        }
    });

    // Jump backward should go to (3, 2)
    let args = CommandContext::new();
    test.with_runtime(|runtime| {
        cmd.execute(runtime, &args);
    });
    test.assert_cursor(3, 2);

    // Jump backward again should go to (0, 0)
    test.with_runtime(|runtime| {
        cmd.execute(runtime, &args);
    });
    test.assert_cursor(0, 0);
}

// =============================================================================
// JumpForward
// =============================================================================

#[test]
fn test_jump_forward_empty_jumplist() {
    let mut test = TestSessionRuntime::with_buffer("hello\nworld\n");
    let cmd = JumpForward;
    let args = CommandContext::new();

    test.with_runtime(|runtime| {
        let result = cmd.execute(runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);
    });

    test.assert_cursor(0, 0);
}

#[test]
fn test_jump_forward_after_backward() {
    let mut test = TestSessionRuntime::with_buffer("line 0\nline 1\nline 2\n");
    let cmd_back = JumpBackward;
    let cmd_fwd = JumpForward;
    let buf_id = test.active_buffer().unwrap();

    // Push entries
    test.with_runtime(|runtime| {
        runtime
            .jumplist_mut()
            .push(JumpEntry::new(buf_id, Position::new(0, 0)));
        runtime
            .jumplist_mut()
            .push(JumpEntry::new(buf_id, Position::new(2, 0)));
    });

    let args = CommandContext::new();

    // Jump backward from end
    test.with_runtime(|runtime| {
        cmd_back.execute(runtime, &args);
    });
    test.assert_cursor(2, 0);

    // Jump backward again
    test.with_runtime(|runtime| {
        cmd_back.execute(runtime, &args);
    });
    test.assert_cursor(0, 0);

    // Jump forward returns entry at current index (0), which is (0, 0)
    // then increments current to 1
    test.with_runtime(|runtime| {
        cmd_fwd.execute(runtime, &args);
    });
    test.assert_cursor(0, 0);

    // Jump forward again returns entry at index 1, which is (2, 0)
    test.with_runtime(|runtime| {
        cmd_fwd.execute(runtime, &args);
    });
    test.assert_cursor(2, 0);
}

// =============================================================================
// Command metadata
// =============================================================================

#[test]
fn test_jump_backward_command_id() {
    let cmd = JumpBackward;
    assert_eq!(cmd.id(), crate::ids::JUMP_BACKWARD);
    assert!(!cmd.description().is_empty());
    assert!(cmd.args().is_empty());
}

#[test]
fn test_jump_forward_command_id() {
    let cmd = JumpForward;
    assert_eq!(cmd.id(), crate::ids::JUMP_FORWARD);
    assert!(!cmd.description().is_empty());
    assert!(cmd.args().is_empty());
}
