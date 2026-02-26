//! Undo/Redo commands.
//!
//! Provides commands for undo/redo operations:
//! - `UndoCommand` (u)
//! - `RedoCommand` (Ctrl-R)
//!
//! # Architecture
//!
//! These commands use the `UndoApi` trait to perform undo/redo operations.
//! The actual undo tree is managed by the undo module's `UndoRegistry`.

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{
        SessionRuntime,
        api::{BufferApi, UndoApi},
    },
    reovim_kernel::api::v1::CommandId,
};

use crate::ids;

/// Undo the last change.
///
/// Uses `UndoApi` to traverse the undo tree and apply inverse edits.
/// Supports count argument to undo multiple changes at once.
#[derive(Debug, Clone, Copy, Default)]
pub struct UndoCommand;

impl Command for UndoCommand {
    fn id(&self) -> CommandId {
        ids::UNDO
    }

    fn description(&self) -> &'static str {
        "Undo the last change"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of changes to undo",
        )]
    }

    fn names(&self) -> &[&'static str] {
        &["u", "undo"]
    }
}

impl CommandHandler for UndoCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer) = runtime.active_buffer() else {
            return CommandResult::Error("No active buffer".to_string());
        };

        let count = args.count().unwrap_or(1);
        let mut undone = 0;

        for _ in 0..count {
            // #471: Try per-client undo first, fall back to regular undo
            // undo_mine() returns None if no owner is set OR nothing to undo
            let result = runtime.undo_mine(buffer).or_else(|| runtime.undo(buffer));

            if result.is_some() {
                undone += 1;
            } else {
                break; // Nothing left to undo
            }
        }

        if undone == 0 {
            CommandResult::Error("Already at oldest change".to_string())
        } else {
            CommandResult::Success
        }
    }
}

/// Redo the last undone change.
///
/// Uses `UndoApi` to traverse the undo tree and apply edits.
/// Supports count argument to redo multiple changes at once.
#[derive(Debug, Clone, Copy, Default)]
pub struct RedoCommand;

impl Command for RedoCommand {
    fn id(&self) -> CommandId {
        ids::REDO
    }

    fn description(&self) -> &'static str {
        "Redo the last undone change"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of changes to redo",
        )]
    }

    fn names(&self) -> &[&'static str] {
        &["redo"]
    }
}

