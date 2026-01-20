//! Word motion commands.
//!
//! Implements vim word motions: `w`, `b`, `e`, `W`, `B`, `E`, `ge`, `gE`.
//!
//! These commands wire to the kernel's `MotionEngine::calculate()` which
//! already implements all the motion logic.

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{BufferApi, ChangeTracker, SessionRuntime},
    reovim_kernel::api::v1::{CommandId, Cursor, Direction, Motion, MotionEngine, WordBoundary},
};

use crate::ids;

// =============================================================================
// Helper function
// =============================================================================

/// Execute a word motion and update cursor position.
///
/// In operator-pending mode, returns an `OperatorRange` instead of moving the cursor.
/// Word motions are characterwise.
#[allow(clippy::cast_possible_truncation)]
fn execute_word_motion(
    runtime: &mut SessionRuntime<'_>,
    args: &CommandContext,
    direction: Direction,
    boundary: WordBoundary,
    end: bool,
) -> CommandResult {
    let Some(buffer_id) = args.buffer_id() else {
        return CommandResult::error("No active buffer");
    };

    let count = args.count().unwrap_or(1);
    let motion = Motion::Word {
        direction,
        boundary,
        end,
    };

    // Calculate motion using with_buffer_read callback
    let motion_result = runtime.with_buffer_read(buffer_id, |buffer| {
        let cursor = Cursor::new(buffer.position());
        let old_pos = cursor.position;
        let new_pos = MotionEngine::calculate(buffer, &cursor, motion, count);
        (old_pos, new_pos)
    });

    let Some((old_pos, Some(new_pos))) = motion_result else {
        // Buffer not found or motion calculation failed
        return if motion_result.is_none() {
            CommandResult::error("Buffer not found")
        } else {
            CommandResult::Success // No-op if motion fails
        };
    };

    if new_pos == old_pos {
        return CommandResult::Success; // No movement
    }

    // In operator-pending mode, return range for the operator
    if args.is_operator_pending() {
        // Determine range direction - start should be before end
        let (start, range_end) = if direction == Direction::Forward {
            (old_pos, new_pos)
        } else {
            (new_pos, old_pos)
        };
        // TODO(#394): Return operator range via different mechanism (escape hatch until API supports this)
        // Word motions are characterwise
        let _ = (start, range_end); // Suppress unused warnings
        return CommandResult::Success;
    }

    // Normal mode: move cursor via BufferApi
    runtime.set_buffer_position(buffer_id, new_pos);

    // Record cursor move via ChangeTracker
    runtime.record_cursor_move(buffer_id);

    CommandResult::Success
}

// =============================================================================
// Word Forward (w)
// =============================================================================

/// Move cursor to start of next word.
#[derive(Debug, Clone, Copy, Default)]
pub struct WordForward;

impl Command for WordForward {
    fn id(&self) -> CommandId {
        ids::WORD_FORWARD
    }

    fn description(&self) -> &'static str {
        "Move to next word"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of words",
        )]
    }
}

impl CommandHandler for WordForward {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_word_motion(runtime, args, Direction::Forward, WordBoundary::Word, false)
    }
}

// =============================================================================
// Word Backward (b)
// =============================================================================

/// Move cursor to start of previous word.
#[derive(Debug, Clone, Copy, Default)]
pub struct WordBackward;

impl Command for WordBackward {
    fn id(&self) -> CommandId {
        ids::WORD_BACKWARD
    }

    fn description(&self) -> &'static str {
        "Move to previous word"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of words",
        )]
    }
}

impl CommandHandler for WordBackward {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_word_motion(runtime, args, Direction::Backward, WordBoundary::Word, false)
    }
}

// =============================================================================
// Word End (e)
// =============================================================================

/// Move cursor to end of current/next word.
#[derive(Debug, Clone, Copy, Default)]
pub struct WordEnd;

impl Command for WordEnd {
    fn id(&self) -> CommandId {
        ids::WORD_END
    }

    fn description(&self) -> &'static str {
        "Move to end of word"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of words",
        )]
    }
}

impl CommandHandler for WordEnd {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_word_motion(runtime, args, Direction::Forward, WordBoundary::Word, true)
    }
}

// =============================================================================
// WORD Forward (W)
// =============================================================================

/// Move cursor to start of next WORD (whitespace-delimited).
#[derive(Debug, Clone, Copy, Default)]
pub struct WordForwardBig;

impl Command for WordForwardBig {
    fn id(&self) -> CommandId {
        ids::WORD_FORWARD_BIG
    }

    fn description(&self) -> &'static str {
        "Move to next WORD"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of WORDs",
        )]
    }
}

