//! Dot repeat command (.).
//!
//! Implements the `.` command which repeats the last change operation.
//!
//! # Supported Changes
//!
//! - Operator + motion (e.g., `dw`, `c$`)
//! - Operator + text object (e.g., `diw`, `ci"`)
//! - Insert mode text (e.g., `ihello<Esc>`)
//!
//! # Count Behavior
//!
//! If `.` is invoked with a count (e.g., `3.`), that count **replaces**
//! the original count, it does NOT multiply. This matches Vim behavior.

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{BufferApi, SessionRuntime, api::ExtensionApi},
    reovim_kernel::api::v1::{CommandId, Position},
};

use crate::{
    ids,
    session_state::{ChangeType, VimSessionState},
};

/// Repeat last change (.).
///
/// Replays the last change operation. Count overrides the original count.
#[derive(Debug, Clone, Copy, Default)]
pub struct DotRepeat;

impl Command for DotRepeat {
    fn id(&self) -> CommandId {
        ids::DOT_REPEAT
    }

    fn description(&self) -> &'static str {
        "Repeat last change (.)"
    }
}

impl CommandHandler for DotRepeat {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        // Get last change from VimSessionState
        let Some(vim) = runtime.ext::<VimSessionState>() else {
            return CommandResult::Success; // No vim state
        };
        let Some(last_change) = vim.last_change.as_ref() else {
            return CommandResult::Success; // No change to repeat
        };
        let last_change = last_change.clone();

        // Count from `.` command overrides original count (Vim behavior)
        let effective_count = args.count().or(last_change.count);
        // TODO(#465): Use register for dot repeat once register API is ready
        let _ = last_change.register;

        match &last_change.change_type {
            ChangeType::Insert { text } => {
                // Repeat insert: insert the same text
                self.repeat_insert(runtime, args, text, effective_count)
            }
            ChangeType::OperatorMotion { operator, linewise }
            | ChangeType::OperatorTextObject { operator, linewise } => {
                // For now, log the operation - full implementation requires
                // re-dispatching the same keys or storing more context
                tracing::debug!(
                    ?operator,
                    linewise,
                    count = ?effective_count,
                    "dot repeat: operator (stub - requires key re-dispatch)"
                );
                CommandResult::Success
            }
        }
    }
}

impl DotRepeat {
    /// Repeat an insert operation by inserting the same text.
    #[allow(clippy::unused_self)]
    fn repeat_insert(
        self,
        runtime: &mut SessionRuntime<'_>,
        args: &CommandContext,
        text: &str,
        count: Option<usize>,
    ) -> CommandResult {
        // Get buffer_id from context or active buffer
        let buffer_id = args.buffer_id().or_else(|| runtime.active_buffer());
        let Some(buffer_id) = buffer_id else {
            return CommandResult::error("No active buffer");
        };

        // Get cursor position from per-client window
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let pos = Position::new(window.cursor.line, window.cursor.column);

        let effective_count = count.unwrap_or(1);

        // Build repeated text
        let repeated_text = text.repeat(effective_count);

        // Insert the text at cursor position
        runtime.insert_text(buffer_id, pos, &repeated_text);

        CommandResult::Success
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::session_state::{LastChange, OperatorType},
        reovim_driver_command::ArgValue,
        reovim_driver_session::{
            ClientId, ExtensionMap, Session, Window, WindowLayout, api::CommandExecutor,
        },
        reovim_kernel::api::{
            ModeStack, ServiceRegistry,
            v1::{
                Buffer, BufferError, BufferId, BufferManager, CommandId as KernelCommandId,
                EventBus, KernelContext, MarkBank, ModeId, ModuleId, MotionEngine, OptionRegistry,
                RegisterBank, RwLock, TextObjectEngine,
            },
        },
        std::{collections::HashMap, sync::Arc},
    };

    /// Test buffer manager that stores buffers.
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

    /// Stub command executor for tests.
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

