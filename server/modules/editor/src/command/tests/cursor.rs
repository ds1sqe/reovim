use {
    super::super::*,
    reovim_driver_command::{ArgKind, ArgValue, Command, CommandContext},
    reovim_driver_session::{
        ClientId, ExtensionMap, Session, SessionRuntime, Window, WindowLayout,
        api::{CommandExecutor, Selection, SelectionMode},
        testing::StubExecutor,
    },
    reovim_kernel::{
        api::v1::{Buffer, BufferId, Jumplist, KernelContext, MarkBank, ModeStack, RwLock},
        testing::{create_test_context, test_mode},
    },
    reovim_types_text::{HistoryRing, Position, RegisterBank},
    std::sync::Arc,
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
        executor: &'a dyn CommandExecutor,
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
// No-window error tests
// =========================================================================

#[test]
fn test_cursor_up_no_window_returns_error() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
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
    assert!(CursorUp.execute(&mut runtime, &args).is_error());
}

#[test]
fn test_cursor_down_no_window_returns_error() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
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
    assert!(CursorDown.execute(&mut runtime, &args).is_error());
}

#[test]
fn test_cursor_left_no_window_returns_error() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
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
    assert!(CursorLeft.execute(&mut runtime, &args).is_error());
}

#[test]
fn test_cursor_right_no_window_returns_error() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
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
    assert!(CursorRight.execute(&mut runtime, &args).is_error());
}

// =========================================================================
// Buffer not found (CursorDown)
// =========================================================================

#[test]
fn test_cursor_down_buffer_not_found() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(99999));
    assert!(CursorDown.execute(&mut runtime, &args).is_error());
}

// =========================================================================
// Operator-pending mode tests
// =========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_cursor_up_operator_pending() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("line 1\nline 2\nline 3");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(2, 0).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set_mode_name("operator-pending");
    assert!(CursorUp.execute(&mut runtime, &args).is_success());
}

#[test]
fn test_cursor_down_operator_pending() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("line 1\nline 2\nline 3");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set_mode_name("operator-pending");
    assert!(CursorDown.execute(&mut runtime, &args).is_success());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_cursor_left_operator_pending() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 5).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set_mode_name("operator-pending");
    assert!(CursorLeft.execute(&mut runtime, &args).is_success());
}

#[test]
fn test_cursor_right_operator_pending() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set_mode_name("operator-pending");
    assert!(CursorRight.execute(&mut runtime, &args).is_success());
}

// =========================================================================
// Visual mode selection extension tests
// =========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_cursor_up_visual_mode_extends_selection() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("line 1\nline 2\nline 3");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(1, 0).into();
        window.selection = Some(Selection::new(
            Position::new(0, 0),
            Position::new(1, 1),
            SelectionMode::Character,
        ));
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    assert!(CursorUp.execute(&mut runtime, &args).is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 0);
    let sel = window.selection.as_ref().unwrap();
    // sel.end = Position::new(new_pos.line=0, new_pos.column=0 + 1=1)
    assert_eq!(sel.end, Position::new(0, 1));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_cursor_down_visual_mode_extends_selection() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("line 1\nline 2\nline 3");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.selection = Some(Selection::new(
            Position::new(0, 0),
            Position::new(0, 1),
            SelectionMode::Character,
        ));
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    assert!(CursorDown.execute(&mut runtime, &args).is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 1);
    let sel = window.selection.as_ref().unwrap();
    // sel.end = Position::new(new_pos.line=1, new_pos.column=0 + 1=1)
    assert_eq!(sel.end, Position::new(1, 1));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_cursor_left_visual_mode_extends_selection() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 5).into();
        window.selection = Some(Selection::new(
            Position::new(0, 0),
            Position::new(0, 6),
            SelectionMode::Character,
        ));
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    assert!(CursorLeft.execute(&mut runtime, &args).is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.column, 4);
    let sel = window.selection.as_ref().unwrap();
    // sel.end = Position::new(0, 4 + 1 = 5)
    assert_eq!(sel.end, Position::new(0, 5));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_cursor_right_visual_mode_extends_selection() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.selection = Some(Selection::new(
            Position::new(0, 0),
            Position::new(0, 1),
            SelectionMode::Character,
        ));
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    assert!(CursorRight.execute(&mut runtime, &args).is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.column, 1);
    let sel = window.selection.as_ref().unwrap();
    // sel.end = Position::new(0, 1 + 1 = 2)
    assert_eq!(sel.end, Position::new(0, 2));
}

