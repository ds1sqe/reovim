use {
    crate::{TEXTOBJECTS_MODULE, paragraph::*},
    reovim_driver_command::{ArgKind, ArgValue, Command, CommandContext, CommandHandler},
    reovim_driver_session::{
        ClientId, ExtensionMap, OperatorPendingState, Session, SessionRuntime, Window,
        WindowLayout,
        api::{CommandExecutor, ExtensionApi},
        testing::StubExecutor,
    },
    reovim_kernel::{
        api::{
            ModeStack,
            v1::{BufferId, HistoryRing, KernelContext, MarkBank, ModeId, Position, RegisterBank},
        },
        testing::{create_test_context, setup_buffer, test_mode},
    },
};

// =========================================================================
// Test Infrastructure
// =========================================================================

struct TestState {
    session: Session,
    mode_stack: ModeStack,
    windows: WindowLayout,
    extensions: ExtensionMap,
    compositor: Option<Box<dyn reovim_driver_display::layout::RootCompositor>>,
    tabs: reovim_driver_session::TabPageSet,
    registers: RegisterBank,
    clipboard_history: HistoryRing,
    local_marks: MarkBank,
    active_buffer: Option<BufferId>,
    terminal_size: (u16, u16),
}

impl TestState {
    fn with_window(buffer_id: BufferId, mode: ModeId) -> Self {
        let session = Session::new(ClientId::new(1), mode.clone());
        let mode_stack = ModeStack::new(mode);
        let mut windows = WindowLayout::empty();
        let extensions = ExtensionMap::new();
        windows.add(Window::with_buffer(buffer_id));
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
            active_buffer: None,
            terminal_size: (80, 24),
        }
    }

    fn with_custom_window(window: Window, mode: ModeId) -> Self {
        let session = Session::new(ClientId::new(1), mode.clone());
        let mode_stack = ModeStack::new(mode);
        let mut windows = WindowLayout::empty();
        let extensions = ExtensionMap::new();
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
            active_buffer: None,
            terminal_size: (80, 24),
        }
    }

    fn empty(mode: ModeId) -> Self {
        let session = Session::new(ClientId::new(1), mode.clone());
        let mode_stack = ModeStack::new(mode);
        let mut windows = WindowLayout::empty();
        let extensions = ExtensionMap::new();
        windows.add(Window::new());
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
            active_buffer: None,
            terminal_size: (80, 24),
        }
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
                active_buffer: &mut self.active_buffer,
                terminal_size: &mut self.terminal_size,
            },
            kernel,
            executor,
        )
    }
}

// =========================================================================
// Command ID Tests
// =========================================================================

#[test]
fn test_inner_paragraph_id() {
    let cmd = InnerParagraph;
    assert_eq!(cmd.id().module(), &TEXTOBJECTS_MODULE);
    assert_eq!(cmd.id().name(), "inner-paragraph");
}

#[test]
fn test_around_paragraph_id() {
    let cmd = AroundParagraph;
    assert_eq!(cmd.id().name(), "around-paragraph");
}

#[test]
fn test_all_commands_count() {
    let cmds = all_commands();
    assert_eq!(cmds.len(), 2);
}

// =========================================================================
// Description Tests
// =========================================================================

#[test]
fn test_paragraph_descriptions() {
    assert_eq!(InnerParagraph.description(), "Inner paragraph text object");
    assert_eq!(AroundParagraph.description(), "Around paragraph text object");
}

// =========================================================================
// Command Args Tests
// =========================================================================

#[test]
fn test_paragraph_commands_have_count_arg() {
    for cmd in all_commands() {
        let args = cmd.args();
        assert!(!args.is_empty(), "Command {} should have count arg", cmd.id());
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
    }
}

// =========================================================================
// Error Handling Tests
// =========================================================================

#[test]
fn test_inner_paragraph_no_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let args = CommandContext::new();
    let result = InnerParagraph.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_around_paragraph_no_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let args = CommandContext::new();
    let result = AroundParagraph.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_inner_paragraph_invalid_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(999));
    let result = InnerParagraph.execute(&mut runtime, &args);
    assert!(result.is_error());
}

// =========================================================================
// Paragraph Execution Tests
// =========================================================================

#[test]
fn test_inner_paragraph_basic() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "line one\nline two\n\nline four");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerParagraph.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_around_paragraph_basic() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "line one\nline two\n\nline four");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundParagraph.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_inner_paragraph_stores_linewise_range() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "line one\nline two\n\nline four");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerParagraph.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Paragraph text objects store linewise ranges
    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

#[test]
fn test_around_paragraph_stores_linewise_range() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "line one\nline two\n\nline four");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundParagraph.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

#[test]
fn test_inner_paragraph_with_cursor_position() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "line one\nline two\n\nline four\nline five");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(3, 0).into(); // on "line four"

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerParagraph.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

// =========================================================================
// Edge Cases
// =========================================================================

#[test]
fn test_empty_buffer_paragraph() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerParagraph.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_single_line_paragraph() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "only one line");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerParagraph.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_all_blank_lines_paragraph() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "\n\n\n");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerParagraph.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_multiple_paragraphs() {
    let kernel = create_test_context();
    let buffer_id =
        setup_buffer(&kernel, "para one\nstill one\n\npara two\nstill two\n\npara three");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(3, 0).into(); // on "para two"

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundParagraph.execute(&mut runtime, &args);
    assert!(result.is_success());
}

