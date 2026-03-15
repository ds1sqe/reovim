//! Tests for mark command handlers.

use {
    reovim_driver_command::{ArgValue, Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::testing::TestSessionRuntime,
    reovim_kernel::api::v1::Position,
};

use crate::command::{GotoMarkExact, GotoMarkLine, SetMark};

// =============================================================================
// SetMark tests
// =============================================================================

#[test]
fn test_set_mark_local() {
    let mut test = TestSessionRuntime::with_buffer("hello\nworld\nfoo\n");
    let cmd = SetMark;
    let buf_id = test.active_buffer().unwrap();

    // Move cursor to (1, 3)
    test.with_runtime(|runtime| {
        if let Some(w) = runtime.windows_mut().active_mut() {
            w.cursor.line = 1;
            w.cursor.column = 3;
        }
    });

    let mut args = CommandContext::new();
    args.set("mark_char", ArgValue::Char('a'));
    args.set_buffer_id(buf_id);

    test.with_runtime(|runtime| {
        let result = cmd.execute(runtime, &args);
        assert_eq!(result, CommandResult::Success);
        let mark = runtime.local_marks().get_local('a');
        assert_eq!(mark, Some(Position::new(1, 3)));
    });
}

#[test]
fn test_set_mark_global() {
    let mut test = TestSessionRuntime::with_buffer("hello\nworld\n");
    let cmd = SetMark;
    let buf_id = test.active_buffer().unwrap();

    let mut args = CommandContext::new();
    args.set("mark_char", ArgValue::Char('A'));
    args.set_buffer_id(buf_id);

    test.with_runtime(|runtime| {
        let result = cmd.execute(runtime, &args);
        assert_eq!(result, CommandResult::Success);
    });

    // Verify global mark was set
    test.with_runtime(|runtime| {
        let mark = runtime
            .kernel()
            .global_marks
            .read()
            .get_global('A')
            .map(|m| (m.position, m.buffer_id));
        assert_eq!(mark, Some((Position::new(0, 0), buf_id)));
    });
}

#[test]
fn test_set_mark_invalid_char() {
    let mut test = TestSessionRuntime::with_buffer("hello\n");
    let cmd = SetMark;
    let buf_id = test.active_buffer().unwrap();

    let mut args = CommandContext::new();
    args.set("mark_char", ArgValue::Char('1'));
    args.set_buffer_id(buf_id);

    test.with_runtime(|runtime| {
        let result = cmd.execute(runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    });
}

#[test]
fn test_set_mark_no_mark_char() {
    let mut test = TestSessionRuntime::with_buffer("hello\n");
    let cmd = SetMark;
    let buf_id = test.active_buffer().unwrap();

    let mut args = CommandContext::new();
    args.set_buffer_id(buf_id);

    test.with_runtime(|runtime| {
        let result = cmd.execute(runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    });
}

#[test]
fn test_set_mark_no_buffer() {
    let mut test = TestSessionRuntime::with_buffer("hello\n");
    let cmd = SetMark;

    let mut args = CommandContext::new();
    args.set("mark_char", ArgValue::Char('a'));
    // No buffer_id set

    test.with_runtime(|runtime| {
        let result = cmd.execute(runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    });
}

#[test]
fn test_set_mark_no_window() {
    let mut test = TestSessionRuntime::new();
    let cmd = SetMark;

    let mut args = CommandContext::new();
    args.set("mark_char", ArgValue::Char('a'));
    args.set_buffer_id(reovim_kernel::api::v1::BufferId::new());

    test.with_runtime(|runtime| {
        let result = cmd.execute(runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    });
}

// =============================================================================
// GotoMarkLine tests
// =============================================================================

#[test]
fn test_goto_mark_line_local() {
    let mut test = TestSessionRuntime::with_buffer("line 0\nline 1\nline 2\nline 3\n");
    let cmd_set = SetMark;
    let cmd_goto = GotoMarkLine;
    let buf_id = test.active_buffer().unwrap();

    // Set mark 'a' at (2, 3)
    test.with_runtime(|runtime| {
        if let Some(w) = runtime.windows_mut().active_mut() {
            w.cursor.line = 2;
            w.cursor.column = 3;
        }
    });

    let mut set_args = CommandContext::new();
    set_args.set("mark_char", ArgValue::Char('a'));
    set_args.set_buffer_id(buf_id);

    test.with_runtime(|runtime| {
        cmd_set.execute(runtime, &set_args);
    });

    // Move cursor elsewhere
    test.with_runtime(|runtime| {
        if let Some(w) = runtime.windows_mut().active_mut() {
            w.cursor.line = 0;
            w.cursor.column = 0;
        }
    });

    // Goto mark line — should jump to (2, 0), not (2, 3)
    let mut goto_args = CommandContext::new();
    goto_args.set("mark_char", ArgValue::Char('a'));
    goto_args.set_buffer_id(buf_id);

    test.with_runtime(|runtime| {
        let result = cmd_goto.execute(runtime, &goto_args);
        assert_eq!(result, CommandResult::Success);
    });

    test.assert_cursor(2, 0);
}

#[test]
fn test_goto_mark_nonexistent() {
    let mut test = TestSessionRuntime::with_buffer("hello\n");
    let cmd = GotoMarkLine;
    let buf_id = test.active_buffer().unwrap();

    let mut args = CommandContext::new();
    args.set("mark_char", ArgValue::Char('z'));
    args.set_buffer_id(buf_id);

    test.with_runtime(|runtime| {
        let result = cmd.execute(runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    });
}

#[test]
fn test_goto_mark_line_no_buffer() {
    let mut test = TestSessionRuntime::with_buffer("hello\n");
    let cmd = GotoMarkLine;

    let mut args = CommandContext::new();
    args.set("mark_char", ArgValue::Char('a'));

    test.with_runtime(|runtime| {
        let result = cmd.execute(runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    });
}

#[test]
fn test_goto_mark_line_no_char() {
    let mut test = TestSessionRuntime::with_buffer("hello\n");
    let cmd = GotoMarkLine;
    let buf_id = test.active_buffer().unwrap();

    let mut args = CommandContext::new();
    args.set_buffer_id(buf_id);

    test.with_runtime(|runtime| {
        let result = cmd.execute(runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    });
}

// =============================================================================
// GotoMarkExact tests
// =============================================================================

#[test]
fn test_goto_mark_exact_local() {
    let mut test = TestSessionRuntime::with_buffer("line 0\nline 1\nline 2\nline 3\n");
    let cmd_set = SetMark;
    let cmd_goto = GotoMarkExact;
    let buf_id = test.active_buffer().unwrap();

    // Set mark 'b' at (2, 3)
    test.with_runtime(|runtime| {
        if let Some(w) = runtime.windows_mut().active_mut() {
            w.cursor.line = 2;
            w.cursor.column = 3;
        }
    });

    let mut set_args = CommandContext::new();
    set_args.set("mark_char", ArgValue::Char('b'));
    set_args.set_buffer_id(buf_id);

    test.with_runtime(|runtime| {
        cmd_set.execute(runtime, &set_args);
    });

    // Move cursor elsewhere
    test.with_runtime(|runtime| {
        if let Some(w) = runtime.windows_mut().active_mut() {
            w.cursor.line = 0;
            w.cursor.column = 0;
        }
    });

    // Goto mark exact — should jump to (2, 3), preserving column
    let mut goto_args = CommandContext::new();
    goto_args.set("mark_char", ArgValue::Char('b'));
    goto_args.set_buffer_id(buf_id);

    test.with_runtime(|runtime| {
        let result = cmd_goto.execute(runtime, &goto_args);
        assert_eq!(result, CommandResult::Success);
    });

    test.assert_cursor(2, 3);
}

#[test]
fn test_goto_mark_exact_no_buffer() {
    let mut test = TestSessionRuntime::with_buffer("hello\n");
    let cmd = GotoMarkExact;

    let mut args = CommandContext::new();
    args.set("mark_char", ArgValue::Char('a'));

    test.with_runtime(|runtime| {
        let result = cmd.execute(runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    });
}

// =============================================================================
// Mark goto pushes to jumplist
// =============================================================================

#[test]
fn test_mark_goto_pushes_to_jumplist() {
    let mut test = TestSessionRuntime::with_buffer("line 0\nline 1\nline 2\n");
    let buf_id = test.active_buffer().unwrap();

    // Set mark 'a' at (2, 0)
    test.with_runtime(|runtime| {
        if let Some(w) = runtime.windows_mut().active_mut() {
            w.cursor.line = 2;
            w.cursor.column = 0;
        }
    });

    let mut set_args = CommandContext::new();
    set_args.set("mark_char", ArgValue::Char('a'));
    set_args.set_buffer_id(buf_id);

    test.with_runtime(|runtime| {
        SetMark.execute(runtime, &set_args);
    });

    // Move to (0, 0)
    test.with_runtime(|runtime| {
        if let Some(w) = runtime.windows_mut().active_mut() {
            w.cursor.line = 0;
            w.cursor.column = 0;
        }
    });

    // Goto mark — should push (0, 0) to jumplist before jumping
    let mut goto_args = CommandContext::new();
    goto_args.set("mark_char", ArgValue::Char('a'));
    goto_args.set_buffer_id(buf_id);

    test.with_runtime(|runtime| {
        GotoMarkLine.execute(runtime, &goto_args);
        assert_eq!(runtime.jumplist().len(), 1);
        let entry = &runtime.jumplist().entries()[0];
        assert_eq!(entry.position, Position::new(0, 0));
        assert_eq!(entry.buffer, buf_id);
    });
}

// =============================================================================
// Command metadata
// =============================================================================

#[test]
fn test_set_mark_command_id() {
    let cmd = SetMark;
    assert_eq!(cmd.id(), crate::ids::SET_MARK);
    assert!(!cmd.description().is_empty());
    assert_eq!(cmd.args().len(), 1);
}

#[test]
fn test_goto_mark_line_command_id() {
    let cmd = GotoMarkLine;
    assert_eq!(cmd.id(), crate::ids::GOTO_MARK_LINE);
    assert!(!cmd.description().is_empty());
    assert_eq!(cmd.args().len(), 1);
}

#[test]
fn test_goto_mark_exact_command_id() {
    let cmd = GotoMarkExact;
    assert_eq!(cmd.id(), crate::ids::GOTO_MARK_EXACT);
    assert!(!cmd.description().is_empty());
    assert_eq!(cmd.args().len(), 1);
}
