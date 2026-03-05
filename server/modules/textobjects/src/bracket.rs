//! Bracket text object commands.
//!
//! Implements text objects: `i(`, `a(`, `i[`, `a[`, `i{`, `a{`, `i<`, `a<`.
//!
//! These commands select text within or around bracket pairs.
//!
//! # Mode-Aware Behavior (Epic #465)
//!
//! Text objects behave differently based on the current mode:
//! - **Operator-pending mode** (d, y, c): Store range for operator consumption
//! - **Visual mode** (v, V, Ctrl-V): Update selection to cover the text object

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{
        OperatorPendingState, SessionRuntime, TextObjRange,
        api::{ExtensionApi, ModeApi, Selection, SelectionMode},
    },
    reovim_kernel::api::v1::{CommandId, Position, TextObject, TextObjectEngine},
};

use crate::ids;

// =============================================================================
// Helper functions
// =============================================================================

/// Check if the current mode is a visual mode.
fn is_visual_mode(runtime: &SessionRuntime<'_>) -> bool {
    let mode = runtime.current_mode();
    let mode_name = mode.name();
    mode_name == "visual" || mode_name == "visual-line" || mode_name == "visual-block"
}

/// Get the selection mode for the current visual mode.
fn visual_selection_mode(runtime: &SessionRuntime<'_>) -> SelectionMode {
    let mode = runtime.current_mode();
    match mode.name() {
        "visual-line" => SelectionMode::Line,
        "visual-block" => SelectionMode::Block,
        _ => SelectionMode::Character,
    }
}

/// Execute a bracket text object and store the range for operator consumption,
/// or update the selection if in visual mode.
#[cfg_attr(coverage_nightly, coverage(off))]
fn execute_bracket_textobj(
    runtime: &mut SessionRuntime<'_>,
    args: &CommandContext,
    text_object: TextObject,
) -> CommandResult {
    let Some(buffer_id) = args.buffer_id() else {
        return CommandResult::error("No active buffer");
    };

    let count = args.count().unwrap_or(1);

    // Get cursor from per-client Window (#471)
    let Some(window) = runtime.windows().active() else {
        return CommandResult::error("No active window");
    };
    let pos = Position::new(window.cursor.line, window.cursor.column);

    // Calculate text object range using with_buffer_read callback
    let range_result = runtime.with_buffer_read(buffer_id, |buffer| {
        TextObjectEngine::range(buffer, pos, text_object, count)
    });

    let Some(range_result) = range_result else {
        return CommandResult::error("Buffer not found");
    };

    let Some((start, end)) = range_result else {
        return CommandResult::Success; // No-op if no matching brackets
    };

    // Kernel returns inclusive end, convert to exclusive
    let end_exclusive = Position::new(end.line, end.column + 1);

    // Check if we're in visual mode
    if is_visual_mode(runtime) {
        // Visual mode: update the selection to cover the text object
        let sel_mode = visual_selection_mode(runtime);
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.selection = Some(Selection::new(start, end_exclusive, sel_mode));
            // Move cursor to end of selection
            window.cursor = end.into();
        }
    } else {
        // Operator-pending mode: store range for operator consumption (Epic #465)
        // The operator resolver's on_command_complete will take() this range
        let state = runtime.ext_mut::<OperatorPendingState>();
        state.set_textobj_range(TextObjRange::characterwise(start, end_exclusive));
    }

    CommandResult::Success
}

// =============================================================================
// Inner Parenthesis (i(, i), ib)
// =============================================================================

/// Inner parenthesis text object.
///
/// Selects text inside parentheses, excluding the parens.
#[derive(Debug, Clone, Copy, Default)]
pub struct InnerParen;

impl Command for InnerParen {
    fn id(&self) -> CommandId {
        ids::INNER_PAREN
    }

    fn description(&self) -> &'static str {
        "Inner parenthesis text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional("count", ArgKind::Count, "Nesting level")]
    }
}

impl CommandHandler for InnerParen {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_bracket_textobj(runtime, args, TextObject::InnerBracket('('))
    }
}

// =============================================================================
// Around Parenthesis (a(, a), ab)
// =============================================================================

/// Around parenthesis text object.
///
/// Selects text including the parentheses.
#[derive(Debug, Clone, Copy, Default)]
pub struct AroundParen;

impl Command for AroundParen {
    fn id(&self) -> CommandId {
        ids::AROUND_PAREN
    }

    fn description(&self) -> &'static str {
        "Around parenthesis text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional("count", ArgKind::Count, "Nesting level")]
    }
}

impl CommandHandler for AroundParen {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_bracket_textobj(runtime, args, TextObject::ABracket('('))
    }
}

