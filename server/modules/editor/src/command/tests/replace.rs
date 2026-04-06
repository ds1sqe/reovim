use {
    super::super::*,
    reovim_driver_command::{ArgKind, ArgValue, Command, CommandContext},
    reovim_driver_session::{
        ClientId, ExtensionMap, Jumplist, MarkBank, Session, SessionRuntime, TextBufferRegistry,
        Window, WindowLayout, testing::StubExecutor,
    },
    reovim_kernel::{
        api::v1::{BufferId, BufferOps, KernelBuffer, KernelContext, ModeStack, RwLock},
        testing::{create_test_context, test_mode},
    },
    reovim_provider_text::Buffer,
    reovim_types_text::{HistoryRing, Position, RegisterBank},
    std::sync::Arc,
};

/// Create a test kernel with a `TextBufferRegistry` in services.
#[cfg_attr(coverage_nightly, coverage(off))]
fn setup_kernel() -> KernelContext {
    let kernel = create_test_context();
    kernel.services.register(Arc::new(TextBufferRegistry::new()));
    kernel
}

/// Register a buffer in both kernel (byte-level) and text registry.
#[cfg_attr(coverage_nightly, coverage(off))]
fn register_buffer(kernel: &KernelContext, buffer: Buffer) -> BufferId {
    let arc = Arc::new(RwLock::new(buffer));
    let id = kernel
        .buffers
        .register(arc.clone() as Arc<RwLock<dyn KernelBuffer>>);
    kernel
        .services
        .get::<TextBufferRegistry>()
        .unwrap()
        .register(arc as Arc<RwLock<dyn BufferOps>>);
    id
}

/// Read a text buffer from the text registry for assertions.
#[cfg_attr(coverage_nightly, coverage(off))]
fn text_buf(
    kernel: &KernelContext,
    id: BufferId,
) -> Arc<RwLock<dyn BufferOps>> {
    kernel
        .services
        .get::<TextBufferRegistry>()
        .unwrap()
        .get(id)
        .unwrap()
}

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
// ReplaceCharStart tests
// =========================================================================

#[test]
fn test_replace_char_start_id() {
    let cmd = ReplaceCharStart;
    assert_eq!(cmd.id().name(), "replace-char-start");
}

#[test]
fn test_replace_char_start_description() {
    let cmd = ReplaceCharStart;
    assert_eq!(cmd.description(), "Replace character under cursor");
}

#[test]
fn test_replace_char_start_args() {
    let cmd = ReplaceCharStart;
    let args = cmd.args();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].name, "count");
    assert_eq!(args[0].kind, ArgKind::Count);
}

// =========================================================================
// RepeatDot tests
// =========================================================================

#[test]
fn test_repeat_dot_id() {
    let cmd = RepeatDot;
    assert_eq!(cmd.id().name(), "repeat-dot");
}

#[test]
fn test_repeat_dot_description() {
    let cmd = RepeatDot;
    assert_eq!(cmd.description(), "Repeat last change");
}

#[test]
fn test_repeat_dot_args() {
    let cmd = RepeatDot;
    let args = cmd.args();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].name, "count");
    assert_eq!(args[0].kind, ArgKind::Count);
}

// =========================================================================
// JoinLines tests
// =========================================================================

#[test]
fn test_join_lines_id() {
    let cmd = JoinLines;
    assert_eq!(cmd.id().name(), "join-lines");
}

#[test]
fn test_join_lines_description() {
    let cmd = JoinLines;
    assert_eq!(cmd.description(), "Join current line with next line");
}

#[test]
fn test_join_lines_args() {
    let cmd = JoinLines;
    let args = cmd.args();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].name, "count");
    assert_eq!(args[0].kind, ArgKind::Count);
}

#[test]
fn test_join_lines_no_buffer_returns_error() {
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
    let result = JoinLines.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_join_lines_no_window_returns_error() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("line 1\nline 2");
    let buffer_id = register_buffer(&kernel, buffer);
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
    let result = JoinLines.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_join_lines_two_lines() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = JoinLines.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let buf_read = buf.read();
    assert_eq!(buf_read.line_count(), 1);
    assert_eq!(buf_read.line(0).as_deref(), Some("hello world"));
    drop(buf_read);
}

#[test]
fn test_join_lines_strips_leading_whitespace() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello\n    world");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = JoinLines.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let buf_read = buf.read();
    assert_eq!(buf_read.line_count(), 1);
    assert_eq!(buf_read.line(0).as_deref(), Some("hello world"));
    drop(buf_read);
}

