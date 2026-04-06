use {
    crate::{TEXTOBJECTS_MODULE, bracket::*},
    reovim_driver_command::{ArgKind, ArgValue, Command, CommandContext, CommandHandler},
    reovim_driver_session::{
        ClientId, ExtensionMap, Jumplist, MarkBank, OperatorPendingState, Session, SessionRuntime,
        Window, WindowLayout,
        api::{CommandExecutor, ExtensionApi, SelectionMode},
        testing::{StubExecutor, create_test_kernel, dual_register_buffer},
    },
    reovim_kernel::{
        api::{
            ModeStack,
            v1::{BufferId, KernelContext, ModeId, ModuleId},
        },
        testing::test_mode,
    },
    reovim_types_text::{HistoryRing, Position, RegisterBank},
};

// =========================================================================
// Test Infrastructure (mirrors word.rs pattern)
// =========================================================================

fn visual_mode() -> ModeId {
    ModeId::new(ModuleId::new("test"), "visual")
}

fn visual_line_mode() -> ModeId {
    ModeId::new(ModuleId::new("test"), "visual-line")
}

fn visual_block_mode() -> ModeId {
    ModeId::new(ModuleId::new("test"), "visual-block")
}

fn operator_pending_mode() -> ModeId {
    ModeId::new(ModuleId::new("test"), "delete")
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
            jumplist: Jumplist::new(),
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
            jumplist: Jumplist::new(),
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
            jumplist: Jumplist::new(),
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
// Command ID Tests
// =========================================================================

#[test]
fn test_inner_paren_id() {
    let cmd = InnerParen;
    assert_eq!(cmd.id().module(), &TEXTOBJECTS_MODULE);
    assert_eq!(cmd.id().name(), "inner-paren");
}

#[test]
fn test_around_paren_id() {
    let cmd = AroundParen;
    assert_eq!(cmd.id().name(), "around-paren");
}

#[test]
fn test_inner_bracket_id() {
    let cmd = InnerSquareBracket;
    assert_eq!(cmd.id().name(), "inner-bracket");
}

#[test]
fn test_around_bracket_id() {
    let cmd = AroundSquareBracket;
    assert_eq!(cmd.id().name(), "around-bracket");
}

#[test]
fn test_inner_brace_id() {
    let cmd = InnerBrace;
    assert_eq!(cmd.id().name(), "inner-brace");
}

#[test]
fn test_around_brace_id() {
    let cmd = AroundBrace;
    assert_eq!(cmd.id().name(), "around-brace");
}

#[test]
fn test_inner_angle_id() {
    let cmd = InnerAngle;
    assert_eq!(cmd.id().name(), "inner-angle");
}

#[test]
fn test_around_angle_id() {
    let cmd = AroundAngle;
    assert_eq!(cmd.id().name(), "around-angle");
}

#[test]
fn test_all_commands_count() {
    let cmds = all_commands();
    assert_eq!(cmds.len(), 8);
}

// =========================================================================
// Description Tests
// =========================================================================

#[test]
fn test_bracket_descriptions() {
    assert_eq!(InnerParen.description(), "Inner parenthesis text object");
    assert_eq!(AroundParen.description(), "Around parenthesis text object");
    assert_eq!(InnerSquareBracket.description(), "Inner square bracket text object");
    assert_eq!(AroundSquareBracket.description(), "Around square bracket text object");
    assert_eq!(InnerBrace.description(), "Inner brace text object");
    assert_eq!(AroundBrace.description(), "Around brace text object");
    assert_eq!(InnerAngle.description(), "Inner angle bracket text object");
    assert_eq!(AroundAngle.description(), "Around angle bracket text object");
}

// =========================================================================
// Command Args Tests
// =========================================================================

#[test]
fn test_bracket_commands_have_count_arg() {
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
fn test_inner_paren_no_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let args = CommandContext::new();
    let result = InnerParen.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_around_paren_no_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let args = CommandContext::new();
    let result = AroundParen.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_inner_paren_invalid_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(999));
    let result = InnerParen.execute(&mut runtime, &args);
    assert!(result.is_error());
}

// =========================================================================
// Parenthesis Tests
// =========================================================================

#[test]
fn test_inner_paren_basic() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo(bar)baz");
    // Cursor on 'b' inside parens at col 4
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerParen.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_around_paren_basic() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo(bar)baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundParen.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_inner_paren_operator_pending_stores_range() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo(bar)baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, operator_pending_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerParen.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

