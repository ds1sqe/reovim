use {
    super::super::*,
    reovim_domain_text::{HistoryRing, Position, RegisterBank},
    reovim_driver_command::{ArgKind, ArgValue, Command, CommandContext},
    reovim_driver_session::{
        ClientId, ExtensionMap, Jumplist, MarkBank, Session, SessionRuntime, Window, WindowLayout,
        testing::StubExecutor,
    },
    reovim_kernel::{
        api::v1::{BufferId, KernelBuffer, KernelContext, ModeStack, RwLock},
        testing::{create_test_context, test_mode},
    },
    reovim_provider_text::{Buffer, BufferOps, TextBufferRegistry},
    std::sync::Arc,
};

/// Create a test kernel with a `TextBufferRegistry` in services.
#[cfg_attr(coverage_nightly, coverage(off))]
fn setup_kernel() -> KernelContext {
    let kernel = create_test_context();
    kernel
        .services
        .register(Arc::new(TextBufferRegistry::new()));
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
// CursorDisplayDown tests
// =========================================================================

#[test]
fn test_cursor_display_down_id() {
    let cmd = CursorDisplayDown;
    assert_eq!(cmd.id().name(), "cursor-display-down");
}

#[test]
fn test_cursor_display_down_description() {
    let cmd = CursorDisplayDown;
    assert_eq!(cmd.description(), "Move cursor down one display line");
}

#[test]
fn test_cursor_display_down_args() {
    let cmd = CursorDisplayDown;
    let args = cmd.args();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].name, "count");
    assert_eq!(args[0].kind, ArgKind::Count);
}

#[test]
fn test_cursor_display_down_no_buffer_returns_error() {
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
    let result = CursorDisplayDown.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_cursor_display_down_no_window_returns_error() {
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
    args.set_buffer_id(buffer_id);
    let result = CursorDisplayDown.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_cursor_display_down_moves_to_next_line() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("line 1\nline 2\nline 3");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = CursorDisplayDown.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 1);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_cursor_display_down_at_eof() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("line 1\nline 2");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(1, 0).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = CursorDisplayDown.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 1);
}

#[test]
fn test_cursor_display_down_with_count() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("line 1\nline 2\nline 3\nline 4");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(3));

    let result = CursorDisplayDown.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 3);
}

#[test]
fn test_cursor_display_down_buffer_not_found() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(99999));

    let result = CursorDisplayDown.execute(&mut runtime, &args);
    assert!(result.is_error());
}

// =========================================================================
// CursorDisplayUp tests
// =========================================================================

#[test]
fn test_cursor_display_up_id() {
    let cmd = CursorDisplayUp;
    assert_eq!(cmd.id().name(), "cursor-display-up");
}

#[test]
fn test_cursor_display_up_description() {
    let cmd = CursorDisplayUp;
    assert_eq!(cmd.description(), "Move cursor up one display line");
}

#[test]
fn test_cursor_display_up_args() {
    let cmd = CursorDisplayUp;
    let args = cmd.args();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].name, "count");
    assert_eq!(args[0].kind, ArgKind::Count);
}

#[test]
fn test_cursor_display_up_no_buffer_returns_error() {
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
    let result = CursorDisplayUp.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_cursor_display_up_no_window_returns_error() {
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
    args.set_buffer_id(buffer_id);
    let result = CursorDisplayUp.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_cursor_display_up_moves_to_prev_line() {
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

    let result = CursorDisplayUp.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 1);
}

#[test]
fn test_cursor_display_up_at_top() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("line 1\nline 2");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = CursorDisplayUp.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 0);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_cursor_display_up_with_count() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("line 1\nline 2\nline 3\nline 4");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(3, 0).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));

    let result = CursorDisplayUp.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 1);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_cursor_display_up_count_exceeds_remaining() {
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
    args.set("count", ArgValue::Count(100));

    let result = CursorDisplayUp.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 0);
}

// =========================================================================
// Wrapped line movement (within same buffer line)
// =========================================================================

