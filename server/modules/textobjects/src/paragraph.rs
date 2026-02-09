//! Paragraph text object commands.
//!
//! Implements text objects: `ip`, `ap`.
//!
//! These commands select paragraph regions (contiguous non-blank lines).

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{
        OperatorPendingState, SessionRuntime, TextObjRange, api::ExtensionApi,
    },
    reovim_kernel::api::v1::{CommandId, Position, TextObject, TextObjectEngine},
};

use crate::ids;

// =============================================================================
// Helper function
// =============================================================================

/// Execute a paragraph text object and store the range for operator consumption.
fn execute_paragraph_textobj(
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
        return CommandResult::Success; // No-op if no paragraph found
    };

    // Kernel returns inclusive end, convert to exclusive
    let end_exclusive = Position::new(end.line, end.column + 1);

    // Store range in OperatorPendingState for operator consumption (Epic #465)
    // Paragraph operations are linewise - they affect entire lines
    let state = runtime.ext_mut::<OperatorPendingState>();
    state.set_textobj_range(TextObjRange::linewise(start, end_exclusive));

    CommandResult::Success
}

// =============================================================================
// Inner Paragraph (ip)
// =============================================================================

/// Inner paragraph text object.
///
/// Selects contiguous non-blank lines.
#[derive(Debug, Clone, Copy, Default)]
pub struct InnerParagraph;

impl Command for InnerParagraph {
    fn id(&self) -> CommandId {
        ids::INNER_PARAGRAPH
    }

    fn description(&self) -> &'static str {
        "Inner paragraph text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of paragraphs",
        )]
    }
}

impl CommandHandler for InnerParagraph {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_paragraph_textobj(runtime, args, TextObject::InnerParagraph)
    }
}

// =============================================================================
// Around Paragraph (ap)
// =============================================================================

/// Around paragraph text object.
///
/// Selects contiguous non-blank lines plus trailing blank lines.
#[derive(Debug, Clone, Copy, Default)]
pub struct AroundParagraph;

impl Command for AroundParagraph {
    fn id(&self) -> CommandId {
        ids::AROUND_PARAGRAPH
    }

    fn description(&self) -> &'static str {
        "Around paragraph text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of paragraphs",
        )]
    }
}

impl CommandHandler for AroundParagraph {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_paragraph_textobj(runtime, args, TextObject::AParagraph)
    }
}

// =============================================================================
// Command Registration
// =============================================================================

/// Get all paragraph text object commands as boxed trait objects.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![Box::new(InnerParagraph), Box::new(AroundParagraph)]
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
            api::CommandExecutor,
        },
        reovim_kernel::api::{
            ModeStack, ServiceRegistry,
            v1::{
                Buffer, BufferError, BufferId, BufferManager, CommandId as KernelCommandId,
                EventBus, KernelContext, MarkBank, ModeId, ModuleId, MotionEngine, OptionRegistry,
                Position, RegisterBank, RwLock, TextObjectEngine,
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

    fn create_test_context() -> KernelContext {
        KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(RwLock::new(RegisterBank::new())),
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
            }
        }

        fn runtime<'a>(
            &'a mut self,
            kernel: &'a KernelContext,
            executor: &'a dyn CommandExecutor,
        ) -> SessionRuntime<'a> {
            SessionRuntime::new(
                &mut self.session,
                &mut self.mode_stack,
                &mut self.windows,
                &mut self.extensions,
                &mut self.compositor,
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
}