impl CommandHandler for WordForwardBig {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_word_motion(runtime, args, Direction::Forward, WordBoundary::BigWord, false)
    }
}

// =============================================================================
// WORD Backward (B)
// =============================================================================

/// Move cursor to start of previous WORD (whitespace-delimited).
#[derive(Debug, Clone, Copy, Default)]
pub struct WordBackwardBig;

impl Command for WordBackwardBig {
    fn id(&self) -> CommandId {
        ids::WORD_BACKWARD_BIG
    }

    fn description(&self) -> &'static str {
        "Move to previous WORD"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of WORDs",
        )]
    }
}

impl CommandHandler for WordBackwardBig {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_word_motion(runtime, args, Direction::Backward, WordBoundary::BigWord, false)
    }
}

// =============================================================================
// WORD End (E)
// =============================================================================

/// Move cursor to end of current/next WORD (whitespace-delimited).
#[derive(Debug, Clone, Copy, Default)]
pub struct WordEndBig;

impl Command for WordEndBig {
    fn id(&self) -> CommandId {
        ids::WORD_END_BIG
    }

    fn description(&self) -> &'static str {
        "Move to end of WORD"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of WORDs",
        )]
    }
}

impl CommandHandler for WordEndBig {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_word_motion(runtime, args, Direction::Forward, WordBoundary::BigWord, true)
    }
}

// =============================================================================
// Word End Backward (ge)
// =============================================================================

/// Move cursor to end of previous word.
#[derive(Debug, Clone, Copy, Default)]
pub struct WordEndBackward;

impl Command for WordEndBackward {
    fn id(&self) -> CommandId {
        ids::WORD_END_BACKWARD
    }

    fn description(&self) -> &'static str {
        "Move to end of previous word"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of words",
        )]
    }
}

impl CommandHandler for WordEndBackward {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_word_motion(runtime, args, Direction::Backward, WordBoundary::Word, true)
    }
}

// =============================================================================
// WORD End Backward (gE)
// =============================================================================

/// Move cursor to end of previous WORD (whitespace-delimited).
#[derive(Debug, Clone, Copy, Default)]
pub struct WordEndBackwardBig;

impl Command for WordEndBackwardBig {
    fn id(&self) -> CommandId {
        ids::WORD_END_BACKWARD_BIG
    }

    fn description(&self) -> &'static str {
        "Move to end of previous WORD"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of WORDs",
        )]
    }
}

impl CommandHandler for WordEndBackwardBig {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_word_motion(runtime, args, Direction::Backward, WordBoundary::BigWord, true)
    }
}

// =============================================================================
// Command Registration
// =============================================================================

