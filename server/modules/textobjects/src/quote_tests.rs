use {
    crate::{TEXTOBJECTS_MODULE, quote::*},
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
// Test Infrastructure
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
fn test_inner_double_quote_id() {
    let cmd = InnerDoubleQuote;
    assert_eq!(cmd.id().module(), &TEXTOBJECTS_MODULE);
    assert_eq!(cmd.id().name(), "inner-double-quote");
}

#[test]
fn test_around_double_quote_id() {
    let cmd = AroundDoubleQuote;
    assert_eq!(cmd.id().name(), "around-double-quote");
}

#[test]
fn test_inner_single_quote_id() {
    let cmd = InnerSingleQuote;
    assert_eq!(cmd.id().name(), "inner-single-quote");
}

#[test]
fn test_around_single_quote_id() {
    let cmd = AroundSingleQuote;
    assert_eq!(cmd.id().name(), "around-single-quote");
}

#[test]
fn test_inner_backtick_id() {
    let cmd = InnerBacktick;
    assert_eq!(cmd.id().name(), "inner-backtick");
}

#[test]
fn test_around_backtick_id() {
    let cmd = AroundBacktick;
    assert_eq!(cmd.id().name(), "around-backtick");
}

#[test]
fn test_all_commands_count() {
    let cmds = all_commands();
    assert_eq!(cmds.len(), 6);
}

// =========================================================================
// Description Tests
// =========================================================================

#[test]
fn test_quote_descriptions() {
    assert_eq!(InnerDoubleQuote.description(), "Inner double quote text object");
    assert_eq!(AroundDoubleQuote.description(), "Around double quote text object");
    assert_eq!(InnerSingleQuote.description(), "Inner single quote text object");
    assert_eq!(AroundSingleQuote.description(), "Around single quote text object");
    assert_eq!(InnerBacktick.description(), "Inner backtick text object");
    assert_eq!(AroundBacktick.description(), "Around backtick text object");
}

// =========================================================================
// Command Args Tests
// =========================================================================

#[test]
fn test_quote_commands_have_count_arg() {
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
fn test_inner_double_quote_no_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let args = CommandContext::new();
    let result = InnerDoubleQuote.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_inner_double_quote_invalid_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(999));
    let result = InnerDoubleQuote.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_all_quote_commands_no_buffer_error() {
    let kernel = KernelContext::default();
    let executor = StubExecutor;

    let commands: Vec<Box<dyn CommandHandler>> = all_commands();
    for cmd in &commands {
        let mut state = TestState::empty(test_mode());
        let mut runtime = state.runtime(&kernel, &executor);
        let args = CommandContext::new();
        let result = cmd.execute(&mut runtime, &args);
        assert!(result.is_error(), "Command {} should error without buffer", cmd.id());
    }
}

// =========================================================================
// Double Quote Tests
// =========================================================================

#[test]
fn test_inner_double_quote_basic() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say \"hello\" world");
    // Cursor on 'h' inside quotes at col 5
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerDoubleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_around_double_quote_basic() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say \"hello\" world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundDoubleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_inner_double_quote_operator_pending_stores_range() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say \"hello\" world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, operator_pending_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerDoubleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

#[test]
fn test_inner_double_quote_visual_mode_sets_selection() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say \"hello\" world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, visual_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerDoubleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
}

#[test]
fn test_around_double_quote_visual_mode_sets_selection() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say \"hello\" world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, visual_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundDoubleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
}

// =========================================================================
// Single Quote Tests
// =========================================================================

#[test]
fn test_inner_single_quote_basic() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say 'hello' world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerSingleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_around_single_quote_basic() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say 'hello' world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundSingleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_inner_single_quote_operator_pending_stores_range() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say 'hello' world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, operator_pending_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerSingleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

#[test]
fn test_inner_single_quote_visual_mode() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say 'hello' world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, visual_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerSingleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
}

// =========================================================================
// Backtick Tests
// =========================================================================

#[test]
fn test_inner_backtick_basic() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say `hello` world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerBacktick.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_around_backtick_basic() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say `hello` world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundBacktick.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_inner_backtick_operator_pending_stores_range() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say `hello` world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, operator_pending_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerBacktick.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

#[test]
fn test_around_backtick_visual_mode() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say `hello` world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, visual_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundBacktick.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
}

