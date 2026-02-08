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
        reovim_kernel::api::{
            ServiceRegistry,
            v1::{
                Buffer, BufferError, BufferId, BufferManager, CommandId as KernelCommandId,
                EventBus, KernelContext, MarkBank, ModeId, ModeStack, ModuleId, MotionEngine,
                OptionRegistry, RegisterBank, RwLock, TextObjectEngine,
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

    fn test_mode() -> ModeId {
        ModeId::new(ModuleId::new("test"), "normal")
    }

    struct StubExecutor;

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

    struct TestState {
        session: Session,
        mode_stack: ModeStack,
        windows: WindowLayout,
        extensions: ExtensionMap,
    }

    impl TestState {
        fn with_window(buffer_id: BufferId) -> Self {
            let home_mode = test_mode();
            let mut state = Self {
                session: Session::new(ClientId::new(1), home_mode.clone()),
                mode_stack: ModeStack::new(home_mode),
                windows: WindowLayout::empty(),
                extensions: ExtensionMap::new(),
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
                &mut self.mode_stack,
                &mut self.windows,
                &mut self.extensions,
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
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
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
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
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
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
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
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
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
}
