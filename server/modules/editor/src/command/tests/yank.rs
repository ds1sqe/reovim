use {
    super::super::*,
    reovim_driver_command::{ArgKind, ArgValue, Command, CommandContext},
    reovim_driver_session::{
        ClientId, ExtensionMap, Jumplist, MarkBank, Session, SessionRuntime, TextBufferRegistry,
        Window, WindowLayout, testing::StubExecutor,
    },
    reovim_kernel::{
        api::v1::{BufferId, KernelBuffer, KernelContext, ModeStack, RwLock},
        testing::{create_test_context, test_mode},
    },
    reovim_provider_text::{Buffer, BufferOps},
    reovim_types_text::{HistoryRing, Position, RegisterBank},
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
// Command trait tests
// =========================================================================

#[test]
fn test_yank_line_id() {
    let cmd = YankLine;
    assert_eq!(cmd.id().name(), "yank-line");
}

#[test]
fn test_yank_line_description() {
    assert_eq!(YankLine.description(), "Yank current line");
}

#[test]
fn test_yank_line_args() {
    let args = YankLine.args();
    assert_eq!(args.len(), 2);
    assert_eq!(args[0].name, "count");
    assert_eq!(args[0].kind, ArgKind::Count);
    assert_eq!(args[1].name, "register");
    assert_eq!(args[1].kind, ArgKind::Register);
}

#[test]
fn test_yank_line_default() {
    let cmd = YankLine;
    assert_eq!(cmd.description(), "Yank current line");
}

// =========================================================================
// Execute tests
// =========================================================================

#[test]
fn test_yank_no_buffer_returns_error() {
    let kernel = setup_kernel();
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
    let result = YankLine.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_yank_no_window_returns_error() {
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
    let result = YankLine.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_yank_buffer_not_found_returns_error() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(99999));
    let result = YankLine.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_yank_empty_buffer_returns_success() {
    let kernel = setup_kernel();
    let buffer = Buffer::new();
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    // Empty buffer, line_count == 0, returns Success early
    let result = YankLine.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_yank_single_line() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = YankLine.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Check register content
    drop(runtime);
    let content = state.registers.get().clone();
    assert!(content.is_linewise());
    assert_eq!(content.text, "hello world\n");
}

#[test]
fn test_yank_with_count() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("line 1\nline 2\nline 3");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));

    let result = YankLine.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let content = state.registers.get().clone();
    assert!(content.is_linewise());
    assert_eq!(content.text, "line 1\nline 2\n");
}

#[test]
fn test_yank_count_clamped_to_eof() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("line 1\nline 2");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(100));

    let result = YankLine.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let content = state.registers.get().clone();
    assert!(content.is_linewise());
    assert_eq!(content.text, "line 1\nline 2\n");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_yank_from_cursor_line() {
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

    let result = YankLine.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let content = state.registers.get().clone();
    assert!(content.is_linewise());
    assert_eq!(content.text, "line 2\n");
}

// =========================================================================
// Metadata tests
// =========================================================================

#[test]
fn test_yank_line_debug() {
    let cmd = YankLine;
    let debug_str = format!("{cmd:?}");
    assert!(debug_str.contains("YankLine"));
}

// =========================================================================
// Yank to named register
// =========================================================================

#[test]
fn test_yank_to_named_register() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("register", ArgValue::Register('a'));

    let result = YankLine.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Check named register content (per-client registers, #515)
    drop(runtime);
    let content = state.registers.get_by_name(Some('a')).cloned();
    let content = content.unwrap();
    assert!(content.is_linewise());
    assert_eq!(content.text, "hello world\n");
}

// =========================================================================
// Yank multiple lines from middle
// =========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_yank_multiple_lines_from_middle() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("line 1\nline 2\nline 3\nline 4\nline 5");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(1, 3).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(3));

    let result = YankLine.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let content = state.registers.get().clone();
    assert!(content.is_linewise());
    assert_eq!(content.text, "line 2\nline 3\nline 4\n");
}

// =========================================================================
// Yank last line
// =========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_yank_last_line() {
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

    let result = YankLine.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let content = state.registers.get().clone();
    assert!(content.is_linewise());
    assert_eq!(content.text, "line 3\n");
}

// =========================================================================
// Yank with cursor column > 0 (still yanks full line)
// =========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_yank_line_with_cursor_at_column() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 5).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = YankLine.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Yank line should yank the entire line regardless of column
    drop(runtime);
    let content = state.registers.get().clone();
    assert!(content.is_linewise());
    assert_eq!(content.text, "hello world\n");
}
