#![allow(clippy::significant_drop_tightening)]

use {
    super::super::*,
    reovim_driver_command::{ArgKind, ArgValue, Command, CommandContext, CommandHandler},
    reovim_driver_session::{
        ClientId, ExtensionMap, Jumplist, MarkBank, Session, SessionRuntime, Window, WindowLayout,
        testing::StubExecutor,
    },
    reovim_kernel::{
        api::v1::{BufferId, KernelContext, ModeStack, RwLock},
        testing::{create_test_context, test_mode},
    },
    reovim_provider_text::Buffer,
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
// Command Identity
// =========================================================================

#[test]
fn test_toggle_case_id() {
    let cmd = ToggleCase;
    assert_eq!(cmd.id().name(), "toggle-case");
}

#[test]
fn test_toggle_case_description() {
    assert!(!ToggleCase.description().is_empty());
}

#[test]
fn test_toggle_case_has_count_arg() {
    let args = ToggleCase.args();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].name, "count");
    assert_eq!(args[0].kind, ArgKind::Count);
}

#[test]
fn test_toggle_case_debug() {
    assert!(!format!("{ToggleCase:?}").is_empty());
}

// =========================================================================
// Error Handling
// =========================================================================

#[test]
fn test_toggle_case_no_buffer_returns_error() {
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
    let result = ToggleCase.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_toggle_case_no_window_returns_error() {
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
    let result = ToggleCase.execute(&mut runtime, &args);
    assert!(result.is_error());
}

// =========================================================================
// Basic Behavior
// =========================================================================

#[test]
fn test_toggle_case_lowercase_to_uppercase() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = ToggleCase.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let read = buf.read();
    assert_eq!(read.line(0).as_deref(), Some("Hello"));
    drop(read);

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.column, 1);
}

#[test]
fn test_toggle_case_uppercase_to_lowercase() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("HELLO");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = ToggleCase.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let read = buf.read();
    assert_eq!(read.line(0).as_deref(), Some("hELLO"));
}

#[test]
fn test_toggle_case_mixed() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("HeLLo");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 0).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(5));

    let result = ToggleCase.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let read = buf.read();
    assert_eq!(read.line(0).as_deref(), Some("hEllO"));
}

// =========================================================================
// Count
// =========================================================================

#[test]
fn test_toggle_case_with_count() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(3));

    let result = ToggleCase.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let read = buf.read();
    assert_eq!(read.line(0).as_deref(), Some("HELlo"));
    drop(read);

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.column, 3);
}

#[test]
fn test_toggle_case_count_exceeds_line_length() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hi");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(100));

    let result = ToggleCase.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let read = buf.read();
    assert_eq!(read.line(0).as_deref(), Some("HI"));
    drop(read);

    // Cursor clamped to last char
    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.column, 1);
}

// =========================================================================
// Edge Cases
// =========================================================================

#[test]
fn test_toggle_case_empty_line() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = ToggleCase.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_toggle_case_at_end_of_line() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("ab");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 2).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = ToggleCase.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Buffer unchanged
    let buf = kernel.buffers.get(buffer_id).unwrap();
    let read = buf.read();
    assert_eq!(read.line(0).as_deref(), Some("ab"));
}

#[test]
fn test_toggle_case_non_alpha_characters() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("1!@#");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(4));

    let result = ToggleCase.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Non-alpha chars unchanged
    let buf = kernel.buffers.get(buffer_id).unwrap();
    let read = buf.read();
    assert_eq!(read.line(0).as_deref(), Some("1!@#"));
    drop(read);

    // Cursor still advances
    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.column, 3);
}

#[test]
fn test_toggle_case_middle_of_line() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("abcDE");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 2).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));

    let result = ToggleCase.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let read = buf.read();
    assert_eq!(read.line(0).as_deref(), Some("abCdE"));
    drop(read);

    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.column, 4);
}

#[test]
fn test_toggle_case_multiline_only_affects_current_line() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("abc\nDEF");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(100));

    let result = ToggleCase.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let read = buf.read();
    assert_eq!(read.line(0).as_deref(), Some("ABC"));
    assert_eq!(read.line(1).as_deref(), Some("DEF")); // Second line unchanged
}

#[test]
fn test_toggle_case_single_char_at_last_position() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("abC");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 2).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = ToggleCase.execute(&mut runtime, &args);
    assert!(result.is_success());

    let buf = kernel.buffers.get(buffer_id).unwrap();
    let read = buf.read();
    assert_eq!(read.line(0).as_deref(), Some("abc"));
    drop(read);

    // Cursor stays at last char (can't advance past end)
    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.column, 2);
}