#[test]
fn test_join_lines_on_last_line_is_noop() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("only line");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = JoinLines.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let buf_read = buf.read();
    assert_eq!(buf_read.line_count(), 1);
    assert_eq!(buf_read.line(0).as_deref(), Some("only line"));
    drop(buf_read);
}

#[test]
fn test_join_lines_with_count() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("line 1\nline 2\nline 3\nline 4");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));

    let result = JoinLines.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let buf_read = buf.read();
    assert_eq!(buf_read.line_count(), 2);
    // First two joins happened: "line 1" + "line 2" + "line 3"
    assert_eq!(buf_read.line(0).as_deref(), Some("line 1 line 2 line 3"));
    assert_eq!(buf_read.line(1).as_deref(), Some("line 4"));
    drop(buf_read);
}

#[test]
fn test_join_lines_count_exceeds_remaining() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("line 1\nline 2");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(10));

    let result = JoinLines.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let buf_read = buf.read();
    assert_eq!(buf_read.line_count(), 1);
    assert_eq!(buf_read.line(0).as_deref(), Some("line 1 line 2"));
    drop(buf_read);
}

#[test]
fn test_join_lines_buffer_not_found() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    // Use a fake buffer_id that doesn't exist
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(99999));

    let result = JoinLines.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_join_lines_no_content_after_join_point() {
    // Tests the branch at line 179: new_line_len > line_len check
    // where there's no content after joining (empty next line)
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello\n");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = JoinLines.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let buf_read = buf.read();
    assert_eq!(buf_read.line_count(), 1);
    // Empty second line joined, no space should be inserted
    assert_eq!(buf_read.line(0).as_deref(), Some("hello"));
    drop(buf_read);
}

#[test]
fn test_replace_char_start_execute() {
    // Execute returns Success even though it's a stub
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let args = CommandContext::new();
    let result = ReplaceCharStart.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_repeat_dot_execute() {
    // Execute returns Success even though it's a stub
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let args = CommandContext::new();
    let result = RepeatDot.execute(&mut runtime, &args);
    assert!(result.is_success());
}

// =========================================================================
// Metadata/Default tests
// =========================================================================

// =========================================================================
// ReplaceCharStart with count
// =========================================================================

#[test]
fn test_replace_char_start_with_count() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set("count", ArgValue::Count(3));
    let result = ReplaceCharStart.execute(&mut runtime, &args);
    assert!(result.is_success());
}

// =========================================================================
// JoinLines cursor position after join
// =========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_join_lines_cursor_position() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 3).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = JoinLines.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let buf_read = buf.read();
    assert_eq!(buf_read.line(0).as_deref(), Some("hello world"));
    drop(buf_read);

    drop(runtime);
    let window = state.windows.active().unwrap();
    // Cursor should be at min(original_col, final_line_len - 1)
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 3);
}

// =========================================================================
// JoinLines with multiple empty lines
// =========================================================================

#[test]
fn test_join_lines_with_empty_next_line() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello\n\nworld");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = JoinLines.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let buf_read = buf.read();
    assert_eq!(buf_read.line_count(), 2);
    // Empty line joined with no trailing space (no content after join)
    assert_eq!(buf_read.line(0).as_deref(), Some("hello"));
    assert_eq!(buf_read.line(1).as_deref(), Some("world"));
    drop(buf_read);
}

// =========================================================================
// JoinLines with multiple joins (count=3 on 4 lines)
// =========================================================================

#[test]
fn test_join_lines_all_lines() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("a\nb\nc\nd");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(3));

    let result = JoinLines.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let buf_read = buf.read();
    assert_eq!(buf_read.line_count(), 1);
    assert_eq!(buf_read.line(0).as_deref(), Some("a b c d"));
    drop(buf_read);
}

// =========================================================================
// JoinLines from non-first line
// =========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_join_lines_from_middle() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("line 1\nline 2\nline 3");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(1, 0).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = JoinLines.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let buf_read = buf.read();
    assert_eq!(buf_read.line_count(), 2);
    assert_eq!(buf_read.line(0).as_deref(), Some("line 1"));
    assert_eq!(buf_read.line(1).as_deref(), Some("line 2 line 3"));
    drop(buf_read);
}

