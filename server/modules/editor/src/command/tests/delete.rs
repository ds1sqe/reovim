use {
    super::super::*,
    reovim_driver_command::{ArgKind, ArgValue, Command, CommandContext},
    reovim_driver_session::{
        ClientId, ExtensionMap, Session, SessionRuntime, Window, WindowLayout,
        testing::StubExecutor,
    },
    reovim_kernel::{
        api::v1::{
            Buffer, BufferId, HistoryRing, Jumplist, KernelContext, MarkBank, ModeStack, Position,
            RegisterBank,
        },
        testing::{create_test_context, test_mode},
    },
};

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

#[cfg_attr(coverage_nightly, coverage(off))]
impl TestState {
    fn with_window(buffer_id: BufferId) -> Self {
        let home_mode = test_mode();
        let mut state = Self {
            session: Session::new(ClientId::new(1), home_mode.clone()),
            mode_stack: ModeStack::new(home_mode),
            windows: WindowLayout::empty(),
            extensions: ExtensionMap::new(),
            compositor: None,
            tabs: reovim_driver_session::TabPageSet::new(),
            registers: RegisterBank::new(),
            clipboard_history: HistoryRing::new(),
            local_marks: MarkBank::new(),
            jumplist: Jumplist::new(),
            active_buffer: None,
            terminal_size: (80, 24),
        };
        let mut window = Window::new();
        window.buffer_id = Some(buffer_id);
        state.windows.add(window);
        state.active_buffer = Some(buffer_id);
        state
    }

    fn runtime<'a>(
        &'a mut self,
        kernel: &'a KernelContext,
        executor: &'a StubExecutor,
    ) -> SessionRuntime<'a> {
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
            executor,
        )
    }
}

// =========================================================================
// DeleteChar tests
// =========================================================================

#[test]
fn test_delete_char_id() {
    let cmd = DeleteChar;
    assert_eq!(cmd.id().name(), "delete-char");
}

#[test]
fn test_delete_char_description() {
    let cmd = DeleteChar;
    assert_eq!(cmd.description(), "Delete character under cursor");
}

#[test]
fn test_delete_char_args() {
    let cmd = DeleteChar;
    let args = cmd.args();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].name, "count");
    assert_eq!(args[0].kind, ArgKind::Count);
}

#[test]
fn test_delete_char_no_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mode = test_mode();
    let mut session = Session::new(ClientId::new(1), mode.clone());
    let executor = StubExecutor;
    let mut mode_stack = ModeStack::new(mode);
    let mut windows = WindowLayout::empty();
    let mut extensions = ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let mut runtime = SessionRuntime::new(
        &mut session,
        reovim_driver_session::ClientContext {
            mode_stack: &mut mode_stack,
            windows: &mut windows,
            extensions: &mut extensions,
            compositor: &mut compositor,
            tabs: &mut tabs,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            local_marks: &mut local_marks,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    let args = CommandContext::new();
    let result = DeleteChar.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_delete_char_no_window_returns_error() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(buffer);
    let mode = test_mode();
    let mut session = Session::new(ClientId::new(1), mode.clone());
    let executor = StubExecutor;
    let mut mode_stack = ModeStack::new(mode);
    let mut windows = WindowLayout::empty();
    let mut extensions = ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let mut runtime = SessionRuntime::new(
        &mut session,
        reovim_driver_session::ClientContext {
            mode_stack: &mut mode_stack,
            windows: &mut windows,
            extensions: &mut extensions,
            compositor: &mut compositor,
            tabs: &mut tabs,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            local_marks: &mut local_marks,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = DeleteChar.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_delete_char_single() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = DeleteChar.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let content = buf.read().line(0).map(str::to_owned);
    assert_eq!(content.as_deref(), Some("ello"));
}

#[test]
fn test_delete_char_with_count() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(3));

    let result = DeleteChar.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let content = buf.read().line(0).map(str::to_owned);
    assert_eq!(content.as_deref(), Some("lo"));
}

#[test]
fn test_delete_char_count_clamped_to_eol() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hi");
    let buffer_id = kernel.buffers.register(buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(100));

    let result = DeleteChar.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let content = buf.read().line(0).map(str::to_owned);
    assert_eq!(content.as_deref(), Some(""));
}

