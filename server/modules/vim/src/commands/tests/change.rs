#![allow(clippy::significant_drop_tightening)]
use {
    super::super::*,
    crate::{ids, modes::VimMode},
    reovim_driver_command::{CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::SessionRuntime,
    reovim_types_text::Position,
};

use {
    reovim_driver_command::Command,
    reovim_driver_session::{ClientId, ExtensionMap, Jumplist, MarkBank, Session, WindowLayout},
    reovim_kernel::{
        api::{
            ModeStack,
            v1::{BufferId, KernelContext, ModeId, ModuleId, RwLock},
        },
        testing::create_test_context,
    },
    reovim_provider_text::Buffer,
    reovim_types_text::{HistoryRing, RegisterBank},
    std::sync::Arc,
};

use reovim_driver_session::{api::ModeApi, testing::StubExecutor};

struct TestState {
    session: Session,
    mode_stack: ModeStack,
    windows: WindowLayout,
    extensions: ExtensionMap,
    compositor: Option<Box<dyn reovim_driver_layout::RootCompositor>>,
    tabs: reovim_driver_session::TabPageSet,
    registers: RegisterBank,
    clipboard_history: HistoryRing,
    local_marks: MarkBank,
    jumplist: Jumplist,
    active_buffer: Option<BufferId>,
    terminal_size: (u16, u16),
}

impl TestState {
    fn with_buffer(buffer_id: Option<BufferId>) -> Self {
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let session = Session::new(ClientId::new(1), home_mode.clone());
        let mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let extensions = ExtensionMap::new();

        let mut window = reovim_driver_session::Window::new();
        if let Some(buffer_id) = buffer_id {
            window.buffer_id = Some(buffer_id);
        }
        windows.add(window);

        Self {
            session,
            mode_stack,
            windows,
            extensions,
            compositor: None,
            tabs: reovim_driver_session::TabPageSet::new(),
            registers: RegisterBank::new(),
            clipboard_history: HistoryRing::new(),
            local_marks: MarkBank::new(),
            jumplist: Jumplist::new(),
            active_buffer: None,
            terminal_size: (80, 24),
        }
    }

    fn runtime<'a>(&'a mut self, kernel: &'a KernelContext) -> SessionRuntime<'a> {
        SessionRuntime::new(
            &mut self.session,
            reovim_driver_session::ClientContext {
                mode_stack: &mut self.mode_stack,
                windows: &mut self.windows,
                extensions: &mut self.extensions,
                compositor: &mut self.compositor,
                tabs: &mut self.tabs,
                registers: &mut self.registers,
                clipboard_history: &mut self.clipboard_history,
                local_marks: &mut self.local_marks,
                jumplist: &mut self.jumplist,
                active_buffer: &mut self.active_buffer,
                terminal_size: &mut self.terminal_size,
            },
            kernel,
            &StubExecutor,
        )
    }
}

// ========================================================================
// Metadata tests
// ========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_line_command_id() {
    let cmd = ChangeLine;
    assert_eq!(cmd.id(), ids::CHANGE_LINE);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_line_description() {
    let cmd = ChangeLine;
    assert!(cmd.description().contains("Change"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_line_args() {
    let cmd = ChangeLine;
    let args = cmd.args();
    assert_eq!(args.len(), 2); // count and register
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_to_eol_command_id() {
    let cmd = ChangeToEndOfLine;
    assert_eq!(cmd.id(), ids::CHANGE_TO_EOL);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_to_eol_description() {
    let cmd = ChangeToEndOfLine;
    assert!(cmd.description().contains("end of line"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_to_eol_args() {
    let cmd = ChangeToEndOfLine;
    let args = cmd.args();
    assert_eq!(args.len(), 1); // register only
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_line_debug() {
    let cmd = ChangeLine;
    let debug = format!("{cmd:?}");
    assert!(debug.contains("ChangeLine"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_to_eol_debug() {
    let cmd = ChangeToEndOfLine;
    let debug = format!("{cmd:?}");
    assert!(debug.contains("ChangeToEndOfLine"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_line_default() {
    let _ = ChangeLine;
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_to_eol_default() {
    let _ = ChangeToEndOfLine;
}

// ========================================================================
// Execute tests - ChangeLine
// ========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_line_no_buffer() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);
    let result = ChangeLine.execute(&mut runtime, &args);
    assert!(matches!(result, CommandResult::Error(_)));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_line_single_line() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = ChangeLine.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.current_mode().name(), VimMode::INSERT_ID.name());

    // Line content should be cleared
    let buf = ctx.buffers.get(buffer_id).unwrap();
    let buf = buf.read();
    assert_eq!(buf.content(), "");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_line_multi_line() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("line1\nline2\nline3");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", reovim_driver_command::ArgValue::Count(2));

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = ChangeLine.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.current_mode().name(), VimMode::INSERT_ID.name());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_line_empty_buffer() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = ChangeLine.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.current_mode().name(), VimMode::INSERT_ID.name());
}

// ========================================================================
// Execute tests - ChangeToEndOfLine
// ========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_to_eol_no_buffer() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);
    let result = ChangeToEndOfLine.execute(&mut runtime, &args);
    assert!(matches!(result, CommandResult::Error(_)));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_to_eol_from_middle() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    // Set cursor at column 5 (on space)
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(0, 5).into();
    }
    let mut runtime = state.runtime(&ctx);
    let result = ChangeToEndOfLine.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.current_mode().name(), VimMode::INSERT_ID.name());

    // Buffer should have "hello" (everything from col 5 deleted)
    let buf = ctx.buffers.get(buffer_id).unwrap();
    let buf = buf.read();
    assert_eq!(buf.content(), "hello");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_to_eol_at_end_of_line() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    // Set cursor at end of line
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(0, 5).into();
    }
    let mut runtime = state.runtime(&ctx);
    let result = ChangeToEndOfLine.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.current_mode().name(), VimMode::INSERT_ID.name());

    // Buffer should be unchanged (already at end of line)
    let buf = ctx.buffers.get(buffer_id).unwrap();
    let buf = buf.read();
    assert_eq!(buf.content(), "hello");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_to_eol_from_start() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = ChangeToEndOfLine.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Entire line content should be deleted
    let buf = ctx.buffers.get(buffer_id).unwrap();
    let buf = buf.read();
    assert_eq!(buf.content(), "");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_line_count_exceeds_buffer() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", reovim_driver_command::ArgValue::Count(99));

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = ChangeLine.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.current_mode().name(), VimMode::INSERT_ID.name());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_line_last_two_lines() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("a\nb\nc");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", reovim_driver_command::ArgValue::Count(2));

    let mut state = TestState::with_buffer(Some(buffer_id));
    // Start on line 1
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(1, 0).into();
    }
    let mut runtime = state.runtime(&ctx);
    let result = ChangeLine.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

// ========================================================================
// Additional ChangeLine execute tests
// ========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_line_with_count_one_explicit() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", reovim_driver_command::ArgValue::Count(1));

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = ChangeLine.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Only first line should be changed
    let buf = ctx.buffers.get(buffer_id).unwrap();
    let buf = buf.read();
    // First line should be cleared, second line should remain
    assert_eq!(buf.line(0).unwrap(), "");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_line_multi_line_three_lines() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("aaa\nbbb\nccc\nddd");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", reovim_driver_command::ArgValue::Count(3));

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = ChangeLine.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.current_mode().name(), VimMode::INSERT_ID.name());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_line_with_register() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("test line");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("register", reovim_driver_command::ArgValue::Register('a'));

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = ChangeLine.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_line_cursor_at_middle_of_line() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(0, 5).into();
    }
    let mut runtime = state.runtime(&ctx);
    let result = ChangeLine.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Cursor should be at start after change
    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.column, 0);
}

// ========================================================================
// Additional ChangeToEndOfLine execute tests
// ========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_to_eol_empty_line() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = ChangeToEndOfLine.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.current_mode().name(), VimMode::INSERT_ID.name());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_to_eol_with_register() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("register", reovim_driver_command::ArgValue::Register('b'));

    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(0, 5).into();
    }
    let mut runtime = state.runtime(&ctx);
    let result = ChangeToEndOfLine.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_to_eol_past_end_of_line() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("abc");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    // Cursor past end of line
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(0, 10).into();
    }
    let mut runtime = state.runtime(&ctx);
    let result = ChangeToEndOfLine.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    // Should enter insert mode without deleting
    assert_eq!(runtime.current_mode().name(), VimMode::INSERT_ID.name());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_to_eol_single_char_line() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("x");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = ChangeToEndOfLine.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let buf = ctx.buffers.get(buffer_id).unwrap();
    let buf = buf.read();
    assert_eq!(buf.content(), "");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_to_eol_multiline_only_affects_current() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(0, 3).into();
    }
    let mut runtime = state.runtime(&ctx);
    let result = ChangeToEndOfLine.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let buf = ctx.buffers.get(buffer_id).unwrap();
    let buf = buf.read();
    // First line should be truncated, second line untouched
    assert_eq!(buf.line(0).unwrap(), "hel");
    assert_eq!(buf.line(1).unwrap(), "world");
}

