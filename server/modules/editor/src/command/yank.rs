//! Yank commands.
//!
//! Provides yank (copy) commands:
//! - `YankLine` (yy, Y)

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{SessionRuntime, api::BufferApi},
    reovim_kernel::api::v1::{CommandId, Position, RegisterContent},
};

use crate::ids;

/// Yank current line(s) to register (yy, Y).
#[derive(Debug, Clone, Copy, Default)]
pub struct YankLine;

impl Command for YankLine {
    fn id(&self) -> CommandId {
        ids::YANK_LINE
    }

    fn description(&self) -> &'static str {
        "Yank current line"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![
            ArgSpec::optional("count", ArgKind::Count, "Number of lines to yank"),
            ArgSpec::optional("register", ArgKind::Register, "Target register"),
        ]
    }
}

impl CommandHandler for YankLine {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get cursor from per-client Window (#471)
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let pos = Position::new(window.cursor.line, window.cursor.column);
        let start_line = pos.line;

        // Get line count via BufferApi
        let Some(line_count) = runtime.buffer_line_count(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        // Can't yank from empty buffer
        if line_count == 0 {
            return CommandResult::Success;
        }

        let count = args.count().unwrap_or(1);

        // Collect lines to yank via BufferApi
        let end_line = (start_line + count).min(line_count);
        let mut yanked = String::new();
        for line_idx in start_line..end_line {
            if let Some(line) = runtime.buffer_line(buffer_id, line_idx) {
                yanked.push_str(&line);
                yanked.push('\n');
            }
        }

        // Store in per-client register with clipboard sync (#515)
        let content = RegisterContent::linewise(yanked);
        let register = args.register();
        runtime.store_register_with_sync(register, content);

        CommandResult::Success
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_driver_command::{ArgValue, Command, CommandContext},
        reovim_driver_session::{
            ClientId, ExtensionMap, Session, SessionRuntime, Window, WindowLayout,
            api::CommandExecutor,
        },
        reovim_kernel::api::{
            ServiceRegistry,
            v1::{
                Buffer, BufferError, BufferId, BufferManager, CommandId as KernelCommandId,
                EventBus, HistoryRing, KernelContext, MarkBank, ModeId, ModeStack, ModuleId,
                MotionEngine, OptionRegistry, RegisterBank, RwLock, TextObjectEngine,
            },
        },
        std::{collections::HashMap, sync::Arc},
    };

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

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_mode() -> ModeId {
        ModeId::new(ModuleId::new("test"), "normal")
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

    #[cfg_attr(coverage_nightly, coverage(off))]
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

    struct TestState {
        session: Session,
        mode_stack: ModeStack,
        windows: WindowLayout,
        extensions: ExtensionMap,
        compositor: Option<Box<dyn reovim_driver_display::layout::RootCompositor>>,
        registers: RegisterBank,
        clipboard_history: HistoryRing,
        local_marks: MarkBank,
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
                registers: RegisterBank::new(),
                clipboard_history: HistoryRing::new(),
                local_marks: MarkBank::new(),
            };
            let mut window = Window::new();
            window.buffer_id = Some(buffer_id);
            state.windows.add(window);
            state.session.set_active_buffer(Some(buffer_id));
            state
        }

        fn runtime<'a>(
            &'a mut self,
            kernel: &'a KernelContext,
            executor: &'a StubExecutor,
        ) -> SessionRuntime<'a> {
            SessionRuntime::new(
                &mut self.session,
                &mut self.mode_stack,
                &mut self.windows,
                &mut self.extensions,
                &mut self.compositor,
                &mut self.registers,
                &mut self.clipboard_history,
                &mut self.local_marks,
                kernel,
                executor,
            )
        }
    }

    // =========================================================================
    // Command trait tests
    // =========================================================================

    #[test]
    fn test_yank_line_id() {
        let cmd = YankLine;
        assert_eq!(cmd.id().name(), "yank-line");
    }

    #[test]
    fn test_yank_line_description() {
        assert_eq!(YankLine.description(), "Yank current line");
    }

    #[test]
    fn test_yank_line_args() {
        let args = YankLine.args();
        assert_eq!(args.len(), 2);
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
        assert_eq!(args[1].name, "register");
        assert_eq!(args[1].kind, ArgKind::Register);
    }

    #[test]
    fn test_yank_line_default() {
        let cmd = YankLine;
        assert_eq!(cmd.description(), "Yank current line");
    }

    // =========================================================================
    // Execute tests
    // =========================================================================

    #[test]
    fn test_yank_no_buffer_returns_error() {
        let kernel = create_test_context();
        let mode = test_mode();
        let mut session = Session::new(ClientId::new(1), mode.clone());
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        let mut compositor = None;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &mut compositor,
            &mut registers,
            &mut clipboard_history,
            &mut local_marks,
            &kernel,
            &executor,
        );
        let args = CommandContext::new();
        let result = YankLine.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_yank_no_window_returns_error() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mode = test_mode();
        let mut session = Session::new(ClientId::new(1), mode.clone());
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        let mut compositor = None;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &mut compositor,
            &mut registers,
            &mut clipboard_history,
            &mut local_marks,
            &kernel,
            &executor,
        );
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = YankLine.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_yank_buffer_not_found_returns_error() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set("buffer_id", ArgValue::BufferId(99999));
        let result = YankLine.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_yank_empty_buffer_returns_success() {
        let kernel = create_test_context();
        let buffer = Buffer::new();
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        // Empty buffer, line_count == 0, returns Success early
        let result = YankLine.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_yank_single_line() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = YankLine.execute(&mut runtime, &args);
        assert!(result.is_success());

        // Check register content
        drop(runtime);
        let content = state.registers.get().clone();
        assert!(content.is_linewise());
        assert_eq!(content.text, "hello world\n");
    }

    #[test]
    fn test_yank_with_count() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2\nline 3");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(2));

        let result = YankLine.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let content = state.registers.get().clone();
        assert!(content.is_linewise());
        assert_eq!(content.text, "line 1\nline 2\n");
    }

    #[test]
    fn test_yank_count_clamped_to_eof() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(100));

        let result = YankLine.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let content = state.registers.get().clone();
        assert!(content.is_linewise());
        assert_eq!(content.text, "line 1\nline 2\n");
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_yank_from_cursor_line() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2\nline 3");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(1, 0).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = YankLine.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let content = state.registers.get().clone();
        assert!(content.is_linewise());
        assert_eq!(content.text, "line 2\n");
    }

    // =========================================================================
    // Metadata tests
    // =========================================================================

    #[test]
    fn test_yank_line_debug() {
        let cmd = YankLine;
        let debug_str = format!("{cmd:?}");
        assert!(debug_str.contains("YankLine"));
    }

    // =========================================================================
    // Yank to named register
    // =========================================================================

    #[test]
    fn test_yank_to_named_register() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("register", ArgValue::Register('a'));

        let result = YankLine.execute(&mut runtime, &args);
        assert!(result.is_success());

        // Check named register content (per-client registers, #515)
        drop(runtime);
        let content = state.registers.get_by_name(Some('a')).cloned();
        let content = content.unwrap();
        assert!(content.is_linewise());
        assert_eq!(content.text, "hello world\n");
    }

    // =========================================================================
    // Yank multiple lines from middle
    // =========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_yank_multiple_lines_from_middle() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2\nline 3\nline 4\nline 5");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(1, 3).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(3));

        let result = YankLine.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let content = state.registers.get().clone();
        assert!(content.is_linewise());
        assert_eq!(content.text, "line 2\nline 3\nline 4\n");
    }

    // =========================================================================
    // Yank last line
    // =========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_yank_last_line() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2\nline 3");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(2, 0).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = YankLine.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let content = state.registers.get().clone();
        assert!(content.is_linewise());
        assert_eq!(content.text, "line 3\n");
    }

    // =========================================================================
    // Yank with cursor column > 0 (still yanks full line)
    // =========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_yank_line_with_cursor_at_column() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(0, 5).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = YankLine.execute(&mut runtime, &args);
        assert!(result.is_success());

        // Yank line should yank the entire line regardless of column
        drop(runtime);
        let content = state.registers.get().clone();
        assert!(content.is_linewise());
        assert_eq!(content.text, "hello world\n");
    }
}