#[test]
fn test_delete_char_empty_line_is_noop() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("");
    let buffer_id = kernel.buffers.register(buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = DeleteChar.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_delete_char_at_eol_is_noop() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("ab");
    let buffer_id = kernel.buffers.register(buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 2).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = DeleteChar.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let content = buf.read().line(0).map(str::to_owned);
    assert_eq!(content.as_deref(), Some("ab"));
}

// =========================================================================
// DeleteCharBefore tests
// =========================================================================

#[test]
fn test_delete_char_before_id() {
    let cmd = DeleteCharBefore;
    assert_eq!(cmd.id().name(), "delete-char-before");
}

#[test]
fn test_delete_char_before_description() {
    let cmd = DeleteCharBefore;
    assert_eq!(cmd.description(), "Delete character before cursor");
}

#[test]
fn test_delete_char_before_args() {
    let cmd = DeleteCharBefore;
    let args = cmd.args();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].name, "count");
    assert_eq!(args[0].kind, ArgKind::Count);
}

#[test]
fn test_delete_char_before_no_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mode = test_mode();
    let mut session = Session::new(ClientId::new(1), mode.clone());
    let executor = StubExecutor;
    let mut mode_stack = ModeStack::new(mode);
    let mut windows = WindowLayout::empty();
    let mut extensions = ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let mut runtime = SessionRuntime::new(
        &mut session,
        reovim_driver_session::ClientContext {
            mode_stack: &mut mode_stack,
            windows: &mut windows,
            extensions: &mut extensions,
            compositor: &mut compositor,
            tabs: &mut tabs,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            local_marks: &mut local_marks,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    let args = CommandContext::new();
    let result = DeleteCharBefore.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_delete_char_before_no_window_returns_error() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(buffer);
    let mode = test_mode();
    let mut session = Session::new(ClientId::new(1), mode.clone());
    let executor = StubExecutor;
    let mut mode_stack = ModeStack::new(mode);
    let mut windows = WindowLayout::empty();
    let mut extensions = ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let mut runtime = SessionRuntime::new(
        &mut session,
        reovim_driver_session::ClientContext {
            mode_stack: &mut mode_stack,
            windows: &mut windows,
            extensions: &mut extensions,
            compositor: &mut compositor,
            tabs: &mut tabs,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            local_marks: &mut local_marks,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = DeleteCharBefore.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_delete_char_before_at_col_zero_first_line() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    // Cursor at (0, 0) - can't delete before first line
    let result = DeleteCharBefore.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let content = buf.read().line(0).map(str::to_owned);
    assert_eq!(content.as_deref(), Some("hello"));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_delete_char_before_joins_with_previous_line() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("line one\nline two");
    let buffer_id = kernel.buffers.register(buffer);
    let mut state = TestState::with_window(buffer_id);
    // Set cursor to start of second line
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(1, 0).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = DeleteCharBefore.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Lines should be joined
    let buf = kernel.buffers.get(buffer_id).unwrap();
    let buf_read = buf.read();
    assert_eq!(buf_read.line_count(), 1);
    assert_eq!(buf_read.line(0), Some("line oneline two"));
    drop(buf_read);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_delete_char_before_single() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 3).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = DeleteCharBefore.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let content = buf.read().line(0).map(str::to_owned);
    assert_eq!(content.as_deref(), Some("helo"));

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.column, 2);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_delete_char_before_with_count() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 4).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(3));

    let result = DeleteCharBefore.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let content = buf.read().line(0).map(str::to_owned);
    assert_eq!(content.as_deref(), Some("ho"));

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.column, 1);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_delete_char_before_count_clamped() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 2).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(100));

    let result = DeleteCharBefore.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let content = buf.read().line(0).map(str::to_owned);
    assert_eq!(content.as_deref(), Some("llo"));

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.column, 0);
}

// =========================================================================
// DeleteLine tests
// =========================================================================