// =========================================================================
// Description and args tests
// =========================================================================

#[test]
fn test_cursor_descriptions() {
    assert_eq!(CursorUp.description(), "Move cursor up");
    assert_eq!(CursorDown.description(), "Move cursor down");
    assert_eq!(CursorLeft.description(), "Move cursor left");
    assert_eq!(CursorRight.description(), "Move cursor right");
}

#[test]
fn test_cursor_args_all_have_count() {
    let commands: Vec<Box<dyn CommandHandler>> = vec![
        Box::new(CursorUp),
        Box::new(CursorDown),
        Box::new(CursorLeft),
        Box::new(CursorRight),
    ];
    for cmd in &commands {
        let args = cmd.args();
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
    }
}

// =========================================================================
// Cursor movement with count
// =========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_cursor_up_with_count() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("line 1\nline 2\nline 3\nline 4");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(3, 0).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));

    let result = CursorUp.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 1);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_cursor_up_count_exceeds_lines_clamps_to_zero() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("line 1\nline 2\nline 3");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(1, 0).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(100));

    let result = CursorUp.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 0);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_cursor_left_with_count() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 8).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(3));

    let result = CursorLeft.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.column, 5);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_cursor_left_count_exceeds_column_clamps_to_zero() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 2).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(100));

    let result = CursorLeft.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.column, 0);
}

// =========================================================================
// Cursor up/down column clamping to shorter lines
// =========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_cursor_down_clamps_column_to_shorter_line() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("long line here\nhi");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 10).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = CursorDown.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 1);
    // "hi" has len 2, valid columns 0-1, so column clamped to 1 (#552)
    assert_eq!(window.cursor.column, 1);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_cursor_up_clamps_column_to_shorter_line() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hi\nlong line here");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(1, 10).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = CursorUp.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 0);
    // "hi" has len 2, valid columns 0-1, so column clamped to 1 (#552)
    assert_eq!(window.cursor.column, 1);
}

// =========================================================================
// Cursor down with count
// =========================================================================

#[test]
fn test_cursor_down_with_count() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("line 1\nline 2\nline 3\nline 4");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(3));

    let result = CursorDown.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 3);
}

#[test]
fn test_cursor_down_count_exceeds_lines_clamps_to_last() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("line 1\nline 2");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(100));

    let result = CursorDown.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 1);
}

// =========================================================================
// Cursor right with count
// =========================================================================

#[test]
fn test_cursor_right_with_count() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(5));

    let result = CursorRight.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.column, 5);
}

#[test]
fn test_cursor_right_count_exceeds_line_clamps_to_last_char() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hi");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(100));

    let result = CursorRight.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    // "hi" has len 2, max_col = 1 (last char index)
    assert_eq!(window.cursor.column, 1);
}

// =========================================================================
// No buffer_id tests
// =========================================================================

#[test]
fn test_cursor_up_no_buffer_id_returns_error() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let args = CommandContext::new();
    assert!(CursorUp.execute(&mut runtime, &args).is_error());
}

#[test]
fn test_cursor_down_no_buffer_id_returns_error() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let args = CommandContext::new();
    assert!(CursorDown.execute(&mut runtime, &args).is_error());
}

#[test]
fn test_cursor_left_no_buffer_id_returns_error() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let args = CommandContext::new();
    assert!(CursorLeft.execute(&mut runtime, &args).is_error());
}

#[test]
fn test_cursor_right_no_buffer_id_returns_error() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let args = CommandContext::new();
    assert!(CursorRight.execute(&mut runtime, &args).is_error());
}

// =========================================================================
// Cursor IDs
// =========================================================================

#[test]
fn test_cursor_ids() {
    assert_eq!(CursorUp.id().name(), "cursor-up");
    assert_eq!(CursorDown.id().name(), "cursor-down");
    assert_eq!(CursorLeft.id().name(), "cursor-left");
    assert_eq!(CursorRight.id().name(), "cursor-right");
}

// =========================================================================
// Empty line handling for cursor right
// =========================================================================

#[test]
fn test_cursor_right_on_empty_line() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = CursorRight.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.column, 0);
}

// =========================================================================
// Cursor down on empty buffer
// =========================================================================

#[test]
fn test_cursor_down_empty_buffer() {
    let kernel = create_test_context();
    let buffer = Buffer::new();
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = CursorDown.execute(&mut runtime, &args);
    assert!(result.is_success());
}
