//! Find-char motion commands.
//!
//! Implements vim find-char motions: `f`, `F`, `t`, `T`, `;`, `,`.
//!
//! # Architecture (#563, Epic #385)
//!
//! ## Coordinator + Execution Split
//!
//! `DISPATCH_FIND_CHAR` (coordinator) records `FindCharState` for repeat,
//! then delegates to `EXECUTE_FIND_CHAR` (execution). Providers override
//! `EXECUTE_FIND_CHAR` only — they never touch repeat state.
//!
//! ## Stub Commands (f/F/t/T)
//!
//! These are **intercepted by the vim resolver** before execution.
//! They exist only for keybinding registration and command metadata.
//!
//! ## Repeat Handlers (;/,)
//!
//! `RepeatFindSame` and `RepeatFindReverse` read `FindCharState` and
//! delegate to `EXECUTE_FIND_CHAR` with stored parameters.

use {
    reovim_driver_command::{ArgValue, Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{
            FindCharState, SessionRuntime,
        api::{CommandApi, ExtensionApi},
    },
    reovim_kernel::api::v1::{CommandId, ModuleId},
};

use crate::ids;

/// The execution command ID for find-char (defined by vim module).
/// Constructed locally to avoid compile-time dependency on vim module.
const EXECUTE_FIND_CHAR: CommandId = CommandId::new(ModuleId::new("vim"), "execute-find-char");

// =============================================================================
// Dispatch Find Char (coordinator) (#563)
// =============================================================================

/// Coordinator command for find-char motions.
///
/// Records `FindCharState` for `;`/`,` repeat, then delegates to
/// `EXECUTE_FIND_CHAR`. This ensures repeat state is always recorded
/// regardless of which provider handles execution.
///
/// WARNING: Do NOT override this command. Overriding would break repeat
/// recording for all providers. Override `EXECUTE_FIND_CHAR` instead.
#[derive(Debug, Clone, Copy, Default)]
pub struct DispatchFindChar;

impl Command for DispatchFindChar {
    fn id(&self) -> CommandId {
        ids::DISPATCH_FIND_CHAR
    }

    fn description(&self) -> &'static str {
        "Record find-char state and delegate to execute-find-char"
    }
}

impl CommandHandler for DispatchFindChar {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(target_char) = args.char("find_char") else {
            return CommandResult::error("find_char argument required");
        };
        let forward = args.string("find_direction") != Some("backward");
        let inclusive = match args.get("find_inclusive") {
            Some(ArgValue::Bang(b)) => *b,
            _ => true,
        };

        // Record state for ;/, repeat
        runtime
            .ext_mut::<FindCharState>()
            .record(target_char, forward, inclusive);

        // Delegate to EXECUTE_FIND_CHAR (same context)
        runtime.execute_command(EXECUTE_FIND_CHAR, args.clone())
    }
}

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
/// in the same direction. Reads `FindCharState` and delegates to
/// `EXECUTE_FIND_CHAR` with stored parameters.
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(record) = runtime.ext_mut::<FindCharState>().last().copied() else {
            return CommandResult::error("No previous find-char");
        };

        let mut ctx = args.clone();
        ctx.set("find_char", ArgValue::Char(record.char()));
        ctx.set(
            "find_direction",
            ArgValue::String(
                if record.forward() {
                    "forward"
                } else {
                    "backward"
                }
                .to_string(),
            ),
        );
        ctx.set("find_inclusive", ArgValue::Bang(record.inclusive()));

        runtime.execute_command(EXECUTE_FIND_CHAR, ctx)
    }
}

// =============================================================================
// Repeat Find Reverse (,)
// =============================================================================

/// Repeat last find-char in the opposite direction.
///
/// After using `f`, `F`, `t`, or `T`, press `,` to repeat that motion
/// in the opposite direction. Reads `FindCharState`, reverses direction,
/// and delegates to `EXECUTE_FIND_CHAR`.
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(record) = runtime.ext_mut::<FindCharState>().last().copied() else {
            return CommandResult::error("No previous find-char");
        };
        let reversed = record.reversed();

        let mut ctx = args.clone();
        ctx.set("find_char", ArgValue::Char(reversed.char()));
        ctx.set(
            "find_direction",
            ArgValue::String(
                if reversed.forward() {
                    "forward"
                } else {
                    "backward"
                }
                .to_string(),
            ),
        );
        ctx.set("find_inclusive", ArgValue::Bang(reversed.inclusive()));

        runtime.execute_command(EXECUTE_FIND_CHAR, ctx)
    }
}

// =============================================================================
// Public API
// =============================================================================