// =========================================================================
// JoinLines on last line of multi-line buffer
// =========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_join_lines_on_last_line_multiline() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("line 1\nline 2\nline 3");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(2, 0).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = JoinLines.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Should be no-op since cursor is on last line
    let buf = text_buf(&kernel, buffer_id);
    let buf_read = buf.read();
    assert_eq!(buf_read.line_count(), 3);
    drop(buf_read);
}

// =========================================================================
// RepeatDot with count
// =========================================================================

#[test]
fn test_repeat_dot_with_count() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set("count", ArgValue::Count(5));
    let result = RepeatDot.execute(&mut runtime, &args);
    assert!(result.is_success());
}

// =========================================================================
// JoinLines with tab-indented next line
// =========================================================================

#[test]
fn test_join_lines_strips_tab_whitespace() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello\n\t\tworld");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = JoinLines.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let buf_read = buf.read();
    assert_eq!(buf_read.line_count(), 1);
    assert_eq!(buf_read.line(0).as_deref(), Some("hello world"));
    drop(buf_read);
}

// =========================================================================
// ReplaceChar tests
// =========================================================================

#[test]
fn test_replace_char_id() {
    let cmd = ReplaceChar;
    assert_eq!(cmd.id().name(), "replace-char");
}

#[test]
fn test_replace_char_description() {
    let cmd = ReplaceChar;
    assert_eq!(cmd.description(), "Replace character(s) under cursor");
}

#[test]
fn test_replace_char_args() {
    let cmd = ReplaceChar;
    let args = cmd.args();
    assert_eq!(args.len(), 2);
    assert_eq!(args[0].name, "replace_char");
    assert_eq!(args[0].kind, ArgKind::Char);
    assert_eq!(args[1].name, "count");
    assert_eq!(args[1].kind, ArgKind::Count);
}

#[test]
fn test_replace_char_no_char_arg_error() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = ReplaceChar.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_replace_char_no_buffer_error() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set("replace_char", ArgValue::Char('x'));
    let result = ReplaceChar.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_replace_char_no_window_error() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
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
    args.set("replace_char", ArgValue::Char('x'));
    args.set_buffer_id(buffer_id);
    let result = ReplaceChar.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_replace_char_single() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("replace_char", ArgValue::Char('x'));

    let result = ReplaceChar.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let buf_read = buf.read();
    assert_eq!(buf_read.line(0).as_deref(), Some("xello"));
    drop(buf_read);
}

#[test]
fn test_replace_char_with_count() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("replace_char", ArgValue::Char('z'));
    args.set("count", ArgValue::Count(3));

    let result = ReplaceChar.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let buf_read = buf.read();
    assert_eq!(buf_read.line(0).as_deref(), Some("zzzlo"));
    drop(buf_read);
}

#[test]
fn test_replace_char_count_clamps_to_line_end() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hi");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("replace_char", ArgValue::Char('x'));
    args.set("count", ArgValue::Count(10));

    let result = ReplaceChar.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let buf_read = buf.read();
    // Count clamped to 2 (line length)
    assert_eq!(buf_read.line(0).as_deref(), Some("xx"));
    drop(buf_read);
}

#[test]
fn test_replace_char_on_empty_line() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("replace_char", ArgValue::Char('x'));

    let result = ReplaceChar.execute(&mut runtime, &args);
    assert!(result.is_success());

    // No change on empty buffer (line_len == 0 early return)
    let buf = text_buf(&kernel, buffer_id);
    let buf_read = buf.read();
    assert_eq!(buf_read.line_count(), 0);
    drop(buf_read);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_replace_char_at_column_offset() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 3).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("replace_char", ArgValue::Char('X'));

    let result = ReplaceChar.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let buf_read = buf.read();
    assert_eq!(buf_read.line(0).as_deref(), Some("helXo"));
    drop(buf_read);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_replace_char_at_end_of_line() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 4).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("replace_char", ArgValue::Char('!'));

    let result = ReplaceChar.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let buf_read = buf.read();
    assert_eq!(buf_read.line(0).as_deref(), Some("hell!"));
    drop(buf_read);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_replace_char_cursor_past_end() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hi");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 5).into(); // Past end
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("replace_char", ArgValue::Char('x'));

    let result = ReplaceChar.execute(&mut runtime, &args);
    assert!(result.is_success());

    // No change since cursor is past end
    let buf = text_buf(&kernel, buffer_id);
    let buf_read = buf.read();
    assert_eq!(buf_read.line(0).as_deref(), Some("hi"));
    drop(buf_read);
}