// =========================================================================
// Additional coverage tests
// =========================================================================

#[test]
fn test_around_paragraph_invalid_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(999));
    let result = AroundParagraph.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_inner_paragraph_with_count() {
    let kernel = create_test_context();
    let buffer_id =
        setup_buffer(&kernel, "para one\nstill one\n\npara two\nstill two\n\npara three");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));

    let result = InnerParagraph.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_around_paragraph_with_count() {
    let kernel = create_test_context();
    let buffer_id =
        setup_buffer(&kernel, "para one\nstill one\n\npara two\nstill two\n\npara three");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));

    let result = AroundParagraph.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_around_paragraph_empty_buffer() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundParagraph.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_around_paragraph_single_line() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "only one line");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundParagraph.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_around_paragraph_all_blank_lines() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "\n\n\n");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundParagraph.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_around_paragraph_with_cursor_position() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "line one\nline two\n\nline four\nline five");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(3, 0).into(); // on "line four"

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundParagraph.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

#[test]
fn test_inner_paragraph_cursor_on_blank_line() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "line one\n\nline three");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(1, 0).into(); // on blank line

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerParagraph.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_around_paragraph_cursor_on_blank_line() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "line one\n\nline three");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(1, 0).into(); // on blank line

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundParagraph.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_inner_paragraph_at_last_line() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "line one\n\nlast line");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(2, 0).into(); // on last line

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerParagraph.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

#[test]
fn test_around_paragraph_at_last_line() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "line one\n\nlast line");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(2, 0).into(); // on last line

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundParagraph.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

#[test]
fn test_inner_paragraph_cursor_mid_column() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello world\nfoo bar");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into(); // on space in "hello world"

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerParagraph.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

// =========================================================================
// MC/DC Coverage: "No active window" path
// =========================================================================

#[test]
fn test_inner_paragraph_no_active_window_returns_error() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "some text");
    // Create state with truly empty window layout (no windows added)
    let session = Session::new(ClientId::new(1), test_mode());
    let mode_stack = ModeStack::new(test_mode());
    let windows = WindowLayout::empty(); // no windows
    let extensions = ExtensionMap::new();
    let mut state = TestState {
        session,
        mode_stack,
        windows,
        extensions,
        compositor: None,
        tabs: reovim_driver_session::TabPageSet::new(),
        registers: RegisterBank::new(),
        clipboard_history: HistoryRing::new(),
        local_marks: MarkBank::new(),
        active_buffer: None,
        terminal_size: (80, 24),
    };
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerParagraph.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_around_paragraph_no_active_window_returns_error() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "some text");
    let session = Session::new(ClientId::new(1), test_mode());
    let mode_stack = ModeStack::new(test_mode());
    let windows = WindowLayout::empty();
    let extensions = ExtensionMap::new();
    let mut state = TestState {
        session,
        mode_stack,
        windows,
        extensions,
        compositor: None,
        tabs: reovim_driver_session::TabPageSet::new(),
        registers: RegisterBank::new(),
        clipboard_history: HistoryRing::new(),
        local_marks: MarkBank::new(),
        active_buffer: None,
        terminal_size: (80, 24),
    };
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundParagraph.execute(&mut runtime, &args);
    assert!(result.is_error());
}

// =========================================================================
// MC/DC Coverage: count path with count provided
// =========================================================================

#[test]
fn test_around_paragraph_with_count_stores_linewise_range() {
    let kernel = create_test_context();
    let buffer_id =
        setup_buffer(&kernel, "para one\nstill one\n\npara two\nstill two\n\npara three");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(3));

    let result = AroundParagraph.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

#[test]
fn test_inner_paragraph_with_count_stores_linewise_range() {
    let kernel = create_test_context();
    let buffer_id =
        setup_buffer(&kernel, "para one\nstill one\n\npara two\nstill two\n\npara three");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(3));

    let result = InnerParagraph.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

// =========================================================================
// MC/DC Coverage: Default trait for paragraph commands
// =========================================================================

#[test]
fn test_inner_paragraph_default() {
    fn make_default<T: Default>() -> T {
        T::default()
    }
    let cmd: InnerParagraph = make_default();
    assert_eq!(cmd.id().name(), "inner-paragraph");
}

#[test]
fn test_around_paragraph_default() {
    fn make_default<T: Default>() -> T {
        T::default()
    }
    let cmd: AroundParagraph = make_default();
    assert_eq!(cmd.id().name(), "around-paragraph");
}

#[test]
fn test_inner_paragraph_debug() {
    let cmd = InnerParagraph;
    let debug_str = format!("{cmd:?}");
    assert!(debug_str.contains("InnerParagraph"));
}

#[test]
fn test_around_paragraph_debug() {
    let cmd = AroundParagraph;
    let debug_str = format!("{cmd:?}");
    assert!(debug_str.contains("AroundParagraph"));
}

#[test]
fn test_inner_paragraph_clone() {
    let cmd = InnerParagraph;
    let cloned = cmd;
    assert_eq!(cloned.id().name(), "inner-paragraph");
}

#[test]
fn test_around_paragraph_clone() {
    let cmd = AroundParagraph;
    let cloned = cmd;
    assert_eq!(cloned.id().name(), "around-paragraph");
}