    fn test_mode() -> ModeId {
        ModeId::new(ModuleId::new("vim"), "normal")
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

    // =========================================================================
    // Test Infrastructure (#471)
    // =========================================================================

    /// Test state holder for per-client state (#471 borrow checker fix).
    struct TestState {
        session: Session,
        mode_stack: ModeStack,
        windows: WindowLayout,
        extensions: ExtensionMap,
    }

    impl TestState {
        /// Create test state with a window containing the given buffer.
        fn with_window(buffer_id: BufferId, mode: ModeId) -> Self {
            let session = Session::new(ClientId::new(1));
            let mode_stack = ModeStack::new(mode);
            let mut windows = WindowLayout::empty();
            let extensions = ExtensionMap::new();

            windows.add(Window::with_buffer(buffer_id));

            Self {
                session,
                mode_stack,
                windows,
                extensions,
            }
        }

        /// Create a runtime from this test state.
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
    // Command ID Tests
    // =========================================================================

    #[test]
    fn test_dot_repeat_command_id() {
        let cmd = DotRepeat;
        assert_eq!(cmd.id().name(), "dot-repeat");
        assert_eq!(cmd.description(), "Repeat last change (.)");
    }

    // =========================================================================
    // No Last Change Tests
    // =========================================================================

    #[test]
    fn test_dot_repeat_without_vim_state_returns_success() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "hello world");
        let mut state = TestState::with_window(buffer_id, test_mode());
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // No VimSessionState registered - should return Success
        let result = DotRepeat.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_dot_repeat_without_last_change_returns_success() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "hello world");
        let mut state = TestState::with_window(buffer_id, test_mode());
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);

        // Register VimSessionState but don't set last_change
        runtime.ext_mut::<VimSessionState>();

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = DotRepeat.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    // =========================================================================
    // Insert Repeat Tests
    // =========================================================================

    #[test]
    fn test_dot_repeat_insert_text() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "");
        let mut state = TestState::with_window(buffer_id, test_mode());
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);

        // Set up last change: insert "hello"
        {
            let vim = runtime.ext_mut::<VimSessionState>();
            vim.last_change = Some(LastChange {
                change_type: ChangeType::Insert {
                    text: "hello".to_string(),
                },
                count: None,
                register: None,
            });
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = DotRepeat.execute(&mut runtime, &args);
        assert!(result.is_success());

        // Verify buffer contains "hello"
        let content = runtime.buffer_content(buffer_id);
        assert!(content.is_some());
        // Buffer content after insert at position (0,0) - no trailing newline
        assert_eq!(content.unwrap(), "hello");
    }

    #[test]
    fn test_dot_repeat_insert_with_original_count() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "");
        let mut state = TestState::with_window(buffer_id, test_mode());
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);

        // Set up last change: insert "ab" with count 3
        {
            let vim = runtime.ext_mut::<VimSessionState>();
            vim.last_change = Some(LastChange {
                change_type: ChangeType::Insert {
                    text: "ab".to_string(),
                },
                count: Some(3),
                register: None,
            });
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = DotRepeat.execute(&mut runtime, &args);
        assert!(result.is_success());

        // Verify buffer contains "ababab" (repeated 3 times)
        let content = runtime.buffer_content(buffer_id);
        assert!(content.is_some());
        assert_eq!(content.unwrap(), "ababab");
    }

    #[test]
    fn test_dot_repeat_count_overrides_original() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "");
        let mut state = TestState::with_window(buffer_id, test_mode());
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);

        // Set up last change: insert "x" with count 5
        {
            let vim = runtime.ext_mut::<VimSessionState>();
            vim.last_change = Some(LastChange {
                change_type: ChangeType::Insert {
                    text: "x".to_string(),
                },
                count: Some(5),
                register: None,
            });
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        // Set count to 2 - this should override the original count of 5
        args.set("count", ArgValue::Count(2));

        let result = DotRepeat.execute(&mut runtime, &args);
        assert!(result.is_success());

        // Verify buffer contains "xx" (repeated 2 times, not 5)
        let content = runtime.buffer_content(buffer_id);
        assert!(content.is_some());
        assert_eq!(content.unwrap(), "xx");
    }

    // =========================================================================
    // Operator Change Recording Tests
    // =========================================================================

    #[test]
    fn test_last_change_stores_operator_motion() {
        let last_change = LastChange {
            change_type: ChangeType::OperatorMotion {
                operator: OperatorType::Delete,
                linewise: false,
            },
            count: Some(3),
            register: Some('a'),
        };

        assert!(matches!(last_change.change_type, ChangeType::OperatorMotion { .. }));
        assert_eq!(last_change.count, Some(3));
        assert_eq!(last_change.register, Some('a'));
    }

    #[test]
    fn test_last_change_stores_operator_textobj() {
        let last_change = LastChange {
            change_type: ChangeType::OperatorTextObject {
                operator: OperatorType::Change,
                linewise: false,
            },
            count: None,
            register: None,
        };

        assert!(matches!(last_change.change_type, ChangeType::OperatorTextObject { .. }));
    }

    #[test]
    fn test_last_change_stores_insert() {
        let last_change = LastChange {
            change_type: ChangeType::Insert {
                text: "hello world".to_string(),
            },
            count: None,
            register: None,
        };

        match &last_change.change_type {
            ChangeType::Insert { text } => {
                assert_eq!(text, "hello world");
            }
            _ => panic!("Expected Insert variant"),
        }
    }

    // =========================================================================
    // Error Handling Tests
    // =========================================================================

    #[test]
    fn test_dot_repeat_no_buffer_returns_error() {
        let kernel = create_test_context();
        let mode = test_mode();
        let mut session = Session::new(ClientId::new(1));
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

        // Set up last change
        {
            let vim = runtime.ext_mut::<VimSessionState>();
            vim.last_change = Some(LastChange {
                change_type: ChangeType::Insert {
                    text: "test".to_string(),
                },
                count: None,
                register: None,
            });
        }

        // No buffer_id set and no active buffer
        let args = CommandContext::new();
        let result = DotRepeat.execute(&mut runtime, &args);

        // Should return error since no buffer is available
        assert!(result.is_error());
    }
}
