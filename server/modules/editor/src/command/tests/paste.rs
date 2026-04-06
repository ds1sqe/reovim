use {
    super::super::*,
    reovim_driver_command::{ArgKind, ArgValue, Command, CommandContext},
    reovim_driver_session::{
        ClientId, ExtensionMap, Jumplist, MarkBank, RegisterContent, Session, SessionRuntime,
        TextBufferRegistry, Window, WindowLayout, testing::StubExecutor,
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
// PasteAfter tests
// =========================================================================

#[test]
fn test_paste_after_id() {
    assert_eq!(PasteAfter.id().name(), "paste-after");
}

#[test]
fn test_paste_after_description() {
    assert_eq!(PasteAfter.description(), "Paste after cursor");
}

#[test]
fn test_paste_after_args() {
    let args = PasteAfter.args();
    assert_eq!(args.len(), 2);
    assert_eq!(args[0].name, "count");
    assert_eq!(args[0].kind, ArgKind::Count);
    assert_eq!(args[1].name, "register");
    assert_eq!(args[1].kind, ArgKind::Register);
}

#[test]
fn test_paste_after_no_buffer_returns_error() {
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
    let result = PasteAfter.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_paste_after_empty_register_returns_success() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = PasteAfter.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_paste_after_empty_content_returns_success() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    state.registers.set(RegisterContent::characterwise(""));
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = PasteAfter.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_paste_after_no_window_returns_error() {
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
    registers.set(RegisterContent::characterwise("X"));
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
    let result = PasteAfter.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_paste_after_linewise() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("line one\nline two");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    state.registers.set(RegisterContent::linewise("pasted\n"));
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = PasteAfter.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let buf_read = buf.read();
    assert_eq!(buf_read.line_count(), 3);
    assert_eq!(buf_read.line(0).as_deref(), Some("line one"));
    assert_eq!(buf_read.line(1).as_deref(), Some("pasted"));
    assert_eq!(buf_read.line(2).as_deref(), Some("line two"));
    drop(buf_read);

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 1);
    assert_eq!(window.cursor.column, 0);
}

#[test]
fn test_paste_after_linewise_empty_buffer() {
    let kernel = setup_kernel();
    let buffer = Buffer::new();
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    state
        .registers
        .set(RegisterContent::linewise("pasted line\n"));
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = PasteAfter.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 0);
}

#[test]
fn test_paste_after_characterwise() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    state.registers.set(RegisterContent::characterwise("XYZ"));
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = PasteAfter.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let content = buf.read().line(0).map(std::borrow::Cow::into_owned);
    assert_eq!(content.as_deref(), Some("hXYZello"));

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.column, 3);
}

#[test]
fn test_paste_after_characterwise_on_empty_line() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    state.registers.set(RegisterContent::characterwise("AB"));
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = PasteAfter.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let content = buf.read().line(0).map(std::borrow::Cow::into_owned);
    assert_eq!(content.as_deref(), Some("AB"));
}

#[test]
fn test_paste_after_characterwise_multiline() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    state.registers.set(RegisterContent::characterwise("X\nY"));
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = PasteAfter.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 1);
}

#[test]
fn test_paste_after_with_count() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    state.registers.set(RegisterContent::characterwise("X"));
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(3));
    let result = PasteAfter.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let content = buf.read().line(0).map(std::borrow::Cow::into_owned);
    assert_eq!(content.as_deref(), Some("hXXXello"));
}

#[test]
fn test_paste_after_linewise_with_count() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("line one");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    state.registers.set(RegisterContent::linewise("pasted\n"));
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));
    let result = PasteAfter.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let buf_read = buf.read();
    assert_eq!(buf_read.line_count(), 3);
    assert_eq!(buf_read.line(1).as_deref(), Some("pasted"));
    assert_eq!(buf_read.line(2).as_deref(), Some("pasted"));
    drop(buf_read);
}

// =========================================================================
// PasteBefore tests
// =========================================================================

#[test]
fn test_paste_before_id() {
    assert_eq!(PasteBefore.id().name(), "paste-before");
}

#[test]
fn test_paste_before_description() {
    assert_eq!(PasteBefore.description(), "Paste before cursor");
}

#[test]
fn test_paste_before_args() {
    let args = PasteBefore.args();
    assert_eq!(args.len(), 2);
    assert_eq!(args[0].name, "count");
    assert_eq!(args[0].kind, ArgKind::Count);
    assert_eq!(args[1].name, "register");
    assert_eq!(args[1].kind, ArgKind::Register);
}