// =========================================================================
// Visual Line Mode Test
// =========================================================================

#[test]
fn test_inner_double_quote_visual_line_mode() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say \"hello\" world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, visual_line_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerDoubleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, SelectionMode::Line);
}

// =========================================================================
// Edge Cases
// =========================================================================

#[test]
fn test_empty_buffer_quote() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerDoubleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_no_matching_quotes_is_noop() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "no quotes here");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerDoubleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_visual_mode_does_not_store_operator_range() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say \"hello\" world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, visual_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerDoubleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    if let Some(ext_state) = ext_state {
        assert!(!ext_state.has_textobj_range());
    }
}

#[test]
fn test_inner_double_quote_visual_block_mode() {
    // Tests visual-block mode handling (line 38-44)
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say \"hello\" world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, visual_block_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerDoubleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, SelectionMode::Block);
}

#[test]
fn test_around_single_quote_visual_line_mode() {
    // Test around with visual-line mode
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say 'hello' world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, visual_line_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundSingleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, SelectionMode::Line);
}

#[test]
fn test_around_backtick_visual_block_mode() {
    // Test around with visual-block mode
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say `hello` world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, visual_block_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundBacktick.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, SelectionMode::Block);
}

#[test]
fn test_inner_double_quote_with_count() {
    // Tests count argument handling
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say \"hello\" world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));

    let result = InnerDoubleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());
}

// =========================================================================
// Additional coverage tests
// =========================================================================

#[test]
fn test_around_double_quote_operator_pending_stores_range() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say \"hello\" world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, operator_pending_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundDoubleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

#[test]
fn test_around_single_quote_operator_pending_stores_range() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say 'hello' world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, operator_pending_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundSingleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

#[test]
fn test_around_backtick_operator_pending_stores_range() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say `hello` world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, operator_pending_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundBacktick.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

#[test]
fn test_around_single_quote_visual_mode() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say 'hello' world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, visual_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundSingleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
}

#[test]
fn test_inner_backtick_visual_mode() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say `hello` world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, visual_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerBacktick.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
}

#[test]
fn test_inner_single_quote_no_buffer_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let args = CommandContext::new();
    let result = InnerSingleQuote.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_around_single_quote_no_buffer_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let args = CommandContext::new();
    let result = AroundSingleQuote.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_inner_backtick_no_buffer_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let args = CommandContext::new();
    let result = InnerBacktick.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_around_backtick_no_buffer_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let args = CommandContext::new();
    let result = AroundBacktick.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_no_matching_single_quotes_is_noop() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "no quotes here");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerSingleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_no_matching_backticks_is_noop() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "no backticks here");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerBacktick.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_inner_single_quote_invalid_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(999));
    let result = InnerSingleQuote.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_inner_backtick_invalid_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(999));
    let result = InnerBacktick.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_inner_single_quote_visual_block_mode() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say 'hello' world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, visual_block_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerSingleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, SelectionMode::Block);
}

#[test]
fn test_inner_backtick_visual_line_mode() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say `hello` world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, visual_line_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerBacktick.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, SelectionMode::Line);
}

#[test]
fn test_empty_buffer_single_quote() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerSingleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_empty_buffer_backtick() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerBacktick.execute(&mut runtime, &args);
    assert!(result.is_success());
}

// =========================================================================
// MC/DC Coverage: "No active window" path
// =========================================================================

#[test]
fn test_inner_double_quote_no_active_window_returns_error() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say \"hello\" world");
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

    let result = InnerDoubleQuote.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_around_double_quote_no_active_window_returns_error() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say \"hello\" world");
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

    let result = AroundDoubleQuote.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_inner_single_quote_no_active_window_returns_error() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say 'hello' world");
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

    let result = InnerSingleQuote.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_inner_backtick_no_active_window_returns_error() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say `hello` world");
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

    let result = InnerBacktick.execute(&mut runtime, &args);
    assert!(result.is_error());
}

// =========================================================================
// MC/DC Coverage: visual_selection_mode sub-conditions (visual-line, visual-block)
// These test the remaining combinations not covered above
// =========================================================================

#[test]
fn test_around_double_quote_visual_line_mode() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say \"hello\" world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, visual_line_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundDoubleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, SelectionMode::Line);
}