#[test]
fn test_inner_paren_visual_mode_sets_selection() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo(bar)baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, visual_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerParen.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some(), "Selection should be set in visual mode");
}

#[test]
fn test_around_paren_visual_mode_sets_selection() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo(bar)baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, visual_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundParen.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some(), "Selection should be set in visual mode for around-paren");
}

#[test]
fn test_inner_paren_no_matching_brackets_is_noop() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "no brackets here");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerParen.execute(&mut runtime, &args);
    assert!(result.is_success()); // no-op, not error
}

// =========================================================================
// Square Bracket Tests
// =========================================================================

#[test]
fn test_inner_square_bracket_basic() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo[bar]baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerSquareBracket.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_around_square_bracket_basic() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo[bar]baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundSquareBracket.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_inner_square_bracket_operator_pending_stores_range() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo[bar]baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, operator_pending_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerSquareBracket.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

#[test]
fn test_inner_square_bracket_visual_mode_sets_selection() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo[bar]baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, visual_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerSquareBracket.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
}

// =========================================================================
// Brace Tests
// =========================================================================

#[test]
fn test_inner_brace_basic() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo{bar}baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerBrace.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_around_brace_basic() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo{bar}baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundBrace.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_inner_brace_operator_pending_stores_range() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo{bar}baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, operator_pending_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerBrace.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

#[test]
fn test_around_brace_visual_mode_sets_selection() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo{bar}baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, visual_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundBrace.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
}

// =========================================================================
// Angle Bracket Tests
// =========================================================================

#[test]
fn test_inner_angle_basic() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo<bar>baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerAngle.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_around_angle_basic() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo<bar>baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundAngle.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_inner_angle_operator_pending_stores_range() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo<bar>baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, operator_pending_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerAngle.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

#[test]
fn test_around_angle_visual_mode_sets_selection() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo<bar>baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, visual_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundAngle.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
}

// =========================================================================
// Visual Line / Visual Block Mode Tests
// =========================================================================

#[test]
fn test_inner_paren_visual_line_mode_sets_line_selection() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo(bar)baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, visual_line_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerParen.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, SelectionMode::Line);
}

#[test]
fn test_inner_paren_visual_block_mode_sets_block_selection() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo(bar)baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, visual_block_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerParen.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, SelectionMode::Block);
}

// =========================================================================
// Edge Cases
// =========================================================================

#[test]
fn test_empty_buffer_bracket() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerParen.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_nested_brackets() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "((inner))");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 3).into(); // inside inner

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerParen.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_bracket_visual_mode_does_not_store_operator_range() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo(bar)baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, visual_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerParen.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    if let Some(ext_state) = ext_state {
        assert!(!ext_state.has_textobj_range());
    }
}

#[test]
fn test_all_bracket_commands_no_buffer_error() {
    let kernel = KernelContext::default();
    let executor = StubExecutor;

    // Test each bracket command with no buffer
    let commands: Vec<Box<dyn CommandHandler>> = all_commands();
    for cmd in &commands {
        let mut state = TestState::empty(test_mode());
        let mut runtime = state.runtime(&kernel, &executor);
        let args = CommandContext::new();
        let result = cmd.execute(&mut runtime, &args);
        assert!(result.is_error(), "Command {} should error without buffer", cmd.id());
    }
}

#[test]
fn test_multiline_brackets() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "func(\n  arg1,\n  arg2\n)");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(1, 2).into(); // on 'a' of arg1

    let mut state = TestState::with_custom_window(window, operator_pending_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerParen.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

// =========================================================================
// Additional coverage tests
// =========================================================================

#[test]
fn test_around_paren_operator_pending_stores_range() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo(bar)baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, operator_pending_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundParen.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

#[test]
fn test_around_square_bracket_operator_pending_stores_range() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo[bar]baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, operator_pending_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundSquareBracket.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

#[test]
fn test_around_brace_operator_pending_stores_range() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo{bar}baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, operator_pending_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundBrace.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

#[test]
fn test_around_angle_operator_pending_stores_range() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo<bar>baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, operator_pending_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundAngle.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

#[test]
fn test_inner_brace_visual_mode_sets_selection() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo{bar}baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, visual_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerBrace.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
}

