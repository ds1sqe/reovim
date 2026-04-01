#![allow(clippy::significant_drop_tightening)]

use {
    super::SubstituteCommand,
    reovim_driver_command::{ArgKind, ArgValue, Command, CommandContext, CommandHandler},
    reovim_driver_session::{
        ClientId, ExtensionMap, Session, SessionRuntime, Window, WindowLayout,
        testing::StubExecutor,
    },
    reovim_kernel::{
        api::v1::{
            Buffer, BufferId, HistoryRing, Jumplist, KernelContext, MarkBank, ModeStack, Position,
            RegisterBank, RwLock,
        },
        testing::{create_test_context, test_mode},
    },
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

#[cfg_attr(coverage_nightly, coverage(off))]
fn read_line(kernel: &KernelContext, buffer_id: BufferId, line: usize) -> Option<String> {
    let buf = kernel.buffers.get(buffer_id)?;
    let read = buf.read();
    read.line(line).map(std::borrow::Cow::into_owned)
}

// =========================================================================
// Command Identity
// =========================================================================

#[test]
fn test_substitute_id() {
    let cmd = SubstituteCommand;
    assert_eq!(cmd.id().name(), "substitute");
}

#[test]
fn test_substitute_description() {
    assert!(!SubstituteCommand.description().is_empty());
}

#[test]
fn test_substitute_names() {
    let names = SubstituteCommand.names();
    assert!(names.contains(&"s"));
    assert!(names.contains(&"substitute"));
}

#[test]
fn test_substitute_has_rest_arg() {
    let args = SubstituteCommand.args();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].kind, ArgKind::Rest);
}

#[test]
fn test_substitute_debug() {
    assert!(!format!("{SubstituteCommand:?}").is_empty());
}

// =========================================================================
// Error Handling
// =========================================================================

#[test]
fn test_substitute_no_buffer_returns_error() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("test");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    // Clear the window's buffer_id to simulate no buffer
    state.windows = WindowLayout::empty();
    state.windows.add(Window::new());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let args = CommandContext::new();
    let result = SubstituteCommand.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_substitute_empty_args_returns_error() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = SubstituteCommand.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_substitute_invalid_syntax_returns_error() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("args", ArgValue::String("invalid".to_string()));
    let result = SubstituteCommand.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_substitute_invalid_regex_returns_error() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("args", ArgValue::String("/[invalid/rep/".to_string()));
    let result = SubstituteCommand.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_substitute_no_match_returns_error() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("args", ArgValue::String("/zzzzz/rep/".to_string()));
    let result = SubstituteCommand.execute(&mut runtime, &args);
    assert!(result.is_error());
}

// =========================================================================
// Basic Substitution
// =========================================================================

#[test]
fn test_substitute_basic_first_match() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("foo bar foo");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("args", ArgValue::String("/foo/baz/".to_string()));

    let result = SubstituteCommand.execute(&mut runtime, &args);
    assert!(result.is_success());
    assert_eq!(read_line(&kernel, buffer_id, 0).as_deref(), Some("baz bar foo"));
}

#[test]
fn test_substitute_global_flag() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("foo bar foo");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("args", ArgValue::String("/foo/baz/g".to_string()));

    let result = SubstituteCommand.execute(&mut runtime, &args);
    assert!(result.is_success());
    assert_eq!(read_line(&kernel, buffer_id, 0).as_deref(), Some("baz bar baz"));
}

#[test]
fn test_substitute_case_insensitive() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("Hello HELLO hello");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("args", ArgValue::String("/hello/world/gi".to_string()));

    let result = SubstituteCommand.execute(&mut runtime, &args);
    assert!(result.is_success());
    assert_eq!(read_line(&kernel, buffer_id, 0).as_deref(), Some("world world world"));
}

// =========================================================================
// Range Handling
// =========================================================================