#[test]
fn test_paste_before_no_buffer_returns_error() {
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
    let result = PasteBefore.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_paste_before_empty_register_returns_success() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = PasteBefore.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_paste_before_empty_content_returns_success() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    state.registers.set(RegisterContent::characterwise(""));
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = PasteBefore.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_paste_before_no_window_returns_error() {
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
    registers.set(RegisterContent::characterwise("X"));
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
    let result = PasteBefore.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_paste_before_linewise() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("line one\nline two");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    state.registers.set(RegisterContent::linewise("pasted\n"));
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = PasteBefore.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let buf_read = buf.read();
    assert_eq!(buf_read.line_count(), 3);
    assert_eq!(buf_read.line(0).as_deref(), Some("pasted"));
    assert_eq!(buf_read.line(1).as_deref(), Some("line one"));
    assert_eq!(buf_read.line(2).as_deref(), Some("line two"));
    drop(buf_read);

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 0);
}

#[test]
fn test_paste_before_characterwise() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    state.registers.set(RegisterContent::characterwise("XYZ"));
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = PasteBefore.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let content = buf.read().line(0).map(std::borrow::Cow::into_owned);
    assert_eq!(content.as_deref(), Some("XYZhello"));

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.column, 2);
}

#[test]
fn test_paste_before_characterwise_multiline() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    state.registers.set(RegisterContent::characterwise("A\nB"));
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = PasteBefore.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 1);
}

#[test]
fn test_paste_before_with_count() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    state.registers.set(RegisterContent::characterwise("X"));
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));
    let result = PasteBefore.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let content = buf.read().line(0).map(std::borrow::Cow::into_owned);
    assert_eq!(content.as_deref(), Some("XXhello"));
}

#[test]
fn test_paste_after_characterwise_single_char_empty_line() {
    // Tests paste on an empty line with characterwise content
    // Covers the branch at line 104-108 where line_len is 0
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    state.registers.set(RegisterContent::characterwise("test"));
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = PasteAfter.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let content = buf.read().line(0).map(std::borrow::Cow::into_owned);
    assert_eq!(content.as_deref(), Some("test"));
}

#[test]
fn test_paste_before_linewise_with_count_multiple() {
    // Tests linewise paste with count, multi-line content
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("line one");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    state.registers.set(RegisterContent::linewise("A\nB\n"));
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));
    let result = PasteBefore.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let buf_read = buf.read();
    assert_eq!(buf_read.line_count(), 5);
    assert_eq!(buf_read.line(0).as_deref(), Some("A"));
    assert_eq!(buf_read.line(1).as_deref(), Some("B"));
    assert_eq!(buf_read.line(2).as_deref(), Some("A"));
    assert_eq!(buf_read.line(3).as_deref(), Some("B"));
    assert_eq!(buf_read.line(4).as_deref(), Some("line one"));
    drop(buf_read);
}

// =========================================================================
// Metadata tests
// =========================================================================

#[test]
fn test_paste_after_default() {
    let cmd = PasteAfter;
    assert_eq!(cmd.id().name(), "paste-after");
}

#[test]
fn test_paste_before_default() {
    let cmd = PasteBefore;
    assert_eq!(cmd.id().name(), "paste-before");
}

// =========================================================================
// Characterwise paste cursor position - multiline with empty last line
// =========================================================================

#[test]
fn test_paste_after_characterwise_multiline_empty_last_line() {
    // "A\n".lines() = ["A"] (single element), so single-line branch is taken.
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    state.registers.set(RegisterContent::characterwise("A\n"));
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = PasteAfter.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    // "A\n".lines() => ["A"], treated as single-line paste
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 2);
}

#[test]
fn test_paste_before_characterwise_multiline_empty_last_line() {
    // "A\n".lines() = ["A"] (single element), so single-line branch is taken.
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    state.registers.set(RegisterContent::characterwise("A\n"));
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = PasteBefore.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    // "A\n".lines() => ["A"], treated as single-line paste
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 1);
}

// =========================================================================
// Paste after characterwise with count and multiline
// =========================================================================

#[test]
fn test_paste_after_characterwise_multiline_with_count() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    state.registers.set(RegisterContent::characterwise("X\nY"));
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));
    let result = PasteAfter.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    // count=2 means "X\nYX\nY" pasted
    // Last line "Y" has 1 char, col = 0
    assert_eq!(window.cursor.column, 0);
}

// =========================================================================
// Paste before with count characterwise
// =========================================================================

