//! Tests for mark command handlers.

use {
    reovim_driver_command::{ArgValue, Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{api::BufferApi, testing::TestSessionRuntime},
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

// =============================================================================
// GotoMarkExact — missing error path
// =============================================================================

#[test]
fn test_goto_mark_exact_no_char() {
    let mut test = TestSessionRuntime::with_buffer("hello\n");
    let cmd = GotoMarkExact;
    let buf_id = test.active_buffer().unwrap();

    let mut args = CommandContext::new();
    args.set_buffer_id(buf_id);
    // No mark_char set

    test.with_runtime(|runtime| {
        let result = cmd.execute(runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    });
}

// =============================================================================
// Global mark goto tests
// =============================================================================

#[test]
fn test_goto_mark_line_global() {
    let mut test = TestSessionRuntime::with_buffer("line 0\nline 1\nline 2\n");
    let buf_id = test.active_buffer().unwrap();

    // Set global mark 'A' at (2, 4)
    test.with_runtime(|runtime| {
        if let Some(w) = runtime.windows_mut().active_mut() {
            w.cursor.line = 2;
            w.cursor.column = 4;
        }
    });

    let mut set_args = CommandContext::new();
    set_args.set("mark_char", ArgValue::Char('A'));
    set_args.set_buffer_id(buf_id);

    test.with_runtime(|runtime| {
        let result = SetMark.execute(runtime, &set_args);
        assert_eq!(result, CommandResult::Success);
    });

    // Move cursor elsewhere
    test.with_runtime(|runtime| {
        if let Some(w) = runtime.windows_mut().active_mut() {
            w.cursor.line = 0;
            w.cursor.column = 0;
        }
    });

    // GotoMarkLine with global mark — should jump to (2, 0)
    let mut goto_args = CommandContext::new();
    goto_args.set("mark_char", ArgValue::Char('A'));
    goto_args.set_buffer_id(buf_id);

    test.with_runtime(|runtime| {
        let result = GotoMarkLine.execute(runtime, &goto_args);
        assert_eq!(result, CommandResult::Success);
    });

    test.assert_cursor(2, 0);
}

#[test]
fn test_goto_mark_exact_global() {
    let mut test = TestSessionRuntime::with_buffer("line 0\nline 1\nline 2\n");
    let buf_id = test.active_buffer().unwrap();

    // Set global mark 'B' at (1, 3)
    test.with_runtime(|runtime| {
        if let Some(w) = runtime.windows_mut().active_mut() {
            w.cursor.line = 1;
            w.cursor.column = 3;
        }
    });

    let mut set_args = CommandContext::new();
    set_args.set("mark_char", ArgValue::Char('B'));
    set_args.set_buffer_id(buf_id);

    test.with_runtime(|runtime| {
        let result = SetMark.execute(runtime, &set_args);
        assert_eq!(result, CommandResult::Success);
    });

    // Move cursor elsewhere
    test.with_runtime(|runtime| {
        if let Some(w) = runtime.windows_mut().active_mut() {
            w.cursor.line = 0;
            w.cursor.column = 0;
        }
    });

    // GotoMarkExact with global mark — should jump to (1, 3)
    let mut goto_args = CommandContext::new();
    goto_args.set("mark_char", ArgValue::Char('B'));
    goto_args.set_buffer_id(buf_id);

    test.with_runtime(|runtime| {
        let result = GotoMarkExact.execute(runtime, &goto_args);
        assert_eq!(result, CommandResult::Success);
    });

    test.assert_cursor(1, 3);
}

#[test]
fn test_goto_mark_global_nonexistent() {
    let mut test = TestSessionRuntime::with_buffer("hello\n");
    let buf_id = test.active_buffer().unwrap();

    let mut args = CommandContext::new();
    args.set("mark_char", ArgValue::Char('Z'));
    args.set_buffer_id(buf_id);

    test.with_runtime(|runtime| {
        let result = GotoMarkLine.execute(runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    });
}

// =============================================================================
// Special mark (non-alphanumeric) tests
// =============================================================================

#[test]
fn test_goto_special_mark_not_set() {
    let mut test = TestSessionRuntime::with_buffer("hello\nworld\n");
    let buf_id = test.active_buffer().unwrap();

    // Try jumping to a special mark that hasn't been set
    let mut args = CommandContext::new();
    args.set("mark_char", ArgValue::Char('.'));
    args.set_buffer_id(buf_id);

    test.with_runtime(|runtime| {
        let result = GotoMarkLine.execute(runtime, &args);
        // Special mark '.' not set → "Mark not set"
        assert!(matches!(result, CommandResult::Error(_)));
    });
}

// =============================================================================
// Cross-buffer mark jump
// =============================================================================

#[test]
fn test_goto_global_mark_cross_buffer() {
    use reovim_kernel::api::v1::{Buffer, Mark, RwLock};
    use std::sync::Arc;

    let mut test = TestSessionRuntime::with_buffer("buffer 1\n");
    let buf1 = test.active_buffer().unwrap();

    // Create a second buffer
    let buffer2 = Buffer::from_string("buffer 2\n");
    let buf2 = test
        .kernel()
        .buffers
        .register(Arc::new(RwLock::new(buffer2)));

    // Set global mark 'C' pointing to buffer 2 at (0, 5)
    test.with_runtime(|runtime| {
        let mark = Mark::new(Position::new(0, 5), buf2);
        runtime.kernel().global_marks.write().set_global('C', mark);
    });

    // Current buffer is buf1, goto mark 'C' which is in buf2
    let mut goto_args = CommandContext::new();
    goto_args.set("mark_char", ArgValue::Char('C'));
    goto_args.set_buffer_id(buf1);

    test.with_runtime(|runtime| {
        let result = GotoMarkExact.execute(runtime, &goto_args);
        assert_eq!(result, CommandResult::Success);

        // Active buffer should have switched to buf2
        assert_eq!(runtime.active_buffer(), Some(buf2));

        // Window's buffer_id should be updated
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.buffer_id, Some(buf2));

        // Cursor should be at (0, 5)
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 5);
    });
}

#[test]
fn test_goto_global_mark_cross_buffer_line_mode() {
    use reovim_kernel::api::v1::{Buffer, Mark, RwLock};
    use std::sync::Arc;

    let mut test = TestSessionRuntime::with_buffer("buffer 1\n");
    let buf1 = test.active_buffer().unwrap();

    // Create a second buffer
    let buffer2 = Buffer::from_string("buffer 2\nline 2\n");
    let buf2 = test
        .kernel()
        .buffers
        .register(Arc::new(RwLock::new(buffer2)));

    // Set global mark 'D' pointing to buffer 2 at (1, 3)
    test.with_runtime(|runtime| {
        let mark = Mark::new(Position::new(1, 3), buf2);
        runtime.kernel().global_marks.write().set_global('D', mark);
    });

    // GotoMarkLine: cross-buffer, line_only=true → column should be 0
    let mut goto_args = CommandContext::new();
    goto_args.set("mark_char", ArgValue::Char('D'));
    goto_args.set_buffer_id(buf1);

    test.with_runtime(|runtime| {
        let result = GotoMarkLine.execute(runtime, &goto_args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.active_buffer(), Some(buf2));

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 1);
        assert_eq!(window.cursor.column, 0);
    });
}

// =============================================================================
// Debug / trait derive tests
// =============================================================================

#[test]
fn test_mark_commands_debug_derive() {
    assert_eq!(format!("{SetMark:?}"), "SetMark");
    assert_eq!(format!("{GotoMarkLine:?}"), "GotoMarkLine");
    assert_eq!(format!("{GotoMarkExact:?}"), "GotoMarkExact");
}

#[test]
fn test_mark_commands_default_derive() {
    fn assert_default<T: Default>(_: T) {}
    assert_default(SetMark);
    assert_default(GotoMarkLine);
    assert_default(GotoMarkExact);
}

#[test]
fn test_mark_commands_copy_clone() {
    fn assert_copy<T: Copy>(_val: T) {}
    assert_copy(SetMark);
    assert_copy(GotoMarkLine);
    assert_copy(GotoMarkExact);
}