#[test]
fn test_cursor_display_down_within_wrapped_line() {
    // Create a line longer than 80 columns so it wraps
    let long_line: String = "a".repeat(200);
    let kernel = setup_kernel();
    let buffer = Buffer::from_string(&long_line);
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    // Move down one display line - should stay on same buffer line
    let result = CursorDisplayDown.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    // Should still be on buffer line 0 but moved to column 80
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 80);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_cursor_display_up_within_wrapped_line() {
    // Create a line longer than 80 columns so it wraps
    let long_line: String = "a".repeat(200);
    let kernel = setup_kernel();
    let buffer = Buffer::from_string(&long_line);
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    // Set cursor to column 90 (display line 1, column 10)
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 90).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    // Move up one display line - should stay on same buffer line
    let result = CursorDisplayUp.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    // Should still be on buffer line 0 but moved back to column 10
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 10);
}

#[test]
fn test_cursor_display_down_count_exceeds_remaining() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("line 1\nline 2");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(100));

    let result = CursorDisplayDown.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    // Should clamp to last line
    assert_eq!(window.cursor.line, 1);
}

#[test]
fn test_cursor_display_down_through_wrapped_to_next_line() {
    // A long line followed by a short one: gj past the wrapped line
    let long_line = "a".repeat(200); // 3 display lines on 80-col terminal
    let content = format!("{long_line}\nshort");
    let kernel = setup_kernel();
    let buffer = Buffer::from_string(&content);
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(5)); // More than display lines in first line

    let result = CursorDisplayDown.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    // Should end up on buffer line 1 (the "short" line)
    assert_eq!(window.cursor.line, 1);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_cursor_display_up_through_wrapped_to_prev_line() {
    // A long line followed by a short one: gk from start of second line
    let long_line = "a".repeat(200); // 3 display lines on 80-col terminal
    let content = format!("{long_line}\nshort");
    let kernel = setup_kernel();
    let buffer = Buffer::from_string(&content);
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    // Set cursor to start of line 1
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(1, 0).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    // Move up one display line - should go to last display line of prev buffer line
    let result = CursorDisplayUp.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    // Should be on buffer line 0 (the long line), somewhere in wrapped region
    assert_eq!(window.cursor.line, 0);
    assert!(window.cursor.column >= 160); // In the last display line of the 200-char line
}

// =========================================================================
// Metadata tests
// =========================================================================

// =========================================================================
// Multiple wrapped lines movement tests
// =========================================================================

#[test]
fn test_cursor_display_down_across_multiple_wrapped_lines() {
    // Two long lines (each wraps): moving down should cross both
    let long_line1 = "a".repeat(200); // 3 display lines
    let long_line2 = "b".repeat(200); // 3 display lines
    let content = format!("{long_line1}\n{long_line2}");
    let kernel = setup_kernel();
    let buffer = Buffer::from_string(&content);
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    // Move down 4 display lines: 3 in first line (remaining) + 1 into second line
    args.set("count", ArgValue::Count(4));

    let result = CursorDisplayDown.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    // Should end up on buffer line 1 (second line)
    assert_eq!(window.cursor.line, 1);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_cursor_display_up_across_multiple_wrapped_lines() {
    // Two long lines: cursor on second line, move up several display lines
    let long_line1 = "a".repeat(200); // 3 display lines
    let long_line2 = "b".repeat(100); // 2 display lines
    let content = format!("{long_line1}\n{long_line2}");
    let kernel = setup_kernel();
    let buffer = Buffer::from_string(&content);
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(1, 90).into(); // display line 1 of second buffer line
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    // Move up 3 display lines: 1 up within second line + 2 into first line
    args.set("count", ArgValue::Count(3));

    let result = CursorDisplayUp.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    // Should be on buffer line 0 (first line)
    assert_eq!(window.cursor.line, 0);
}

// =========================================================================
// Display down on single-line buffer
// =========================================================================

#[test]
fn test_cursor_display_down_single_short_line() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("short");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = CursorDisplayDown.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    // Only one line, cursor stays at line 0
    assert_eq!(window.cursor.line, 0);
}

#[test]
fn test_cursor_display_up_single_short_line() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("short");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = CursorDisplayUp.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 0);
}