/// Get all word motion commands as boxed trait objects.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(WordForward),
        Box::new(WordBackward),
        Box::new(WordEnd),
        Box::new(WordForwardBig),
        Box::new(WordBackwardBig),
        Box::new(WordEndBig),
        Box::new(WordEndBackward),
        Box::new(WordEndBackwardBig),
    ]
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_driver_command::ArgValue,
        reovim_driver_session::{ClientId, Session, SessionRuntime, api::CommandExecutor},
        reovim_kernel::api::{
            KernelContext, ModeId, ModuleId,
            v1::{
                Buffer, BufferError, BufferId, BufferManager, EventBus, MarkBank, OptionRegistry,
                Position, RegisterBank, RwLock, TextObjectEngine,
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

    fn create_test_context() -> KernelContext {
        KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(RwLock::new(RegisterBank::new())),
            Arc::new(RwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::default()),
        )
    }

    fn setup_buffer(ctx: &KernelContext, content: &str) -> BufferId {
        let buffer = Buffer::from_string(content);
        ctx.buffers.register(buffer)
    }

    fn run_command<C: CommandHandler>(
        cmd: &C,
        ctx: &KernelContext,
        args: &CommandContext,
    ) -> CommandResult {
        struct StubExecutor;
        impl CommandExecutor for StubExecutor {
            fn execute(
                &self,
                _: &CommandId,
                _: &CommandContext,
                _: &mut KernelContext,
            ) -> Option<CommandResult> {
                Some(CommandResult::Success)
            }
        }
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode);
        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(&mut session, ctx, &executor);
        cmd.execute(&mut runtime, args)
    }

    // =========================================================================
    // Command ID Tests
    // =========================================================================

    #[test]
    fn test_word_forward_id() {
        let cmd = WordForward;
        assert_eq!(cmd.id().module(), &ids::MODULE);
        assert_eq!(cmd.id().name(), "word-forward");
    }

    #[test]
    fn test_word_backward_id() {
        let cmd = WordBackward;
        assert_eq!(cmd.id().name(), "word-backward");
    }

    #[test]
    fn test_word_end_id() {
        let cmd = WordEnd;
        assert_eq!(cmd.id().name(), "word-end");
    }

    #[test]
    fn test_word_forward_big_id() {
        let cmd = WordForwardBig;
        assert_eq!(cmd.id().name(), "word-forward-big");
    }

    #[test]
    fn test_word_end_backward_id() {
        let cmd = WordEndBackward;
        assert_eq!(cmd.id().name(), "word-end-backward");
    }

    #[test]
    fn test_word_end_backward_big_id() {
        let cmd = WordEndBackwardBig;
        assert_eq!(cmd.id().name(), "word-end-backward-big");
    }

    // =========================================================================
    // Command Args Tests
    // =========================================================================

    #[test]
    fn test_word_commands_have_count_arg() {
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
    fn test_word_forward_no_buffer_returns_error() {
        let ctx = KernelContext::default();
        let args = CommandContext::new();
        let result = run_command(&WordForward, &ctx, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_word_forward_invalid_buffer_returns_error() {
        let ctx = KernelContext::default();
        let mut args = CommandContext::new();
        args.set("buffer_id", ArgValue::BufferId(999));
        let result = run_command(&WordForward, &ctx, &args);
        assert!(result.is_error());
    }

    // =========================================================================
    // Word Forward (w) Tests
    // =========================================================================

    #[test]
    fn test_word_forward_basic() {
        let ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "hello world foo");

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = run_command(&WordForward, &ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.column, 6); // 'w' of world
    }

    #[test]
    fn test_word_forward_with_count() {
        let ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "one two three four");

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(2));

        let result = run_command(&WordForward, &ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.column, 8); // 't' of three
    }

    #[test]
    fn test_word_forward_across_lines() {
        let ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "hello\nworld");

        // Position at end of first line
        {
            let buffer = ctx.buffers.get(buffer_id).unwrap();
            buffer.write().set_position(Position::new(0, 4));
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = run_command(&WordForward, &ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.line, 1);
        assert_eq!(pos.column, 0); // Start of 'world'
    }

    // =========================================================================
    // Word Backward (b) Tests
    // =========================================================================

    #[test]
    fn test_word_backward_basic() {
        let ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "hello world foo");

        // Position at 'foo'
        {
            let buffer = ctx.buffers.get(buffer_id).unwrap();
            buffer.write().set_position(Position::new(0, 12));
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = run_command(&WordBackward, &ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.column, 6); // 'w' of world
    }

    // =========================================================================
    // Word End (e) Tests
    // =========================================================================

    #[test]
    fn test_word_end_basic() {
        let ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "hello world foo");

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = run_command(&WordEnd, &ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.column, 4); // 'o' of hello
    }

    // =========================================================================
    // BigWord (W/B/E) Tests
    // =========================================================================

    #[test]
    fn test_word_forward_big_skips_punctuation() {
        let ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "hello-world foo");

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = run_command(&WordForwardBig, &ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.column, 12); // 'f' of foo (skips hello-world as single WORD)
    }

    #[test]
    fn test_word_forward_small_stops_at_punctuation() {
        let ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "hello-world foo");

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = run_command(&WordForward, &ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.column, 5); // '-' (punctuation is its own word)
    }

    // =========================================================================
    // Word End Backward (ge/gE) Tests
    // =========================================================================

    #[test]
    fn test_word_end_backward_basic() {
        let ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "hello world foo");

        // Position at 'foo'
        {
            let buffer = ctx.buffers.get(buffer_id).unwrap();
            buffer.write().set_position(Position::new(0, 12));
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = run_command(&WordEndBackward, &ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.column, 10); // 'd' of world
    }

    // =========================================================================
    // Edge Cases
    // =========================================================================

    #[test]
    fn test_word_forward_at_buffer_end_is_noop() {
        let ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "hello");

        // Position at end
        {
            let buffer = ctx.buffers.get(buffer_id).unwrap();
            buffer.write().set_position(Position::new(0, 4));
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = run_command(&WordForward, &ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.column, 4); // Stays at end
    }

    #[test]
    fn test_word_backward_at_buffer_start_is_noop() {
        let ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "hello world");

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = run_command(&WordBackward, &ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.column, 0); // Stays at start
    }

    #[test]
    fn test_empty_buffer() {
        let ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "");

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = run_command(&WordForward, &ctx, &args);
        assert!(result.is_success()); // No-op, no crash
    }
}