/// Get all find-char motion commands.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(DispatchFindChar),
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
        reovim_kernel::testing::{create_test_context, setup_buffer},
        super::*,
        crate::ids,
        reovim_driver_session::{
            testing::StubExecutor,
            ClientId, ExtensionMap, FindCharState, Session, Window, WindowLayout,
            api::{CommandExecutor, CommandHandle, ExtensionApi},
        },
        reovim_kernel::api::{
            KernelContext, ModeStack,
            v1::{
                BufferId, HistoryRing, MarkBank,
                ModeId, ModuleId, RegisterBank,
            },
        },
        std::sync::Arc,
    };

    // =========================================================================
    // Test Infrastructure
    // =========================================================================

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
    // RecordingExecutor (test infrastructure for coordinator/repeat tests)
    // =========================================================================

    use std::sync::Mutex;

    /// Captures command ID and context for delegation assertions.
    struct RecordingExecutor {
        calls: Arc<Mutex<Vec<(CommandId, CommandContext)>>>,
    }

    impl RecordingExecutor {
        fn new() -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn take_calls(&self) -> Vec<(CommandId, CommandContext)> {
            self.calls.lock().unwrap().drain(..).collect()
        }
    }

    struct RecordingHandle {
        calls: Arc<Mutex<Vec<(CommandId, CommandContext)>>>,
        id: CommandId,
    }

    impl CommandHandle for RecordingHandle {
        fn execute(
            &self,
            _runtime: &mut SessionRuntime<'_>,
            ctx: &CommandContext,
        ) -> CommandResult {
            self.calls
                .lock()
                .unwrap()
                .push((self.id.clone(), ctx.clone()));
            CommandResult::Success
        }
    }

    impl CommandExecutor for RecordingExecutor {
        fn get_handle(&self, id: &CommandId) -> Option<std::sync::Arc<dyn CommandHandle>> {
            let calls = Arc::clone(&self.calls);
            let id = id.clone();
            Some(std::sync::Arc::new(RecordingHandle { calls, id }))
        }
    }

    // =========================================================================
    // Command ID Tests
    // =========================================================================

    #[test]
    fn test_dispatch_find_char_id() {
        let cmd = DispatchFindChar;
        assert_eq!(cmd.id().module(), &ids::MODULE);
        assert_eq!(cmd.id().name(), "dispatch-find-char");
        assert_eq!(cmd.description(), "Record find-char state and delegate to execute-find-char");
    }

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
        assert_eq!(cmds.len(), 7); // dispatch, f, F, t, T, ;, ,
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
    fn test_repeat_find_same_no_prior_find_returns_error() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "hello world");
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let args = CommandContext::new();

        let result = RepeatFindSame.execute(&mut runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    }

    #[test]
    fn test_repeat_find_reverse_no_prior_find_returns_error() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "hello world");
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let args = CommandContext::new();

        let result = RepeatFindReverse.execute(&mut runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
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
    fn test_stub_executor_returns_none() {
        let executor = StubExecutor;
        let cmd_id = ids::FIND_CHAR_FORWARD;
        let result = executor.get_handle(&cmd_id);
        assert!(result.is_none());
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

    // =========================================================================
    // DispatchFindChar trait coverage
    // =========================================================================

    #[test]
    fn test_dispatch_find_char_clone() {
        let cmd = DispatchFindChar;
        let cloned = cmd;
        assert_eq!(cloned.id(), cmd.id());
    }

    #[test]
    fn test_dispatch_find_char_debug() {
        assert!(!format!("{DispatchFindChar:?}").is_empty());
    }

    #[test]
    fn test_dispatch_find_char_default() {
        let _: DispatchFindChar = DispatchFindChar;
    }

    // =========================================================================
    // DispatchFindChar execution tests (with RecordingExecutor)
    // =========================================================================

    #[test]
    fn test_dispatch_find_char_records_state_and_delegates() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "hello world");
        let mut state = TestState::with_window(buffer_id);
        let executor = RecordingExecutor::new();
        let mut runtime = state.runtime(&kernel, &executor);

        let mut args = CommandContext::new();
        args.set("find_char", ArgValue::Char('o'));
        args.set("find_direction", ArgValue::String("forward".to_string()));
        args.set("find_inclusive", ArgValue::Bang(true));

        let result = DispatchFindChar.execute(&mut runtime, &args);
        assert!(result.is_success());

        // Verify state was recorded
        let find_state = runtime.ext_mut::<FindCharState>();
        let record = find_state.last().unwrap();
        assert_eq!(record.char(), 'o');
        assert!(record.forward());
        assert!(record.inclusive());

        // Verify delegation to EXECUTE_FIND_CHAR
        let calls = executor.take_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0.name(), "execute-find-char");
        assert_eq!(calls[0].0.module().as_str(), "vim");
    }

    #[test]
    fn test_dispatch_find_char_backward_till() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "hello world");
        let mut state = TestState::with_window(buffer_id);
        let executor = RecordingExecutor::new();
        let mut runtime = state.runtime(&kernel, &executor);

        let mut args = CommandContext::new();
        args.set("find_char", ArgValue::Char('l'));
        args.set("find_direction", ArgValue::String("backward".to_string()));
        args.set("find_inclusive", ArgValue::Bang(false));

        let result = DispatchFindChar.execute(&mut runtime, &args);
        assert!(result.is_success());

        let find_state = runtime.ext_mut::<FindCharState>();
        let record = find_state.last().unwrap();
        assert_eq!(record.char(), 'l');
        assert!(!record.forward());
        assert!(!record.inclusive());
    }

    #[test]
    fn test_dispatch_find_char_missing_arg() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "hello world");
        let mut state = TestState::with_window(buffer_id);
        let executor = RecordingExecutor::new();
        let mut runtime = state.runtime(&kernel, &executor);

        let args = CommandContext::new();
        let result = DispatchFindChar.execute(&mut runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));

        // No delegation should have happened
        let calls = executor.take_calls();
        assert!(calls.is_empty());
    }

    #[test]
    fn test_dispatch_find_char_defaults() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "hello world");
        let mut state = TestState::with_window(buffer_id);
        let executor = RecordingExecutor::new();
        let mut runtime = state.runtime(&kernel, &executor);

        // Only set find_char, let direction and inclusive use defaults
        let mut args = CommandContext::new();
        args.set("find_char", ArgValue::Char('x'));

        let result = DispatchFindChar.execute(&mut runtime, &args);
        assert!(result.is_success());

        let find_state = runtime.ext_mut::<FindCharState>();
        let record = find_state.last().unwrap();
        assert_eq!(record.char(), 'x');
        assert!(record.forward()); // default: forward
        assert!(record.inclusive()); // default: inclusive
    }

    // =========================================================================
    // RepeatFindSame execution tests (with RecordingExecutor + prior state)
    // =========================================================================

    #[test]
    fn test_repeat_find_same_delegates_with_stored_params() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "hello world");
        let mut state = TestState::with_window(buffer_id);
        let executor = RecordingExecutor::new();
        let mut runtime = state.runtime(&kernel, &executor);

        // Pre-populate FindCharState
        runtime.ext_mut::<FindCharState>().record('o', true, true);

        let args = CommandContext::new();
        let result = RepeatFindSame.execute(&mut runtime, &args);
        assert!(result.is_success());

        let calls = executor.take_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0.name(), "execute-find-char");
        assert_eq!(calls[0].1.char("find_char"), Some('o'));
        assert_eq!(calls[0].1.string("find_direction"), Some("forward"));
        assert!(matches!(calls[0].1.get("find_inclusive"), Some(ArgValue::Bang(true))));
    }

    #[test]
    fn test_repeat_find_same_backward_till() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "hello world");
        let mut state = TestState::with_window(buffer_id);
        let executor = RecordingExecutor::new();
        let mut runtime = state.runtime(&kernel, &executor);

        runtime.ext_mut::<FindCharState>().record('l', false, false);

        let args = CommandContext::new();
        let result = RepeatFindSame.execute(&mut runtime, &args);
        assert!(result.is_success());

        let calls = executor.take_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1.char("find_char"), Some('l'));
        assert_eq!(calls[0].1.string("find_direction"), Some("backward"));
        assert!(matches!(calls[0].1.get("find_inclusive"), Some(ArgValue::Bang(false))));
    }

    // =========================================================================
    // RepeatFindReverse execution tests (with RecordingExecutor + prior state)
    // =========================================================================

    #[test]
    fn test_repeat_find_reverse_delegates_with_reversed_params() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "hello world");
        let mut state = TestState::with_window(buffer_id);
        let executor = RecordingExecutor::new();
        let mut runtime = state.runtime(&kernel, &executor);

        // Pre-populate: forward find 'o'
        runtime.ext_mut::<FindCharState>().record('o', true, true);

        let args = CommandContext::new();
        let result = RepeatFindReverse.execute(&mut runtime, &args);
        assert!(result.is_success());

        let calls = executor.take_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0.name(), "execute-find-char");
        assert_eq!(calls[0].1.char("find_char"), Some('o'));
        // Direction reversed: forward → backward
        assert_eq!(calls[0].1.string("find_direction"), Some("backward"));
        // Inclusive preserved
        assert!(matches!(calls[0].1.get("find_inclusive"), Some(ArgValue::Bang(true))));
    }

    #[test]
    fn test_repeat_find_reverse_from_backward() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "hello world");
        let mut state = TestState::with_window(buffer_id);
        let executor = RecordingExecutor::new();
        let mut runtime = state.runtime(&kernel, &executor);

        // Pre-populate: backward till 'l'
        runtime.ext_mut::<FindCharState>().record('l', false, false);

        let args = CommandContext::new();
        let result = RepeatFindReverse.execute(&mut runtime, &args);
        assert!(result.is_success());

        let calls = executor.take_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1.char("find_char"), Some('l'));
        // Direction reversed: backward → forward
        assert_eq!(calls[0].1.string("find_direction"), Some("forward"));
        // Inclusive preserved (false stays false)
        assert!(matches!(calls[0].1.get("find_inclusive"), Some(ArgValue::Bang(false))));
    }

    // =========================================================================
    // RecordingExecutor infrastructure test
    // =========================================================================

    #[test]
    fn test_recording_executor_returns_handle() {
        let executor = RecordingExecutor::new();
        let cmd_id = ids::DISPATCH_FIND_CHAR;
        let handle = executor.get_handle(&cmd_id);
        assert!(handle.is_some());
    }

    #[test]
    fn test_recording_executor_take_calls_empty() {
        let executor = RecordingExecutor::new();
        let calls = executor.take_calls();
        assert!(calls.is_empty());
    }
}