// =========================================================================
// Column preservation during display line movement
// =========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_cursor_display_down_preserves_display_column() {
    // A 200-char line: cursor at col 10, move down 1 display line
    // should end up at col 90 (display line 1, display col 10 -> buffer col 80+10)
    let long_line = "a".repeat(200);
    let kernel = setup_kernel();
    let buffer = Buffer::from_string(&long_line);
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 10).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = CursorDisplayDown.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 90);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_cursor_display_up_preserves_display_column() {
    // A 200-char line: cursor at col 170, move up 1 display line
    // display line 2, display col 10 -> display line 1, display col 10 -> buffer col 90
    let long_line = "a".repeat(200);
    let kernel = setup_kernel();
    let buffer = Buffer::from_string(&long_line);
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 170).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = CursorDisplayUp.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 90);
}

// =========================================================================
// No buffer_id returns error
// =========================================================================

#[test]
fn test_cursor_display_down_no_buffer_id() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let args = CommandContext::new();
    let result = CursorDisplayDown.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_cursor_display_down_within_wrapped_line_with_count() {
    // A 250-char line: move down 2 display lines within same buffer line
    let long_line: String = "a".repeat(250);
    let kernel = setup_kernel();
    let buffer = Buffer::from_string(&long_line);
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));

    let result = CursorDisplayDown.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    // Should stay on buffer line 0, move to display line 2 (col 160)
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 160);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_cursor_display_up_within_wrapped_line_with_count() {
    // A 250-char line: cursor at col 200 (display line 2), move up 2 display lines
    let long_line: String = "a".repeat(250);
    let kernel = setup_kernel();
    let buffer = Buffer::from_string(&long_line);
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 200).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));

    let result = CursorDisplayUp.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    // At col 200 = display line 2, display col 40
    // Move up 2 -> display line 0, display col 40 -> buffer col 40
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 40);
}

#[test]
fn test_cursor_display_down_crosses_multiple_buffer_lines() {
    // Two short lines then a long line: gj with large count crosses multiple buffer lines
    let content = "short1\nshort2\nshort3\nshort4";
    let kernel = setup_kernel();
    let buffer = Buffer::from_string(content);
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));

    let result = CursorDisplayDown.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    // Should end up on buffer line 2
    assert_eq!(window.cursor.line, 2);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_cursor_display_up_crosses_multiple_buffer_lines() {
    // Four short lines: cursor at line 3, gk with count=2 crosses multiple lines
    let content = "short1\nshort2\nshort3\nshort4";
    let kernel = setup_kernel();
    let buffer = Buffer::from_string(content);
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(3, 0).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));

    let result = CursorDisplayUp.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    // Should end up on buffer line 1
    assert_eq!(window.cursor.line, 1);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_cursor_display_down_wraps_into_next_buffer_line() {
    // Line 1: 200 chars (wraps to 3 display lines), Line 2: "short"
    // Cursor at col 160 (display line 2 of line 1), gj should go to line 2
    let long_line = "a".repeat(200);
    let content = format!("{long_line}\nshort");
    let kernel = setup_kernel();
    let buffer = Buffer::from_string(&content);
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 165).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = CursorDisplayDown.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    // At display line 2 (last display line of the 200-char line), gj crosses to line 1
    assert_eq!(window.cursor.line, 1);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_cursor_display_up_wraps_into_prev_buffer_line() {
    // Line 1: "short", Line 2: 200 chars (wraps to 3 display lines)
    // Cursor at col 5 (display line 0 of line 2), gk should go to line 1
    let long_line = "b".repeat(200);
    let content = format!("short\n{long_line}");
    let kernel = setup_kernel();
    let buffer = Buffer::from_string(&content);
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(1, 5).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = CursorDisplayUp.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    // Should end up on buffer line 0
    assert_eq!(window.cursor.line, 0);
}

#[test]
fn test_cursor_display_up_no_buffer_id() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let args = CommandContext::new();
    let result = CursorDisplayUp.execute(&mut runtime, &args);
    assert!(result.is_error());
}