// =============================================================================
// Inner Square Bracket (i[, i])
// =============================================================================

/// Inner square bracket text object.
///
/// Selects text inside square brackets, excluding the brackets.
#[derive(Debug, Clone, Copy, Default)]
pub struct InnerSquareBracket;

impl Command for InnerSquareBracket {
    fn id(&self) -> CommandId {
        ids::INNER_BRACKET
    }

    fn description(&self) -> &'static str {
        "Inner square bracket text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional("count", ArgKind::Count, "Nesting level")]
    }
}

impl CommandHandler for InnerSquareBracket {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_bracket_textobj(runtime, args, TextObject::InnerBracket('['))
    }
}

// =============================================================================
// Around Square Bracket (a[, a])
// =============================================================================

/// Around square bracket text object.
///
/// Selects text including the square brackets.
#[derive(Debug, Clone, Copy, Default)]
pub struct AroundSquareBracket;

impl Command for AroundSquareBracket {
    fn id(&self) -> CommandId {
        ids::AROUND_BRACKET
    }

    fn description(&self) -> &'static str {
        "Around square bracket text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional("count", ArgKind::Count, "Nesting level")]
    }
}

impl CommandHandler for AroundSquareBracket {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_bracket_textobj(runtime, args, TextObject::ABracket('['))
    }
}

// =============================================================================
// Inner Brace (i{, i}, iB)
// =============================================================================

/// Inner brace text object.
///
/// Selects text inside braces, excluding the braces.
#[derive(Debug, Clone, Copy, Default)]
pub struct InnerBrace;

impl Command for InnerBrace {
    fn id(&self) -> CommandId {
        ids::INNER_BRACE
    }

    fn description(&self) -> &'static str {
        "Inner brace text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional("count", ArgKind::Count, "Nesting level")]
    }
}

impl CommandHandler for InnerBrace {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_bracket_textobj(runtime, args, TextObject::InnerBracket('{'))
    }
}

// =============================================================================
// Around Brace (a{, a}, aB)
// =============================================================================

/// Around brace text object.
///
/// Selects text including the braces.
#[derive(Debug, Clone, Copy, Default)]
pub struct AroundBrace;

impl Command for AroundBrace {
    fn id(&self) -> CommandId {
        ids::AROUND_BRACE
    }

    fn description(&self) -> &'static str {
        "Around brace text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional("count", ArgKind::Count, "Nesting level")]
    }
}

impl CommandHandler for AroundBrace {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_bracket_textobj(runtime, args, TextObject::ABracket('{'))
    }
}

// =============================================================================
// Inner Angle Bracket (i<, i>)
// =============================================================================

/// Inner angle bracket text object.
///
/// Selects text inside angle brackets, excluding the brackets.
#[derive(Debug, Clone, Copy, Default)]
pub struct InnerAngle;

impl Command for InnerAngle {
    fn id(&self) -> CommandId {
        ids::INNER_ANGLE
    }

    fn description(&self) -> &'static str {
        "Inner angle bracket text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional("count", ArgKind::Count, "Nesting level")]
    }
}

impl CommandHandler for InnerAngle {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_bracket_textobj(runtime, args, TextObject::InnerBracket('<'))
    }
}

// =============================================================================
// Around Angle Bracket (a<, a>)
// =============================================================================

/// Around angle bracket text object.
///
/// Selects text including the angle brackets.
#[derive(Debug, Clone, Copy, Default)]
pub struct AroundAngle;

impl Command for AroundAngle {
    fn id(&self) -> CommandId {
        ids::AROUND_ANGLE
    }

    fn description(&self) -> &'static str {
        "Around angle bracket text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional("count", ArgKind::Count, "Nesting level")]
    }
}

impl CommandHandler for AroundAngle {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_bracket_textobj(runtime, args, TextObject::ABracket('<'))
    }
}

// =============================================================================
// Command Registration
// =============================================================================