#[test]
fn test_around_double_quote_visual_block_mode() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say \"hello\" world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, visual_block_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundDoubleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, SelectionMode::Block);
}

#[test]
fn test_inner_single_quote_visual_line_mode() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say 'hello' world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, visual_line_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerSingleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, SelectionMode::Line);
}

#[test]
fn test_around_single_quote_visual_block_mode() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say 'hello' world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, visual_block_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundSingleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, SelectionMode::Block);
}

#[test]
fn test_inner_backtick_visual_block_mode() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say `hello` world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, visual_block_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerBacktick.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, SelectionMode::Block);
}

#[test]
fn test_around_backtick_visual_line_mode() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say `hello` world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, visual_line_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundBacktick.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, SelectionMode::Line);
}

// =========================================================================
// MC/DC Coverage: Default/Debug/Copy traits
// =========================================================================

#[test]
fn test_quote_defaults() {
    fn make_default<T: Default>() -> T {
        T::default()
    }
    let _: InnerDoubleQuote = make_default();
    let _: AroundDoubleQuote = make_default();
    let _: InnerSingleQuote = make_default();
    let _: AroundSingleQuote = make_default();
    let _: InnerBacktick = make_default();
    let _: AroundBacktick = make_default();
}

#[test]
fn test_quote_debug() {
    assert!(format!("{InnerDoubleQuote:?}").contains("InnerDoubleQuote"));
    assert!(format!("{AroundDoubleQuote:?}").contains("AroundDoubleQuote"));
    assert!(format!("{InnerSingleQuote:?}").contains("InnerSingleQuote"));
    assert!(format!("{AroundSingleQuote:?}").contains("AroundSingleQuote"));
    assert!(format!("{InnerBacktick:?}").contains("InnerBacktick"));
    assert!(format!("{AroundBacktick:?}").contains("AroundBacktick"));
}

#[test]
fn test_quote_copy() {
    let a = InnerDoubleQuote;
    let b = a;
    assert_eq!(b.id().name(), "inner-double-quote");

    let a = AroundDoubleQuote;
    let b = a;
    assert_eq!(b.id().name(), "around-double-quote");
}

// =========================================================================
// MC/DC Coverage: count with quote commands
// =========================================================================

#[test]
fn test_around_double_quote_with_count() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say \"hello\" world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));

    let result = AroundDoubleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_inner_single_quote_with_count() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say 'hello' world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));

    let result = InnerSingleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_inner_backtick_with_count() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "say `hello` world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 5).into();

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));

    let result = InnerBacktick.execute(&mut runtime, &args);
    assert!(result.is_success());
}

// =========================================================================
// MC/DC Coverage: no matching quotes for around variants
// =========================================================================

#[test]
fn test_no_matching_around_double_quotes_is_noop() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "no quotes here");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundDoubleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_no_matching_around_single_quotes_is_noop() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "no quotes here");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundSingleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_no_matching_around_backticks_is_noop() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "no backticks here");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundBacktick.execute(&mut runtime, &args);
    assert!(result.is_success());
}

// =========================================================================
// MC/DC Coverage: around invalid buffer for remaining types
// =========================================================================

#[test]
fn test_around_double_quote_invalid_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(999));
    let result = AroundDoubleQuote.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_around_single_quote_invalid_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(999));
    let result = AroundSingleQuote.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_around_backtick_invalid_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(999));
    let result = AroundBacktick.execute(&mut runtime, &args);
    assert!(result.is_error());
}

// =========================================================================
// MC/DC Coverage: around no_buffer for remaining types
// =========================================================================

#[test]
fn test_around_double_quote_no_buffer_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let args = CommandContext::new();
    let result = AroundDoubleQuote.execute(&mut runtime, &args);
    assert!(result.is_error());
}

// =========================================================================
// MC/DC Coverage: empty buffer for around variants
// =========================================================================

#[test]
fn test_around_double_quote_empty_buffer() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundDoubleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_around_single_quote_empty_buffer() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundSingleQuote.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_around_backtick_empty_buffer() {
    let kernel = create_test_kernel();
    let buffer_id = dual_register_buffer(&kernel, "");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AroundBacktick.execute(&mut runtime, &args);
    assert!(result.is_success());
}
