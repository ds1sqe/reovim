//! Quote text object commands.
//!
//! Implements text objects: `i"`, `a"`, `i'`, `a'`, `` i` ``, `` a` ``.
//!
//! These commands select text within or around quote characters.
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

/// Execute a quote text object and store the range for operator consumption,
/// or update the selection if in visual mode.
#[cfg_attr(coverage_nightly, coverage(off))]
fn execute_quote_textobj(
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
        return CommandResult::Success; // No-op if no matching quotes
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
// Inner Double Quote (i")
// =============================================================================

/// Inner double quote text object.
///
/// Selects text inside double quotes, excluding the quotes.
#[derive(Debug, Clone, Copy, Default)]
pub struct InnerDoubleQuote;

impl Command for InnerDoubleQuote {
    fn id(&self) -> CommandId {
        ids::INNER_DOUBLE_QUOTE
    }

    fn description(&self) -> &'static str {
        "Inner double quote text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Quote nesting level",
        )]
    }
}

impl CommandHandler for InnerDoubleQuote {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_quote_textobj(runtime, args, TextObject::InnerQuote('"'))
    }
}

// =============================================================================
// Around Double Quote (a")
// =============================================================================

/// Around double quote text object.
///
/// Selects text including the double quotes.
#[derive(Debug, Clone, Copy, Default)]
pub struct AroundDoubleQuote;

impl Command for AroundDoubleQuote {
    fn id(&self) -> CommandId {
        ids::AROUND_DOUBLE_QUOTE
    }

    fn description(&self) -> &'static str {
        "Around double quote text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Quote nesting level",
        )]
    }
}

impl CommandHandler for AroundDoubleQuote {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_quote_textobj(runtime, args, TextObject::AQuote('"'))
    }
}

// =============================================================================
// Inner Single Quote (i')
// =============================================================================

/// Inner single quote text object.
///
/// Selects text inside single quotes, excluding the quotes.
#[derive(Debug, Clone, Copy, Default)]
pub struct InnerSingleQuote;

impl Command for InnerSingleQuote {
    fn id(&self) -> CommandId {
        ids::INNER_SINGLE_QUOTE
    }

    fn description(&self) -> &'static str {
        "Inner single quote text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Quote nesting level",
        )]
    }
}

impl CommandHandler for InnerSingleQuote {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_quote_textobj(runtime, args, TextObject::InnerQuote('\''))
    }
}

// =============================================================================
// Around Single Quote (a')
// =============================================================================

/// Around single quote text object.
///
/// Selects text including the single quotes.
#[derive(Debug, Clone, Copy, Default)]
pub struct AroundSingleQuote;

impl Command for AroundSingleQuote {
    fn id(&self) -> CommandId {
        ids::AROUND_SINGLE_QUOTE
    }

    fn description(&self) -> &'static str {
        "Around single quote text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Quote nesting level",
        )]
    }
}

impl CommandHandler for AroundSingleQuote {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_quote_textobj(runtime, args, TextObject::AQuote('\''))
    }
}

// =============================================================================
// Inner Backtick (i`)
// =============================================================================

/// Inner backtick text object.
///
/// Selects text inside backticks, excluding the backticks.
#[derive(Debug, Clone, Copy, Default)]
pub struct InnerBacktick;

impl Command for InnerBacktick {
    fn id(&self) -> CommandId {
        ids::INNER_BACKTICK
    }

    fn description(&self) -> &'static str {
        "Inner backtick text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Quote nesting level",
        )]
    }
}

impl CommandHandler for InnerBacktick {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_quote_textobj(runtime, args, TextObject::InnerQuote('`'))
    }
}

// =============================================================================
// Around Backtick (a`)
// =============================================================================

/// Around backtick text object.
///
/// Selects text including the backticks.
#[derive(Debug, Clone, Copy, Default)]
pub struct AroundBacktick;

impl Command for AroundBacktick {
    fn id(&self) -> CommandId {
        ids::AROUND_BACKTICK
    }

    fn description(&self) -> &'static str {
        "Around backtick text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Quote nesting level",
        )]
    }
}

impl CommandHandler for AroundBacktick {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_quote_textobj(runtime, args, TextObject::AQuote('`'))
    }
}

// =============================================================================
// Command Registration
// =============================================================================

/// Get all quote text object commands as boxed trait objects.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(InnerDoubleQuote),
        Box::new(AroundDoubleQuote),
        Box::new(InnerSingleQuote),
        Box::new(AroundSingleQuote),
        Box::new(InnerBacktick),
        Box::new(AroundBacktick),
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
    // Test Infrastructure
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say \"hello\" world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say \"hello\" world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say \"hello\" world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say \"hello\" world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say \"hello\" world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say 'hello' world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say 'hello' world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say 'hello' world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say 'hello' world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say `hello` world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say `hello` world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say `hello` world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say `hello` world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say \"hello\" world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "no quotes here");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say \"hello\" world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say \"hello\" world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say 'hello' world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say `hello` world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say \"hello\" world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say \"hello\" world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say 'hello' world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say `hello` world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say 'hello' world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say `hello` world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "no quotes here");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "no backticks here");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say 'hello' world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say `hello` world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say \"hello\" world");
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

        let result = InnerDoubleQuote.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_around_double_quote_no_active_window_returns_error() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say \"hello\" world");
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

        let result = AroundDoubleQuote.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_inner_single_quote_no_active_window_returns_error() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say 'hello' world");
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

        let result = InnerSingleQuote.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_inner_backtick_no_active_window_returns_error() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say `hello` world");
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

        let result = InnerBacktick.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    // =========================================================================
    // MC/DC Coverage: visual_selection_mode sub-conditions (visual-line, visual-block)
    // These test the remaining combinations not covered above
    // =========================================================================

    #[test]
    fn test_around_double_quote_visual_line_mode() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say \"hello\" world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say \"hello\" world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say 'hello' world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say 'hello' world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say `hello` world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say `hello` world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say \"hello\" world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say 'hello' world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "say `hello` world");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "no quotes here");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "no quotes here");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "no backticks here");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "");
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
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "");
        let mut state = TestState::with_window(buffer_id, test_mode());
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = AroundBacktick.execute(&mut runtime, &args);
        assert!(result.is_success());
    }
}
