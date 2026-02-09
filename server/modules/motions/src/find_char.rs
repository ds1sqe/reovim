//! Find-char motion commands.
//!
//! Implements vim find-char motions: `f`, `F`, `t`, `T`, `;`, `,`.
//!
//! # Architecture (Epic #385)
//!
//! These commands are **intercepted by the vim resolver** before execution.
//! The resolver handles `pending_char` state via `VimSessionState`. These command
//! definitions exist only for:
//! 1. Keybinding registration (command IDs)
//! 2. Command metadata (description, args)
//!
//! The actual find-char logic lives in:
//! - `VimNormalResolver::classify_find_char_command()` - intercepts these commands
//! - `vim::commands::ExecuteFindChar` - executes the motion with char from context

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::SessionRuntime,
    reovim_kernel::api::v1::CommandId,
};

use crate::ids;

// =============================================================================
// Find Char Forward (f)
// =============================================================================

/// Find character forward - cursor lands on the character.
///
/// Press `f` followed by a character to move cursor to next occurrence
/// of that character on the current line.
///
/// Note: This command is intercepted by the vim resolver. The `execute()`
/// method should never be called in normal operation.
#[derive(Debug, Clone, Copy, Default)]
pub struct FindCharForward;

impl Command for FindCharForward {
    fn id(&self) -> CommandId {
        ids::FIND_CHAR_FORWARD
    }

    fn description(&self) -> &'static str {
        "Find character forward (f)"
    }
}

impl CommandHandler for FindCharForward {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Intercepted by vim resolver - should not reach here
        CommandResult::Success
    }
}

// =============================================================================
// Find Char Backward (F)
// =============================================================================

/// Find character backward - cursor lands on the character.
///
/// Press `F` followed by a character to move cursor to previous occurrence
/// of that character on the current line.
#[derive(Debug, Clone, Copy, Default)]
pub struct FindCharBackward;

impl Command for FindCharBackward {
    fn id(&self) -> CommandId {
        ids::FIND_CHAR_BACKWARD
    }

    fn description(&self) -> &'static str {
        "Find character backward (F)"
    }
}

impl CommandHandler for FindCharBackward {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Intercepted by vim resolver - should not reach here
        CommandResult::Success
    }
}

// =============================================================================
// Till Char Forward (t)
// =============================================================================

/// Till character forward - cursor stops before the character.
///
/// Press `t` followed by a character to move cursor to position just
/// before the next occurrence of that character on the current line.
#[derive(Debug, Clone, Copy, Default)]
pub struct TillCharForward;

impl Command for TillCharForward {
    fn id(&self) -> CommandId {
        ids::TILL_CHAR_FORWARD
    }

    fn description(&self) -> &'static str {
        "Till character forward (t)"
    }
}

impl CommandHandler for TillCharForward {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Intercepted by vim resolver - should not reach here
        CommandResult::Success
    }
}

// =============================================================================
// Till Char Backward (T)
// =============================================================================

/// Till character backward - cursor stops after the character.
///
/// Press `T` followed by a character to move cursor to position just
/// after the previous occurrence of that character on the current line.
#[derive(Debug, Clone, Copy, Default)]
pub struct TillCharBackward;

impl Command for TillCharBackward {
    fn id(&self) -> CommandId {
        ids::TILL_CHAR_BACKWARD
    }

    fn description(&self) -> &'static str {
        "Till character backward (T)"
    }
}

impl CommandHandler for TillCharBackward {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Intercepted by vim resolver - should not reach here
        CommandResult::Success
    }
}

// =============================================================================
// Repeat Find Same (;)
// =============================================================================

/// Repeat last find-char in the same direction.
///
/// After using `f`, `F`, `t`, or `T`, press `;` to repeat that motion
/// in the same direction.
///
/// Note: Repeat logic is handled by vim resolver via `VimSessionState.last_find`.
#[derive(Debug, Clone, Copy, Default)]
pub struct RepeatFindSame;

impl Command for RepeatFindSame {
    fn id(&self) -> CommandId {
        ids::REPEAT_FIND_SAME
    }

    fn description(&self) -> &'static str {
        "Repeat last find in same direction (;)"
    }
}

impl CommandHandler for RepeatFindSame {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via VimSessionState.last_find (escape hatch until API supports this)
        CommandResult::Success
    }
}

// =============================================================================
// Repeat Find Reverse (,)
// =============================================================================

/// Repeat last find-char in the opposite direction.
///
/// After using `f`, `F`, `t`, or `T`, press `,` to repeat that motion
/// in the opposite direction.
///
/// Note: Repeat logic is handled by vim resolver via `VimSessionState.last_find`.
#[derive(Debug, Clone, Copy, Default)]
pub struct RepeatFindReverse;

impl Command for RepeatFindReverse {
    fn id(&self) -> CommandId {
        ids::REPEAT_FIND_REVERSE
    }