// ========================================================================
// Clone tests
// ========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_line_clone() {
    let cmd = ChangeLine;
    let cloned = cmd;
    assert_eq!(cloned.id(), ids::CHANGE_LINE);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_to_eol_clone() {
    let cmd = ChangeToEndOfLine;
    let cloned = cmd;
    assert_eq!(cloned.id(), ids::CHANGE_TO_EOL);
}

// ========================================================================
// ChangeLine multi-line end_line >= line_count tests
// ========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_line_last_lines_of_buffer() {
    // Tests the branch: end_line >= line_count (changing to end of buffer)
    let ctx = create_test_context();
    let buffer = Buffer::from_string("a\nb\nc");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", reovim_driver_command::ArgValue::Count(3));

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = ChangeLine.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.current_mode().name(), VimMode::INSERT_ID.name());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_line_two_of_two_from_start() {
    // Change all lines (count=2 from line 0 of 2-line buffer)
    // This triggers: end_line (0+2=2) >= line_count (2)
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", reovim_driver_command::ArgValue::Count(2));

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = ChangeLine.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 0);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_line_multi_from_middle_to_end() {
    // Change from line 2 with count 3, but only 2 lines remain
    // This triggers: end_line >= line_count
    let ctx = create_test_context();
    let buffer = Buffer::from_string("a\nb\nc\nd");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", reovim_driver_command::ArgValue::Count(3));

    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(2, 0).into();
    }
    let mut runtime = state.runtime(&ctx);
    let result = ChangeLine.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_line_multi_not_at_end() {
    // Change 2 lines from middle (NOT at end of buffer) - the normal case branch
    let ctx = create_test_context();
    let buffer = Buffer::from_string("a\nb\nc\nd\ne");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", reovim_driver_command::ArgValue::Count(2));

    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(1, 0).into();
    }
    let mut runtime = state.runtime(&ctx);
    let result = ChangeLine.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Lines b and c should be deleted, replaced with cursor at line 1
    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.line, 1);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_line_registers_deleted_text() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", reovim_driver_command::ArgValue::Count(2));

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    ChangeLine.execute(&mut runtime, &args);
    drop(runtime);

    // Check the register has the deleted text
    let reg = state.registers.get();
    assert!(reg.text.contains("hello"));
    assert!(reg.text.contains("world"));
}