/// Get all bracket text object commands as boxed trait objects.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(InnerParen),
        Box::new(AroundParen),
        Box::new(InnerSquareBracket),
        Box::new(AroundSquareBracket),
        Box::new(InnerBrace),
        Box::new(AroundBrace),
        Box::new(InnerAngle),
        Box::new(AroundAngle),
    ]
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::TEXTOBJECTS_MODULE,
        reovim_driver_command::ArgValue,
        reovim_driver_session::{
            ClientId, ExtensionMap, OperatorPendingState, Session, Window, WindowLayout,
            api::{CommandExecutor, SelectionMode},
        },
        reovim_kernel::api::{
            ModeStack, ServiceRegistry,
            v1::{
                Buffer, BufferError, BufferId, BufferManager, CommandId as KernelCommandId,
                EventBus, HistoryRing, KernelContext, MarkBank, ModeId, ModuleId, MotionEngine,
                OptionRegistry, Position, RegisterBank, RwLock, TextObjectEngine,
            },
        },
        std::{collections::HashMap, sync::Arc},
    };

    // =========================================================================
    // Test Infrastructure (mirrors word.rs pattern)
    // =========================================================================

    struct TestBufferManager {
        buffers: RwLock<HashMap<BufferId, Arc<RwLock<Buffer>>>>,
    }

    impl TestBufferManager {
        fn new() -> Self {
            Self {
                buffers: RwLock::new(HashMap::new()),
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl BufferManager for TestBufferManager {
        fn get(&self, id: BufferId) -> Option<Arc<RwLock<Buffer>>> {
            self.buffers.read().get(&id).cloned()
        }

        fn create(&self) -> BufferId {
            let id = BufferId::new();
            let buffer = Arc::new(RwLock::new(Buffer::new()));
            self.buffers.write().insert(id, buffer);
            id
        }

        fn register(&self, buffer: Buffer) -> BufferId {
            let id = BufferId::new();
            let buffer = Arc::new(RwLock::new(buffer));
            self.buffers.write().insert(id, buffer);
            id
        }

        fn unregister(&self, id: BufferId) -> Result<Buffer, BufferError> {
            self.buffers
                .write()
                .remove(&id)
                .map_or(Err(BufferError::NotFound(id)), |arc_buffer| {
                    Arc::try_unwrap(arc_buffer)
                        .map_or_else(|arc| Ok(arc.read().clone()), |rwlock| Ok(rwlock.into_inner()))
                })
        }

        fn list(&self) -> Vec<BufferId> {
            self.buffers.read().keys().copied().collect()
        }

        fn count(&self) -> usize {
            self.buffers.read().len()
        }
    }

    struct StubExecutor;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl CommandExecutor for StubExecutor {
        fn execute(
            &self,
            _cmd: &KernelCommandId,
            _ctx: &CommandContext,
            _kernel: &KernelContext,
        ) -> Option<CommandResult> {
            Some(CommandResult::Success)
        }
    }

    fn test_mode() -> ModeId {
        ModeId::new(ModuleId::new("test"), "normal")
    }

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

    fn create_test_context() -> KernelContext {
        KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(RwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::default()),
            Arc::new(ServiceRegistry::new()),
        )
    }

    fn setup_buffer(ctx: &KernelContext, content: &str) -> BufferId {
        let buffer = Buffer::from_string(content);
        ctx.buffers.register(buffer)
    }

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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo(bar)baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo(bar)baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo(bar)baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo(bar)baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo(bar)baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "no brackets here");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo[bar]baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo[bar]baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo[bar]baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo[bar]baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo{bar}baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo{bar}baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo{bar}baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo{bar}baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo<bar>baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo<bar>baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo<bar>baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo<bar>baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo(bar)baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo(bar)baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "((inner))");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo(bar)baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "func(\n  arg1,\n  arg2\n)");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo(bar)baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo[bar]baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo{bar}baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo<bar>baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo{bar}baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo<bar>baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo[bar]baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo[bar]baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo{bar}baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo<bar>baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "no brackets here");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "no braces here");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "no angles here");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "[[inner]]");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "{{inner}}");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "<<inner>>");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "arr[\n  elem1,\n  elem2\n]");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "fn {\n  body;\n}");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "tag<\n  content\n>");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "((inner))");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo(bar)baz");
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

        let result = InnerParen.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_around_paren_no_active_window_returns_error() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo(bar)baz");
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

        let result = AroundParen.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_inner_square_bracket_no_active_window_returns_error() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo[bar]baz");
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

        let result = InnerSquareBracket.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_inner_brace_no_active_window_returns_error() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo{bar}baz");
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

        let result = InnerBrace.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_inner_angle_no_active_window_returns_error() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo<bar>baz");
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

        let result = InnerAngle.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    // =========================================================================
    // MC/DC Coverage: visual-line and visual-block for remaining bracket types
    // =========================================================================

    #[test]
    fn test_around_paren_visual_line_mode() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo(bar)baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo(bar)baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo[bar]baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo[bar]baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo{bar}baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo{bar}baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo<bar>baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo<bar>baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo[bar]baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo{bar}baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "foo<bar>baz");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "((inner))");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "[[inner]]");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "{{inner}}");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "<<inner>>");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "no parens here");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "no brackets here");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "no braces here");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "no angles here");
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
}
