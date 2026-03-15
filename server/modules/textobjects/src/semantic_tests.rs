use {
    crate::{TEXTOBJECTS_MODULE, semantic::*},
    reovim_driver_command::{Command, CommandContext, CommandHandler},
    reovim_driver_session::{
        ClientId, ExtensionMap, OperatorPendingState, Session, SessionRuntime, Window,
        WindowLayout,
        api::{CommandExecutor, ExtensionApi},
        testing::StubExecutor,
    },
    reovim_kernel::{
        api::{
            ModeStack,
            v1::{BufferId, HistoryRing, Jumplist, KernelContext, MarkBank, ModeId, RegisterBank},
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
fn test_inner_function_id() {
    let cmd = InnerFunction;
    assert_eq!(cmd.id().module(), &TEXTOBJECTS_MODULE);
    assert_eq!(cmd.id().name(), "inner-function");
}

#[test]
fn test_around_function_id() {
    let cmd = AroundFunction;
    assert_eq!(cmd.id().module(), &TEXTOBJECTS_MODULE);
    assert_eq!(cmd.id().name(), "around-function");
}

#[test]
fn test_inner_class_id() {
    let cmd = InnerClass;
    assert_eq!(cmd.id().module(), &TEXTOBJECTS_MODULE);
    assert_eq!(cmd.id().name(), "inner-class");
}

#[test]
fn test_around_class_id() {
    let cmd = AroundClass;
    assert_eq!(cmd.id().module(), &TEXTOBJECTS_MODULE);
    assert_eq!(cmd.id().name(), "around-class");
}

#[test]
fn test_inner_argument_id() {
    let cmd = InnerArgument;
    assert_eq!(cmd.id().module(), &TEXTOBJECTS_MODULE);
    assert_eq!(cmd.id().name(), "inner-argument");
}

#[test]
fn test_around_argument_id() {
    let cmd = AroundArgument;
    assert_eq!(cmd.id().module(), &TEXTOBJECTS_MODULE);
    assert_eq!(cmd.id().name(), "around-argument");
}

#[test]
fn test_inner_conditional_id() {
    let cmd = InnerConditional;
    assert_eq!(cmd.id().module(), &TEXTOBJECTS_MODULE);
    assert_eq!(cmd.id().name(), "inner-conditional");
}

#[test]
fn test_around_conditional_id() {
    let cmd = AroundConditional;
    assert_eq!(cmd.id().module(), &TEXTOBJECTS_MODULE);
    assert_eq!(cmd.id().name(), "around-conditional");
}

#[test]
fn test_inner_loop_id() {
    let cmd = InnerLoop;
    assert_eq!(cmd.id().module(), &TEXTOBJECTS_MODULE);
    assert_eq!(cmd.id().name(), "inner-loop");
}

#[test]
fn test_around_loop_id() {
    let cmd = AroundLoop;
    assert_eq!(cmd.id().module(), &TEXTOBJECTS_MODULE);
    assert_eq!(cmd.id().name(), "around-loop");
}

#[test]
fn test_inner_comment_id() {
    let cmd = InnerComment;
    assert_eq!(cmd.id().module(), &TEXTOBJECTS_MODULE);
    assert_eq!(cmd.id().name(), "inner-comment");
}

#[test]
fn test_around_comment_id() {
    let cmd = AroundComment;
    assert_eq!(cmd.id().module(), &TEXTOBJECTS_MODULE);
    assert_eq!(cmd.id().name(), "around-comment");
}

#[test]
fn test_inner_block_ts_id() {
    let cmd = InnerBlockTs;
    assert_eq!(cmd.id().module(), &TEXTOBJECTS_MODULE);
    assert_eq!(cmd.id().name(), "inner-block-ts");
}

#[test]
fn test_around_block_ts_id() {
    let cmd = AroundBlockTs;
    assert_eq!(cmd.id().module(), &TEXTOBJECTS_MODULE);
    assert_eq!(cmd.id().name(), "around-block-ts");
}

// =========================================================================
// Description Tests
// =========================================================================

#[test]
fn test_all_semantic_commands_have_descriptions() {
    let cmds = all_commands();
    for cmd in &cmds {
        assert!(!cmd.description().is_empty(), "Command {} should have description", cmd.id());
    }
}

// =========================================================================
// Debug Derive Tests
// =========================================================================

#[test]
fn test_semantic_debug() {
    let inner_fn = InnerFunction;
    let around_fn = AroundFunction;
    let inner_cls = InnerClass;
    let around_cls = AroundClass;
    assert_eq!(format!("{inner_fn:?}"), "InnerFunction");
    assert_eq!(format!("{around_fn:?}"), "AroundFunction");
    assert_eq!(format!("{inner_cls:?}"), "InnerClass");
    assert_eq!(format!("{around_cls:?}"), "AroundClass");
}

#[test]
fn test_semantic_defaults() {
    fn assert_default<T: Default>(_: T) {}
    assert_default(InnerFunction);
    assert_default(AroundFunction);
    assert_default(InnerBlockTs);
    assert_default(AroundBlockTs);
}

#[test]
fn test_semantic_copy() {
    let cmd = InnerFunction;
    let cmd2 = cmd;
    assert_eq!(format!("{cmd2:?}"), "InnerFunction");
}

// =========================================================================
// Execution Tests (graceful degradation)
// =========================================================================

#[test]
fn test_no_buffer_returns_success() {
    let kernel = create_test_context();
    let executor = StubExecutor;
    let mode = test_mode();

    let mut state = TestState::empty(mode);
    let mut runtime = state.runtime(&kernel, &executor);

    // No buffer_id in context
    let ctx = CommandContext::new();
    let result = InnerFunction.execute(&mut runtime, &ctx);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);
}

#[test]
fn test_no_syntax_driver_returns_success() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "fn main() {}");
    let executor = StubExecutor;
    let mode = test_mode();

    let mut state = TestState::with_window(buffer_id, mode);
    let mut runtime = state.runtime(&kernel, &executor);

    // Buffer exists but no syntax driver installed
    let mut ctx = CommandContext::new();
    ctx.set_buffer_id(buffer_id);
    let result = InnerFunction.execute(&mut runtime, &ctx);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);
}

#[test]
fn test_no_syntax_driver_no_operator_state() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "fn main() {}");
    let executor = StubExecutor;
    let mode = test_mode();

    let mut state = TestState::with_window(buffer_id, mode);
    let mut runtime = state.runtime(&kernel, &executor);

    let mut ctx = CommandContext::new();
    ctx.set_buffer_id(buffer_id);
    AroundClass.execute(&mut runtime, &ctx);

    // No operator pending state should be set (no syntax driver)
    let op_state = runtime.ext::<OperatorPendingState>();
    assert!(op_state.is_none() || !op_state.unwrap().has_textobj_range());
}

#[test]
fn test_all_14_commands_graceful_without_driver() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "let x = 1;");
    let executor = StubExecutor;
    let mode = test_mode();

    let cmds = all_commands();
    assert_eq!(cmds.len(), 14);

    for cmd in &cmds {
        let mut state = TestState::with_window(buffer_id, mode.clone());
        let mut runtime = state.runtime(&kernel, &executor);
        let mut ctx = CommandContext::new();
        ctx.set_buffer_id(buffer_id);
        let result = cmd.execute(&mut runtime, &ctx);
        assert_eq!(
            result,
            reovim_driver_command::CommandResult::Success,
            "Command {} should gracefully return Success without syntax driver",
            cmd.id()
        );
    }
}