    fn description(&self) -> &'static str {
        "Repeat last find in opposite direction (,)"
    }
}

impl CommandHandler for RepeatFindReverse {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via VimSessionState.last_find (escape hatch until API supports this)
        CommandResult::Success
    }
}

// =============================================================================
// Public API
// =============================================================================

/// Get all find-char motion commands.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(FindCharForward),
        Box::new(FindCharBackward),
        Box::new(TillCharForward),
        Box::new(TillCharBackward),
        Box::new(RepeatFindSame),
        Box::new(RepeatFindReverse),
    ]
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::ids,
        reovim_driver_session::{
            ClientId, ExtensionMap, Session, Window, WindowLayout, api::CommandExecutor,
        },
        reovim_kernel::api::{
            KernelContext, ModeStack, ServiceRegistry,
            v1::{
                Buffer, BufferError, BufferId, BufferManager, EventBus, MarkBank, ModeId, ModuleId,
                MotionEngine, OptionRegistry, RegisterBank, RwLock, TextObjectEngine,
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
            _: &CommandId,
            _: &CommandContext,
            _: &KernelContext,
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
        fn with_window(buffer_id: BufferId) -> Self {
            let home_mode = ModeId::new(ModuleId::new("test"), "normal");
            let session = Session::new(ClientId::new(1), home_mode.clone());
            let mode_stack = ModeStack::new(home_mode);
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
    fn test_find_char_forward_id() {
        let cmd = FindCharForward;
        assert_eq!(cmd.id().module(), &ids::MODULE);
        assert_eq!(cmd.id().name(), "find-char-forward");
        assert_eq!(cmd.description(), "Find character forward (f)");
    }

    #[test]
    fn test_find_char_backward_id() {
        let cmd = FindCharBackward;
        assert_eq!(cmd.id().module(), &ids::MODULE);
        assert_eq!(cmd.id().name(), "find-char-backward");
        assert_eq!(cmd.description(), "Find character backward (F)");
    }

    #[test]
    fn test_till_char_forward_id() {
        let cmd = TillCharForward;
        assert_eq!(cmd.id().module(), &ids::MODULE);
        assert_eq!(cmd.id().name(), "till-char-forward");
        assert_eq!(cmd.description(), "Till character forward (t)");
    }

    #[test]
    fn test_till_char_backward_id() {
        let cmd = TillCharBackward;
        assert_eq!(cmd.id().module(), &ids::MODULE);
        assert_eq!(cmd.id().name(), "till-char-backward");
        assert_eq!(cmd.description(), "Till character backward (T)");
    }

    #[test]
    fn test_repeat_find_same_id() {
        let cmd = RepeatFindSame;
        assert_eq!(cmd.id().module(), &ids::MODULE);
        assert_eq!(cmd.id().name(), "repeat-find-same");
        assert_eq!(cmd.description(), "Repeat last find in same direction (;)");
    }

    #[test]
    fn test_repeat_find_reverse_id() {
        let cmd = RepeatFindReverse;
        assert_eq!(cmd.id().module(), &ids::MODULE);
        assert_eq!(cmd.id().name(), "repeat-find-reverse");
        assert_eq!(cmd.description(), "Repeat last find in opposite direction (,)");
    }

    #[test]
    fn test_all_commands_count() {
        let cmds = all_commands();
        assert_eq!(cmds.len(), 6); // f, F, t, T, ;, ,
    }

    // =========================================================================
    // Command Args Tests (find-char commands have no args)
    // =========================================================================

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_find_char_commands_have_no_args() {
        for cmd in all_commands() {
            let args = cmd.args();
            assert!(
                args.is_empty(),
                "Command {} should have no args (intercepted by vim resolver)",
                cmd.id()
            );
        }
    }

    // =========================================================================
    // Execution Tests (all intercepted, should return Success)
    // =========================================================================

    #[test]
    fn test_find_char_forward_execute_returns_success() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "hello world");
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let args = CommandContext::new();

        let result = FindCharForward.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_find_char_backward_execute_returns_success() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "hello world");
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let args = CommandContext::new();

        let result = FindCharBackward.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_till_char_forward_execute_returns_success() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "hello world");
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let args = CommandContext::new();

        let result = TillCharForward.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_till_char_backward_execute_returns_success() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "hello world");
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let args = CommandContext::new();

        let result = TillCharBackward.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_repeat_find_same_execute_returns_success() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "hello world");
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let args = CommandContext::new();

        let result = RepeatFindSame.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_repeat_find_reverse_execute_returns_success() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "hello world");
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let args = CommandContext::new();

        let result = RepeatFindReverse.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    // =========================================================================
    // Default Trait Tests
    // =========================================================================

    #[test]
    fn test_find_char_default_trait() {
        let _: FindCharForward = FindCharForward;
        let _: FindCharBackward = FindCharBackward;
        let _: TillCharForward = TillCharForward;
        let _: TillCharBackward = TillCharBackward;
        let _: RepeatFindSame = RepeatFindSame;
        let _: RepeatFindReverse = RepeatFindReverse;
    }

    // =========================================================================
    // Additional coverage tests for test infrastructure
    // =========================================================================

    #[test]
    fn test_buffer_manager_list_and_count() {
        let ctx = create_test_context();
        assert_eq!(ctx.buffers.count(), 0);
        assert!(ctx.buffers.list().is_empty());

        let bid = setup_buffer(&ctx, "hello");
        assert_eq!(ctx.buffers.count(), 1);
        assert!(ctx.buffers.list().contains(&bid));
    }

    #[test]
    fn test_buffer_manager_unregister() {
        let ctx = create_test_context();
        let bid = setup_buffer(&ctx, "hello");
        let result = ctx.buffers.unregister(bid);
        assert!(result.is_ok());
        assert_eq!(ctx.buffers.count(), 0);
    }

    #[test]
    fn test_buffer_manager_unregister_nonexistent() {
        let ctx = create_test_context();
        let bid = BufferId::from_raw(999);
        let result = ctx.buffers.unregister(bid);
        assert!(result.is_err());
    }

    #[test]
    fn test_buffer_manager_create() {
        let ctx = create_test_context();
        let bid = ctx.buffers.create();
        assert!(ctx.buffers.get(bid).is_some());
    }

    #[test]
    fn test_stub_executor_returns_success() {
        let executor = StubExecutor;
        let ctx = create_test_context();
        let cmd_id = ids::FIND_CHAR_FORWARD;
        let args = CommandContext::new();
        let result = executor.execute(&cmd_id, &args, &ctx);
        assert!(result.is_some());
        assert!(result.unwrap().is_success());
    }

    #[test]
    fn test_test_state_runtime_method() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "test content");
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        // Verify runtime can be created without panicking
        let _runtime = state.runtime(&kernel, &executor);
    }

    // =========================================================================
    // Clone/Copy/Debug trait tests
    // =========================================================================

    #[test]
    fn test_find_char_forward_clone() {
        let cmd = FindCharForward;
        let cloned = cmd;
        assert_eq!(cloned.id(), cmd.id());
    }

    #[test]
    fn test_find_char_backward_clone() {
        let cmd = FindCharBackward;
        let cloned = cmd;
        assert_eq!(cloned.id(), cmd.id());
    }

    #[test]
    fn test_till_char_forward_clone() {
        let cmd = TillCharForward;
        let cloned = cmd;
        assert_eq!(cloned.id(), cmd.id());
    }

    #[test]
    fn test_till_char_backward_clone() {
        let cmd = TillCharBackward;
        let cloned = cmd;
        assert_eq!(cloned.id(), cmd.id());
    }

    #[test]
    fn test_repeat_find_same_clone() {
        let cmd = RepeatFindSame;
        let cloned = cmd;
        assert_eq!(cloned.id(), cmd.id());
    }

    #[test]
    fn test_repeat_find_reverse_clone() {
        let cmd = RepeatFindReverse;
        let cloned = cmd;
        assert_eq!(cloned.id(), cmd.id());
    }

    #[test]
    fn test_find_char_debug_formats() {
        assert!(!format!("{FindCharForward:?}").is_empty());
        assert!(!format!("{FindCharBackward:?}").is_empty());
        assert!(!format!("{TillCharForward:?}").is_empty());
        assert!(!format!("{TillCharBackward:?}").is_empty());
        assert!(!format!("{RepeatFindSame:?}").is_empty());
        assert!(!format!("{RepeatFindReverse:?}").is_empty());
    }

    // =========================================================================
    // Individual all_commands entry verification
    // =========================================================================

    #[test]
    fn test_all_commands_descriptions() {
        assert_eq!(FindCharForward.description(), "Find character forward (f)");
        assert_eq!(FindCharBackward.description(), "Find character backward (F)");
        assert_eq!(TillCharForward.description(), "Till character forward (t)");
        assert_eq!(TillCharBackward.description(), "Till character backward (T)");
        assert_eq!(RepeatFindSame.description(), "Repeat last find in same direction (;)");
        assert_eq!(RepeatFindReverse.description(), "Repeat last find in opposite direction (,)");
    }

    #[test]
    fn test_all_commands_ids_unique() {
        let cmds = all_commands();
        let ids: Vec<_> = cmds.iter().map(|c| c.id()).collect();
        for (i, id) in ids.iter().enumerate() {
            for (j, other_id) in ids.iter().enumerate() {
                if i != j {
                    assert_ne!(id, other_id, "Command IDs must be unique");
                }
            }
        }
    }
}
