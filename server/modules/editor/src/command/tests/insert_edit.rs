use {
    super::*,
    reovim_driver_command::{Command, CommandContext},
    reovim_driver_session::{
        ClientId, ExtensionMap, Session, SessionRuntime, Window, WindowLayout,
        testing::StubExecutor,
    },
    reovim_kernel::{
        api::v1::{
            Buffer, BufferId, HistoryRing, KernelContext, MarkBank, ModeStack, OptionScope,
            OptionSpec, OptionValue, RegisterBank,
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
                active_buffer: &mut self.active_buffer,
                terminal_size: &mut self.terminal_size,
            },
            kernel,
            executor,
        )
    }
}

// =========================================================================
// get_line_indent tests
// =========================================================================

#[test]
fn test_get_line_indent_no_indent() {
    assert_eq!(get_line_indent("hello"), "");
}

#[test]
fn test_get_line_indent_spaces() {
    assert_eq!(get_line_indent("    hello"), "    ");
}

#[test]
fn test_get_line_indent_tabs() {
    assert_eq!(get_line_indent("\t\thello"), "\t\t");
}

#[test]
fn test_get_line_indent_mixed() {
    assert_eq!(get_line_indent("  \t hello"), "  \t ");
}

#[test]
fn test_get_line_indent_all_whitespace() {
    assert_eq!(get_line_indent("   "), "   ");
}

#[test]
fn test_get_line_indent_empty() {
    assert_eq!(get_line_indent(""), "");
}

// =========================================================================
// InsertNewline tests
// =========================================================================

#[test]
fn test_insert_newline_id() {
    let cmd = InsertNewline;
    assert_eq!(cmd.id().name(), "insert-newline");
}

#[test]
fn test_insert_newline_description() {
    let cmd = InsertNewline;
    assert_eq!(cmd.description(), "Insert newline at cursor position");
}

#[test]
fn test_insert_newline_no_buffer_returns_error() {
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
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    let args = CommandContext::new();
    let result = InsertNewline.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_insert_newline_no_window_returns_error() {
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
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = InsertNewline.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_insert_newline_at_end() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 5).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InsertNewline.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let buf_read = buf.read();
    assert_eq!(buf_read.line_count(), 2);
    assert_eq!(buf_read.line(0), Some("hello"));
    assert_eq!(buf_read.line(1), Some(""));

    drop(buf_read);
    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 1);
    assert_eq!(window.cursor.column, 0);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_insert_newline_at_middle() {
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

    let result = InsertNewline.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let buf_read = buf.read();
    assert_eq!(buf_read.line_count(), 2);
    assert_eq!(buf_read.line(0), Some("hello"));
    assert_eq!(buf_read.line(1), Some(" world"));
    drop(buf_read);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_insert_newline_with_autoindent() {
    let kernel = create_test_context();

    // Register the option spec so set() can find it
    let _ = kernel.options.register(
        OptionSpec::new("autoindent", "Auto indent new lines", OptionValue::bool(true))
            .with_scope(OptionScope::Buffer),
    );

    let buffer = Buffer::from_string("    indented line");
    let buffer_id = kernel.buffers.register(buffer);

    kernel
        .options
        .set("autoindent", OptionValue::bool(true), OptionScopeId::Buffer(buffer_id))
        .expect("autoindent option should be settable");

    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 17).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InsertNewline.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let buf_read = buf.read();
    assert_eq!(buf_read.line_count(), 2);
    assert_eq!(buf_read.line(1), Some("    "));

    drop(buf_read);
    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 1);
    assert_eq!(window.cursor.column, 4);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_insert_newline_without_autoindent() {
    let kernel = create_test_context();

    // Register the option spec so set() can find it
    let _ = kernel.options.register(
        OptionSpec::new("autoindent", "Auto indent new lines", OptionValue::bool(true))
            .with_scope(OptionScope::Buffer),
    );

    let buffer = Buffer::from_string("    indented line");
    let buffer_id = kernel.buffers.register(buffer);

    kernel
        .options
        .set("autoindent", OptionValue::bool(false), OptionScopeId::Buffer(buffer_id))
        .expect("autoindent option should be settable");

    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 17).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InsertNewline.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let buf_read = buf.read();
    assert_eq!(buf_read.line(1), Some(""));

    drop(buf_read);
    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 1);
    assert_eq!(window.cursor.column, 0);
}

// =========================================================================
// InsertTab tests
// =========================================================================

#[test]
fn test_insert_tab_id() {
    let cmd = InsertTab;
    assert_eq!(cmd.id().name(), "insert-tab");
}

#[test]
fn test_insert_tab_description() {
    let cmd = InsertTab;
    assert_eq!(cmd.description(), "Insert tab at cursor position");
}