#[test]
fn test_inner_angle_visual_mode_sets_selection() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo<bar>baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, visual_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerAngle.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
}

#[test]
fn test_around_square_bracket_visual_mode_sets_selection() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo[bar]baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, visual_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundSquareBracket.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
}

#[test]
fn test_inner_square_bracket_visual_line_mode() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo[bar]baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, visual_line_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerSquareBracket.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, SelectionMode::Line);
}

#[test]
fn test_inner_brace_visual_block_mode() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo{bar}baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, visual_block_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerBrace.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, SelectionMode::Block);
}

#[test]
fn test_inner_angle_visual_line_mode() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo<bar>baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, visual_line_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerAngle.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, SelectionMode::Line);
}

#[test]
fn test_no_matching_square_brackets_is_noop() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "no brackets here");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerSquareBracket.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_no_matching_braces_is_noop() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "no braces here");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerBrace.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_no_matching_angles_is_noop() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "no angles here");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerAngle.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_inner_square_bracket_no_buffer_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let args = CommandContext::new();
    let result = InnerSquareBracket.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_inner_brace_no_buffer_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let args = CommandContext::new();
    let result = InnerBrace.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_inner_angle_no_buffer_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let args = CommandContext::new();
    let result = InnerAngle.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_inner_square_bracket_invalid_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(999));
    let result = InnerSquareBracket.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_inner_brace_invalid_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(999));
    let result = InnerBrace.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_inner_angle_invalid_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(999));
    let result = InnerAngle.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_empty_buffer_square_bracket() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerSquareBracket.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_empty_buffer_brace() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerBrace.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_empty_buffer_angle() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerAngle.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_nested_square_brackets() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "[[inner]]");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 3).into();

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerSquareBracket.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_nested_braces() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "{{inner}}");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 3).into();

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerBrace.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_nested_angle_brackets() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "<<inner>>");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 3).into();

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerAngle.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_multiline_square_brackets() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "arr[\n  elem1,\n  elem2\n]");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(1, 2).into();

    let mut state = TestState::with_custom_window(window, operator_pending_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerSquareBracket.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

#[test]
fn test_multiline_braces() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "fn {\n  body;\n}");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(1, 2).into();

    let mut state = TestState::with_custom_window(window, operator_pending_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerBrace.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

#[test]
fn test_multiline_angle_brackets() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "tag<\n  content\n>");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(1, 2).into();

    let mut state = TestState::with_custom_window(window, operator_pending_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerAngle.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

#[test]
fn test_inner_paren_with_count() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "((inner))");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 3).into();

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));

    let result = InnerParen.execute(&mut runtime, &args);
    assert!(result.is_success());
}

// =========================================================================
// MC/DC Coverage: "No active window" path
// =========================================================================

#[test]
fn test_inner_paren_no_active_window_returns_error() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo(bar)baz");
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
        jumplist: Jumplist::new(),
        active_buffer: None,
        terminal_size: (80, 24),
    };
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerParen.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_around_paren_no_active_window_returns_error() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo(bar)baz");
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
        jumplist: Jumplist::new(),
        active_buffer: None,
        terminal_size: (80, 24),
    };
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundParen.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_inner_square_bracket_no_active_window_returns_error() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo[bar]baz");
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
        jumplist: Jumplist::new(),
        active_buffer: None,
        terminal_size: (80, 24),
    };
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerSquareBracket.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_inner_brace_no_active_window_returns_error() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo{bar}baz");
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
        jumplist: Jumplist::new(),
        active_buffer: None,
        terminal_size: (80, 24),
    };
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerBrace.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_inner_angle_no_active_window_returns_error() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo<bar>baz");
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
        jumplist: Jumplist::new(),
        active_buffer: None,
        terminal_size: (80, 24),
    };
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerAngle.execute(&mut runtime, &args);
    assert!(result.is_error());
}

// =========================================================================
// MC/DC Coverage: visual-line and visual-block for remaining bracket types
// =========================================================================

#[test]
fn test_around_paren_visual_line_mode() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo(bar)baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, visual_line_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundParen.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, SelectionMode::Line);
}

#[test]
fn test_around_paren_visual_block_mode() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo(bar)baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, visual_block_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundParen.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, SelectionMode::Block);
}