impl CommandHandler for RedoCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer) = runtime.active_buffer() else {
            return CommandResult::Error("No active buffer".to_string());
        };

        let count = args.count().unwrap_or(1);
        let mut redone = 0;

        for _ in 0..count {
            // #471: Try per-client redo first, fall back to regular redo
            // redo_mine() returns None if no owner is set OR nothing to redo
            let result = runtime.redo_mine(buffer).or_else(|| runtime.redo(buffer));

            if result.is_some() {
                redone += 1;
            } else {
                break; // Nothing left to redo
            }
        }

        if redone == 0 {
            CommandResult::Error("Already at newest change".to_string())
        } else {
            CommandResult::Success
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_driver_command::{ArgKind, ArgValue, CommandContext},
        reovim_driver_session::{
            ClientId, ExtensionMap, Session, SessionRuntime, Window, WindowLayout,
            api::CommandExecutor,
        },
        reovim_driver_undo::{UndoKey, UndoProviderRegistry},
        reovim_kernel::api::{
            ServiceRegistry,
            v1::{
                Buffer, BufferError, BufferId, BufferManager, CommandId as KernelCommandId, Edit,
                EventBus, HistoryRing, KernelContext, MarkBank, ModeId, ModeStack, ModuleId,
                MotionEngine, OptionRegistry, Position, RegisterBank, RwLock, TextObjectEngine,
                UndoResult,
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
            executor: &'a dyn CommandExecutor,
        ) -> SessionRuntime<'a> {
            SessionRuntime::new(
                &mut self.session,
                reovim_driver_session::ClientContext {
                    mode_stack: &mut self.mode_stack,
                    windows: &mut self.windows,
                    extensions: &mut self.extensions,
                    compositor: &mut self.compositor,
                    registers: &mut self.registers,
                    clipboard_history: &mut self.clipboard_history,
                    local_marks: &mut self.local_marks,
                },
                kernel,
                executor,
            )
        }
    }

    // =========================================================================
    // UndoCommand tests
    // =========================================================================

    #[test]
    fn test_undo_id() {
        let cmd = UndoCommand;
        assert_eq!(cmd.id().name(), "undo");
    }

    #[test]
    fn test_undo_description() {
        assert_eq!(UndoCommand.description(), "Undo the last change");
    }

    #[test]
    fn test_undo_args() {
        let args = UndoCommand.args();
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
    }

    #[test]
    fn test_undo_names() {
        let names = UndoCommand.names();
        assert!(names.contains(&"u"));
        assert!(names.contains(&"undo"));
    }

    #[test]
    fn test_undo_no_active_buffer_returns_error() {
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
            reovim_driver_session::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
            },
            &kernel,
            &executor,
        );
        let args = CommandContext::new();
        let result = UndoCommand.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_undo_nothing_to_undo_returns_error() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let args = CommandContext::new();
        // No changes made, so nothing to undo
        let result = UndoCommand.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_undo_with_count_nothing_to_undo() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set("count", ArgValue::Count(5));
        let result = UndoCommand.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    // =========================================================================
    // RedoCommand tests
    // =========================================================================

    #[test]
    fn test_redo_id() {
        let cmd = RedoCommand;
        assert_eq!(cmd.id().name(), "redo");
    }

    #[test]
    fn test_redo_description() {
        assert_eq!(RedoCommand.description(), "Redo the last undone change");
    }

    #[test]
    fn test_redo_args() {
        let args = RedoCommand.args();
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
    }

    #[test]
    fn test_redo_names() {
        let names = RedoCommand.names();
        assert!(names.contains(&"redo"));
    }

    #[test]
    fn test_redo_no_active_buffer_returns_error() {
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
            reovim_driver_session::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
            },
            &kernel,
            &executor,
        );
        let args = CommandContext::new();
        let result = RedoCommand.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_redo_nothing_to_redo_returns_error() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let args = CommandContext::new();
        // No undo done, so nothing to redo
        let result = RedoCommand.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_redo_with_count_nothing_to_redo() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set("count", ArgValue::Count(3));
        let result = RedoCommand.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    // =========================================================================
    // UndoCommand Default trait
    // =========================================================================

    // =========================================================================
    // Error message content tests
    // =========================================================================

    #[test]
    fn test_undo_no_buffer_error_message() {
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
            reovim_driver_session::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
            },
            &kernel,
            &executor,
        );
        let args = CommandContext::new();
        let result = UndoCommand.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Error("No active buffer".to_string()));
    }

    #[test]
    fn test_redo_no_buffer_error_message() {
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
            reovim_driver_session::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
            },
            &kernel,
            &executor,
        );
        let args = CommandContext::new();
        let result = RedoCommand.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Error("No active buffer".to_string()));
    }

    #[test]
    fn test_undo_nothing_to_undo_error_message() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let args = CommandContext::new();
        let result = UndoCommand.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Error("Already at oldest change".to_string()));
    }

    #[test]
    fn test_redo_nothing_to_redo_error_message() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let args = CommandContext::new();
        let result = RedoCommand.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Error("Already at newest change".to_string()));
    }

    // =========================================================================
    // Count default (count=None uses 1)
    // =========================================================================

    #[test]
    fn test_undo_default_count_is_one() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("test");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        // No count set - should default to 1
        let args = CommandContext::new();
        let result = UndoCommand.execute(&mut runtime, &args);
        // With no undo history, still gets error "Already at oldest change"
        assert!(result.is_error());
    }

    #[test]
    fn test_redo_default_count_is_one() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("test");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        // No count set - should default to 1
        let args = CommandContext::new();
        let result = RedoCommand.execute(&mut runtime, &args);
        // With no redo history, still gets error "Already at newest change"
        assert!(result.is_error());
    }

    // =========================================================================
    // Debug/Clone trait tests
    // =========================================================================

    #[test]
    fn test_undo_command_debug() {
        let cmd = UndoCommand;
        let debug_str = format!("{cmd:?}");
        assert!(debug_str.contains("UndoCommand"));
    }

    #[test]
    fn test_redo_command_debug() {
        let cmd = RedoCommand;
        let debug_str = format!("{cmd:?}");
        assert!(debug_str.contains("RedoCommand"));
    }

    #[test]
    fn test_undo_command_clone() {
        let cmd = UndoCommand;
        let cloned = cmd;
        assert_eq!(cloned.id().name(), "undo");
    }

    #[test]
    fn test_redo_command_clone() {
        let cmd = RedoCommand;
        let cloned = cmd;
        assert_eq!(cloned.id().name(), "redo");
    }

    // =========================================================================
    // Mock UndoProvider for success-path testing
    // =========================================================================

    struct MockUndoProvider;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl reovim_driver_undo::UndoProvider for MockUndoProvider {
        fn undo(&self, _buffer_id: BufferId) -> Option<UndoResult> {
            Some(UndoResult {
                edits: vec![],
                cursor: Position::new(0, 0),
            })
        }

        fn redo(&self, _buffer_id: BufferId) -> Option<UndoResult> {
            Some(UndoResult {
                edits: vec![],
                cursor: Position::new(0, 0),
            })
        }

        fn redo_branch(&self, _buffer_id: BufferId, _branch_idx: usize) -> Option<UndoResult> {
            None
        }

        fn record(
            &self,
            _buffer_id: BufferId,
            _edits: Vec<Edit>,
            _cursor_before: Position,
            _cursor_after: Position,
        ) {
        }

        fn has_history(&self, _buffer_id: BufferId) -> bool {
            true
        }

        fn remove(&self, _buffer_id: BufferId) {}

        fn buffer_count(&self) -> usize {
            1
        }

        fn get_tree(&self, _buffer_id: BufferId) -> Option<reovim_kernel::api::v1::UndoTree> {
            None
        }

        fn begin_batch(&self, _buffer_id: BufferId, _cursor_before: Position) {}
        fn end_batch(&self, _buffer_id: BufferId, _cursor_after: Position) {}
        fn is_batching(&self, _buffer_id: BufferId) -> bool {
            false
        }

        fn persist(
            &self,
            _buffer_id: BufferId,
            _buffer_path: &str,
            _vfs: &dyn reovim_driver_vfs::VfsDriver,
        ) -> Result<(), reovim_driver_undo::UndoPersistError> {
            Ok(())
        }

        fn load(
            &self,
            _buffer_id: BufferId,
            _buffer_path: &str,
            _vfs: &dyn reovim_driver_vfs::VfsDriver,
        ) -> Result<bool, reovim_driver_undo::UndoPersistError> {
            Ok(false)
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn create_test_context_with_undo() -> KernelContext {
        let services = Arc::new(ServiceRegistry::new());
        let undo_registry = UndoProviderRegistry::new();
        undo_registry.register(UndoKey::Buffer, Arc::new(MockUndoProvider));
        services.register(Arc::new(undo_registry));
        KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(RwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::default()),
            services,
        )
    }

    // =========================================================================
    // Undo/Redo success path tests
    // =========================================================================

    #[test]
    fn test_undo_success_with_provider() {
        let kernel = create_test_context_with_undo();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let args = CommandContext::new();
        let result = UndoCommand.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_redo_success_with_provider() {
        let kernel = create_test_context_with_undo();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let args = CommandContext::new();
        let result = RedoCommand.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_undo_with_count_success() {
        let kernel = create_test_context_with_undo();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set("count", ArgValue::Count(3));
        let result = UndoCommand.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_redo_with_count_success() {
        let kernel = create_test_context_with_undo();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set("count", ArgValue::Count(3));
        let result = RedoCommand.execute(&mut runtime, &args);
        assert!(result.is_success());
    }
}