#[test]
fn test_delete_line_id() {
    let cmd = DeleteLine;
    assert_eq!(cmd.id().name(), "delete-line");
}

#[test]
fn test_delete_line_description() {
    let cmd = DeleteLine;
    assert_eq!(cmd.description(), "Delete current line");
}

#[test]
fn test_delete_line_args() {
    let cmd = DeleteLine;
    let args = cmd.args();
    assert_eq!(args.len(), 2);
    assert_eq!(args[0].name, "count");
    assert_eq!(args[0].kind, ArgKind::Count);
    assert_eq!(args[1].name, "register");
    assert_eq!(args[1].kind, ArgKind::Register);
}

#[test]
fn test_delete_line_no_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mode = test_mode();
    let mut session = Session::new(ClientId::new(1), mode.clone());
    let executor = StubExecutor;
    let mut mode_stack = ModeStack::new(mode);
    let mut windows = WindowLayout::empty();
    let mut extensions = ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let mut runtime = SessionRuntime::new(
        &mut session,
        reovim_driver_session::ClientContext {
            mode_stack: &mut mode_stack,
            windows: &mut windows,
            extensions: &mut extensions,
            compositor: &mut compositor,
            tabs: &mut tabs,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            local_marks: &mut local_marks,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    let args = CommandContext::new();
    let result = DeleteLine.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_delete_line_empty_buffer() {
    let kernel = create_test_context();
    let buffer = Buffer::new();
    let buffer_id = kernel.buffers.register(buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = DeleteLine.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_delete_line_single_line() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("only line");
    let buffer_id = kernel.buffers.register(buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = DeleteLine.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Check register has deleted content (per-client registers, #515)
    drop(runtime);
    let content = state.registers.get().clone();
    assert!(content.is_linewise());
    assert_eq!(content.text, "only line\n");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_delete_line_middle_line() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("line 1\nline 2\nline 3");
    let buffer_id = kernel.buffers.register(buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(1, 0).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = DeleteLine.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let buf_read = buf.read();
    assert_eq!(buf_read.line_count(), 2);
    assert_eq!(buf_read.line(0), Some("line 1"));
    assert_eq!(buf_read.line(1), Some("line 3"));
    drop(buf_read);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_delete_line_last_line() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("line 1\nline 2\nline 3");
    let buffer_id = kernel.buffers.register(buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(2, 0).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = DeleteLine.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let buf_read = buf.read();
    assert_eq!(buf_read.line_count(), 2);
    assert_eq!(buf_read.line(0), Some("line 1"));
    assert_eq!(buf_read.line(1), Some("line 2"));
    drop(buf_read);
}

#[test]
fn test_delete_line_with_count() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("line 1\nline 2\nline 3\nline 4");
    let buffer_id = kernel.buffers.register(buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));

    let result = DeleteLine.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let buf_read = buf.read();
    assert_eq!(buf_read.line_count(), 2);
    assert_eq!(buf_read.line(0), Some("line 3"));
    assert_eq!(buf_read.line(1), Some("line 4"));
    drop(buf_read);

    // Check register content (per-client registers, #515)
    drop(runtime);
    let content = state.registers.get().clone();
    assert!(content.is_linewise());
    assert_eq!(content.text, "line 1\nline 2\n");
}

#[test]
fn test_delete_line_with_indented_remaining() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("first\n    indented\nthird");
    let buffer_id = kernel.buffers.register(buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = DeleteLine.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Cursor should be at first non-blank of remaining line
    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 4); // First non-blank in "    indented"
}

// =========================================================================
// DeleteToEndOfLine tests
// =========================================================================

#[test]
fn test_delete_to_eol_id() {
    let cmd = DeleteToEndOfLine;
    assert_eq!(cmd.id().name(), "delete-to-eol");
}

#[test]
fn test_delete_to_eol_description() {
    let cmd = DeleteToEndOfLine;
    assert_eq!(cmd.description(), "Delete to end of line");
}

#[test]
fn test_delete_to_eol_args() {
    let cmd = DeleteToEndOfLine;
    let args = cmd.args();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].name, "register");
    assert_eq!(args[0].kind, ArgKind::Register);
}

#[test]
fn test_delete_to_eol_no_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mode = test_mode();
    let mut session = Session::new(ClientId::new(1), mode.clone());
    let executor = StubExecutor;
    let mut mode_stack = ModeStack::new(mode);
    let mut windows = WindowLayout::empty();
    let mut extensions = ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let mut runtime = SessionRuntime::new(
        &mut session,
        reovim_driver_session::ClientContext {
            mode_stack: &mut mode_stack,
            windows: &mut windows,
            extensions: &mut extensions,
            compositor: &mut compositor,
            tabs: &mut tabs,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            local_marks: &mut local_marks,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    let args = CommandContext::new();
    let result = DeleteToEndOfLine.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_delete_to_eol_no_window_returns_error() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(buffer);
    let mode = test_mode();
    let mut session = Session::new(ClientId::new(1), mode.clone());
    let executor = StubExecutor;
    let mut mode_stack = ModeStack::new(mode);
    let mut windows = WindowLayout::empty();
    let mut extensions = ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let mut runtime = SessionRuntime::new(
        &mut session,
        reovim_driver_session::ClientContext {
            mode_stack: &mut mode_stack,
            windows: &mut windows,
            extensions: &mut extensions,
            compositor: &mut compositor,
            tabs: &mut tabs,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            local_marks: &mut local_marks,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = DeleteToEndOfLine.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_delete_to_eol_from_start() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = kernel.buffers.register(buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = DeleteToEndOfLine.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let content = buf.read().line(0).map(str::to_owned);
    assert_eq!(content.as_deref(), Some(""));

    // Check register content (per-client registers, #515)
    drop(runtime);
    let content = state.registers.get().clone();
    assert!(!content.is_linewise());
    assert_eq!(content.text, "hello world");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_delete_to_eol_from_middle() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = kernel.buffers.register(buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 5).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = DeleteToEndOfLine.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let content = buf.read().line(0).map(str::to_owned);
    assert_eq!(content.as_deref(), Some("hello"));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_delete_to_eol_at_eol_is_noop() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hi");
    let buffer_id = kernel.buffers.register(buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 2).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = DeleteToEndOfLine.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let content = buf.read().line(0).map(str::to_owned);
    assert_eq!(content.as_deref(), Some("hi"));
}

// =========================================================================
// DeleteLine - delete all content from first line (else branch)
// =========================================================================

#[test]
fn test_delete_line_all_from_first_line() {
    // This tests the else branch at lines 222-228:
    // Deleting to end of buffer from the first line - delete just the content
    let kernel = create_test_context();
    let buffer = Buffer::from_string("only line here");
    let buffer_id = kernel.buffers.register(buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = DeleteLine.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Check register has the deleted content (per-client registers, #515)
    drop(runtime);
    let content = state.registers.get().clone();
    assert!(content.is_linewise());
    assert_eq!(content.text, "only line here\n");
}

#[test]
fn test_delete_line_all_lines_from_first_with_count() {
    // Delete all lines from the first line using count
    let kernel = create_test_context();
    let buffer = Buffer::from_string("first\nsecond\nthird");
    let buffer_id = kernel.buffers.register(buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(10)); // More than available lines

    let result = DeleteLine.execute(&mut runtime, &args);
    assert!(result.is_success());

    // All lines should be deleted (per-client registers, #515)
    drop(runtime);
    let content = state.registers.get().clone();
    assert!(content.is_linewise());
    assert_eq!(content.text, "first\nsecond\nthird\n");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_delete_line_cursor_beyond_buffer_end() {
    // Cursor at line 5 but buffer has only 2 lines: lines_to_delete = 0
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = kernel.buffers.register(buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(5, 0).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = DeleteLine.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Buffer should remain unchanged
    let buf = kernel.buffers.get(buffer_id).unwrap();
    let buf_read = buf.read();
    assert_eq!(buf_read.line_count(), 2);
    assert_eq!(buf_read.line(0), Some("hello"));
    assert_eq!(buf_read.line(1), Some("world"));
    drop(buf_read);
}