#[test]
fn test_around_square_bracket_visual_line_mode() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo[bar]baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, visual_line_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundSquareBracket.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, SelectionMode::Line);
}

#[test]
fn test_around_square_bracket_visual_block_mode() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo[bar]baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, visual_block_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundSquareBracket.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, SelectionMode::Block);
}

#[test]
fn test_around_brace_visual_line_mode() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo{bar}baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, visual_line_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundBrace.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, SelectionMode::Line);
}

#[test]
fn test_around_brace_visual_block_mode() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo{bar}baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, visual_block_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundBrace.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, SelectionMode::Block);
}

#[test]
fn test_around_angle_visual_line_mode() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo<bar>baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, visual_line_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundAngle.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, SelectionMode::Line);
}

#[test]
fn test_around_angle_visual_block_mode() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo<bar>baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, visual_block_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundAngle.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, SelectionMode::Block);
}

#[test]
fn test_inner_square_bracket_visual_block_mode() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo[bar]baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, visual_block_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerSquareBracket.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, SelectionMode::Block);
}

#[test]
fn test_inner_brace_visual_line_mode() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo{bar}baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, visual_line_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerBrace.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, SelectionMode::Line);
}

#[test]
fn test_inner_angle_visual_block_mode() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "foo<bar>baz");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, visual_block_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerAngle.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, SelectionMode::Block);
}

// =========================================================================
// MC/DC Coverage: Default/Debug/Copy traits
// =========================================================================

#[test]
fn test_bracket_defaults() {
    fn make_default<T: Default>() -> T {
        T::default()
    }
    let _: InnerParen = make_default();
    let _: AroundParen = make_default();
    let _: InnerSquareBracket = make_default();
    let _: AroundSquareBracket = make_default();
    let _: InnerBrace = make_default();
    let _: AroundBrace = make_default();
    let _: InnerAngle = make_default();
    let _: AroundAngle = make_default();
}

#[test]
fn test_bracket_debug() {
    assert!(format!("{InnerParen:?}").contains("InnerParen"));
    assert!(format!("{AroundParen:?}").contains("AroundParen"));
    assert!(format!("{InnerSquareBracket:?}").contains("InnerSquareBracket"));
    assert!(format!("{AroundSquareBracket:?}").contains("AroundSquareBracket"));
    assert!(format!("{InnerBrace:?}").contains("InnerBrace"));
    assert!(format!("{AroundBrace:?}").contains("AroundBrace"));
    assert!(format!("{InnerAngle:?}").contains("InnerAngle"));
    assert!(format!("{AroundAngle:?}").contains("AroundAngle"));
}

#[test]
fn test_bracket_copy() {
    let a = InnerParen;
    let b = a;
    assert_eq!(b.id().name(), "inner-paren");
}

// =========================================================================
// MC/DC Coverage: count with more bracket types
// =========================================================================

#[test]
fn test_around_paren_with_count() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "((inner))");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 3).into();

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));

    let result = AroundParen.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_inner_square_bracket_with_count() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "[[inner]]");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 3).into();

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));

    let result = InnerSquareBracket.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_inner_brace_with_count() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "{{inner}}");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 3).into();

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));

    let result = InnerBrace.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_inner_angle_with_count() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "<<inner>>");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 3).into();

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));

    let result = InnerAngle.execute(&mut runtime, &args);
    assert!(result.is_success());
}

// =========================================================================
// MC/DC Coverage: no matching brackets for around variants
// =========================================================================

#[test]
fn test_no_matching_around_parens_is_noop() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "no parens here");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundParen.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_no_matching_around_square_brackets_is_noop() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "no brackets here");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundSquareBracket.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_no_matching_around_braces_is_noop() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "no braces here");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundBrace.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_no_matching_around_angles_is_noop() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "no angles here");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundAngle.execute(&mut runtime, &args);
    assert!(result.is_success());
}

// =========================================================================
// MC/DC Coverage: around invalid buffer for remaining types
// =========================================================================

#[test]
fn test_around_paren_invalid_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(999));
    let result = AroundParen.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_around_square_bracket_invalid_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(999));
    let result = AroundSquareBracket.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_around_brace_invalid_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(999));
    let result = AroundBrace.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_around_angle_invalid_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(999));
    let result = AroundAngle.execute(&mut runtime, &args);
    assert!(result.is_error());
}