#[test]
fn test_paste_before_characterwise_with_count() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    state.registers.set(RegisterContent::characterwise("AB"));
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));
    let result = PasteBefore.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let content = buf.read().line(0).map(std::borrow::Cow::into_owned);
    assert_eq!(content.as_deref(), Some("ABABhello"));

    drop(runtime);
    let window = state.windows.active().unwrap();
    // "ABAB" is 4 chars, col = 4-1 = 3
    assert_eq!(window.cursor.column, 3);
}

// =========================================================================
// Paste after linewise on non-first line
// =========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_paste_after_linewise_on_second_line() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("line 1\nline 2\nline 3");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    state.registers.set(RegisterContent::linewise("pasted\n"));
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(1, 0).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = PasteAfter.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let buf_read = buf.read();
    assert_eq!(buf_read.line_count(), 4);
    assert_eq!(buf_read.line(1).as_deref(), Some("line 2"));
    assert_eq!(buf_read.line(2).as_deref(), Some("pasted"));
    assert_eq!(buf_read.line(3).as_deref(), Some("line 3"));
    drop(buf_read);

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 2);
    assert_eq!(window.cursor.column, 0);
}

// =========================================================================
// Paste before linewise on non-first line
// =========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_paste_before_linewise_on_second_line() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("line 1\nline 2\nline 3");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    state.registers.set(RegisterContent::linewise("pasted\n"));
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(1, 0).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = PasteBefore.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let buf_read = buf.read();
    assert_eq!(buf_read.line_count(), 4);
    assert_eq!(buf_read.line(0).as_deref(), Some("line 1"));
    assert_eq!(buf_read.line(1).as_deref(), Some("pasted"));
    assert_eq!(buf_read.line(2).as_deref(), Some("line 2"));
    drop(buf_read);

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 1);
    assert_eq!(window.cursor.column, 0);
}

// =========================================================================
// Paste after characterwise at end of line
// =========================================================================

#[test]
fn test_paste_after_characterwise_multiline_last_line_empty() {
    // "A\n\n".lines() = ["A", ""], last_line_len = 0, col = 0 branch
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    state.registers.set(RegisterContent::characterwise("A\n\n"));
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = PasteAfter.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    // "A\n\n".lines() => ["A", ""], len=2, last_line_len=0, col=0
    // insert at (0, 1), final_line = 0 + 2 - 1 = 1
    assert_eq!(window.cursor.column, 0);
    assert_eq!(window.cursor.line, 1);
}

#[test]
fn test_paste_before_characterwise_multiline_last_line_empty() {
    // "A\n\n".lines() = ["A", ""], last_line_len = 0, col = 0 branch
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    state.registers.set(RegisterContent::characterwise("A\n\n"));
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = PasteBefore.execute(&mut runtime, &args);
    assert!(result.is_success());

    drop(runtime);
    let window = state.windows.active().unwrap();
    // "A\n\n".lines() => ["A", ""], len=2, last_line_len=0, col=0
    // insert at (0, 0), final_line = 0 + 2 - 1 = 1
    assert_eq!(window.cursor.column, 0);
    assert_eq!(window.cursor.line, 1);
}

#[test]
fn test_paste_after_named_register_not_set_returns_success() {
    // When a named register (e.g., 'a') hasn't been set, get_register returns None.
    // This covers line 54: `return CommandResult::Success; // Empty register`
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("register", ArgValue::Register('a'));
    let result = PasteAfter.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Buffer should be unchanged
    let buf = text_buf(&kernel, buffer_id);
    let content = buf.read().line(0).map(std::borrow::Cow::into_owned);
    assert_eq!(content.as_deref(), Some("hello"));
}

#[test]
fn test_paste_before_named_register_not_set_returns_success() {
    // When a named register (e.g., 'a') hasn't been set, get_register returns None.
    // This covers line 181: `return CommandResult::Success; // Empty register`
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("register", ArgValue::Register('a'));
    let result = PasteBefore.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Buffer should be unchanged
    let buf = text_buf(&kernel, buffer_id);
    let content = buf.read().line(0).map(std::borrow::Cow::into_owned);
    assert_eq!(content.as_deref(), Some("hello"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_paste_after_characterwise_at_end_of_line() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    state.registers.set(RegisterContent::characterwise("XY"));
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 4).into(); // last char 'o'
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = PasteAfter.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = text_buf(&kernel, buffer_id);
    let content = buf.read().line(0).map(std::borrow::Cow::into_owned);
    assert_eq!(content.as_deref(), Some("helloXY"));

    drop(runtime);
    let window = state.windows.active().unwrap();
    // Cursor at end of pasted text - 1
    assert_eq!(window.cursor.column, 6);
}