#[test]
fn test_substitute_with_range_all_lines() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("foo\nfoo\nfoo");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("args", ArgValue::String("/foo/bar/".to_string()));
    args.set("range", ArgValue::Range(0, 2));

    let result = SubstituteCommand.execute(&mut runtime, &args);
    assert!(result.is_success());
    assert_eq!(read_line(&kernel, buffer_id, 0).as_deref(), Some("bar"));
    assert_eq!(read_line(&kernel, buffer_id, 1).as_deref(), Some("bar"));
    assert_eq!(read_line(&kernel, buffer_id, 2).as_deref(), Some("bar"));
}

#[test]
fn test_substitute_current_line_only() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("foo\nfoo\nfoo");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(1, 0).into();
    }
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("args", ArgValue::String("/foo/bar/".to_string()));

    let result = SubstituteCommand.execute(&mut runtime, &args);
    assert!(result.is_success());
    assert_eq!(read_line(&kernel, buffer_id, 0).as_deref(), Some("foo"));
    assert_eq!(read_line(&kernel, buffer_id, 1).as_deref(), Some("bar"));
    assert_eq!(read_line(&kernel, buffer_id, 2).as_deref(), Some("foo"));
}

// =========================================================================
// Count-only Mode (n flag)
// =========================================================================

#[test]
fn test_substitute_count_only() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("foo bar foo baz foo");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("args", ArgValue::String("/foo/bar/n".to_string()));

    let result = SubstituteCommand.execute(&mut runtime, &args);
    assert!(result.is_success());
    // Buffer should be unchanged
    assert_eq!(read_line(&kernel, buffer_id, 0).as_deref(), Some("foo bar foo baz foo"));
}

// =========================================================================
// Regex Patterns
// =========================================================================

#[test]
fn test_substitute_regex_pattern() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("abc123def456");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("args", ArgValue::String("/\\d+/NUM/g".to_string()));

    let result = SubstituteCommand.execute(&mut runtime, &args);
    assert!(result.is_success());
    assert_eq!(read_line(&kernel, buffer_id, 0).as_deref(), Some("abcNUMdefNUM"));
}

// =========================================================================
// Delimiter Handling
// =========================================================================

#[test]
fn test_substitute_alternate_delimiter() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("/path/to/file");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    // Use | as delimiter to avoid escaping /
    args.set("args", ArgValue::String("|/path/to|/new/path|".to_string()));

    let result = SubstituteCommand.execute(&mut runtime, &args);
    assert!(result.is_success());
    assert_eq!(read_line(&kernel, buffer_id, 0).as_deref(), Some("/new/path/file"));
}

#[test]
fn test_substitute_escaped_delimiter() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("a/b");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    // Escaped / in pattern
    args.set("args", ArgValue::String("/a\\/b/c/".to_string()));

    let result = SubstituteCommand.execute(&mut runtime, &args);
    assert!(result.is_success());
    assert_eq!(read_line(&kernel, buffer_id, 0).as_deref(), Some("c"));
}

// =========================================================================
// Edge Cases
// =========================================================================

#[test]
fn test_substitute_empty_replacement() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("args", ArgValue::String("/world//".to_string()));

    let result = SubstituteCommand.execute(&mut runtime, &args);
    assert!(result.is_success());
    assert_eq!(read_line(&kernel, buffer_id, 0).as_deref(), Some("hello "));
}

#[test]
fn test_substitute_empty_pattern_returns_error() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("args", ArgValue::String("//rep/".to_string()));
    let result = SubstituteCommand.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_substitute_multiline_global() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("aaa\nbbb\naaa");
    let buffer_id = kernel.buffers.register(Arc::new(RwLock::new(buffer)));
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("args", ArgValue::String("/aaa/zzz/".to_string()));
    args.set("range", ArgValue::Range(0, 2));

    let result = SubstituteCommand.execute(&mut runtime, &args);
    assert!(result.is_success());
    assert_eq!(read_line(&kernel, buffer_id, 0).as_deref(), Some("zzz"));
    assert_eq!(read_line(&kernel, buffer_id, 1).as_deref(), Some("bbb"));
    assert_eq!(read_line(&kernel, buffer_id, 2).as_deref(), Some("zzz"));
}