#[test]
fn test_insert_tab_no_buffer_returns_error() {
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
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    let args = CommandContext::new();
    let result = InsertTab.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_insert_tab_no_window_returns_error() {
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
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = InsertTab.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_insert_tab_expandtab() {
    let kernel = create_test_context();

    // Register the option specs so set() can find them
    let _ = kernel.options.register(
        OptionSpec::new("expandtab", "Use spaces instead of tabs", OptionValue::bool(true))
            .with_scope(OptionScope::Buffer),
    );
    let _ = kernel.options.register(
        OptionSpec::new("tabstop", "Number of spaces per tab", OptionValue::int(4))
            .with_scope(OptionScope::Buffer),
    );

    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(buffer);

    kernel
        .options
        .set("expandtab", OptionValue::bool(true), OptionScopeId::Buffer(buffer_id))
        .expect("expandtab option should be settable");
    kernel
        .options
        .set("tabstop", OptionValue::int(4), OptionScopeId::Buffer(buffer_id))
        .expect("tabstop option should be settable");

    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InsertTab.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let content = buf.read().line(0).map(str::to_owned);
    assert_eq!(content.as_deref(), Some("    hello"));

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.column, 4);
}

#[test]
fn test_insert_tab_noexpandtab() {
    let kernel = create_test_context();

    // Register the option spec so set() can find it
    let _ = kernel.options.register(
        OptionSpec::new("expandtab", "Use spaces instead of tabs", OptionValue::bool(true))
            .with_scope(OptionScope::Buffer),
    );

    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(buffer);

    kernel
        .options
        .set("expandtab", OptionValue::bool(false), OptionScopeId::Buffer(buffer_id))
        .expect("expandtab option should be settable");

    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InsertTab.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let content = buf.read().line(0).map(str::to_owned);
    assert_eq!(content.as_deref(), Some("\thello"));

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.column, 1);
}

// =========================================================================
// InsertNewline at beginning of line
// =========================================================================

#[test]
fn test_insert_newline_at_beginning() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(buffer);
    let mut state = TestState::with_window(buffer_id);
    // Cursor at column 0
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InsertNewline.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let buf_read = buf.read();
    assert_eq!(buf_read.line_count(), 2);
    assert_eq!(buf_read.line(0), Some(""));
    assert_eq!(buf_read.line(1), Some("hello"));

    drop(buf_read);
    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 1);
    assert_eq!(window.cursor.column, 0);
}

// =========================================================================
// InsertTab with custom tabstop
// =========================================================================

#[test]
fn test_insert_tab_expandtab_custom_tabstop() {
    let kernel = create_test_context();

    let _ = kernel.options.register(
        OptionSpec::new("expandtab", "Use spaces instead of tabs", OptionValue::bool(true))
            .with_scope(OptionScope::Buffer),
    );
    let _ = kernel.options.register(
        OptionSpec::new("tabstop", "Number of spaces per tab", OptionValue::int(4))
            .with_scope(OptionScope::Buffer),
    );

    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(buffer);

    kernel
        .options
        .set("expandtab", OptionValue::bool(true), OptionScopeId::Buffer(buffer_id))
        .expect("expandtab option should be settable");
    kernel
        .options
        .set("tabstop", OptionValue::int(2), OptionScopeId::Buffer(buffer_id))
        .expect("tabstop option should be settable");

    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InsertTab.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let content = buf.read().line(0).map(str::to_owned);
    // tabstop=2, expandtab=true -> 2 spaces
    assert_eq!(content.as_deref(), Some("  hello"));

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.column, 2);
}

// =========================================================================
// InsertTab default (no options set, should use defaults)
// =========================================================================

#[test]
fn test_insert_tab_default_options() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InsertTab.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let content = buf.read().line(0).map(str::to_owned);
    // Default: expandtab=true, tabstop=4 -> 4 spaces
    assert_eq!(content.as_deref(), Some("    hello"));

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.column, 4);
}

// =========================================================================
// InsertNewline in multiline buffer
// =========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_insert_newline_in_middle_of_multiline() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("line 1\nline 2\nline 3");
    let buffer_id = kernel.buffers.register(buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(1, 4).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InsertNewline.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let buf_read = buf.read();
    assert_eq!(buf_read.line_count(), 4);
    assert_eq!(buf_read.line(1), Some("line"));
    assert_eq!(buf_read.line(2), Some(" 2"));
    drop(buf_read);

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 2);
    assert_eq!(window.cursor.column, 0);
}

// =========================================================================
// InsertTab in middle of text
// =========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_insert_tab_in_middle_of_text() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("helloworld");
    let buffer_id = kernel.buffers.register(buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 5).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InsertTab.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let content = buf.read().line(0).map(str::to_owned);
    // Default: expandtab=true, tabstop=4 -> 4 spaces
    assert_eq!(content.as_deref(), Some("hello    world"));

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.column, 9); // 5 + 4
}

// =========================================================================
// Metadata tests
// =========================================================================

#[test]
fn test_insert_newline_args_empty() {
    let cmd = InsertNewline;
    let args = cmd.args();
    assert!(args.is_empty());
}

#[test]
fn test_insert_tab_args_empty() {
    let cmd = InsertTab;
    let args = cmd.args();
    assert!(args.is_empty());
}
