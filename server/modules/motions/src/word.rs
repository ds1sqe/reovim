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
    reovim_driver_session::{ChangeTracker, SessionRuntime},
    reovim_kernel::api::v1::{
        CommandId, Cursor, Direction, Motion, MotionEngine, Position, WordBoundary,
    },
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

    // Get cursor from per-client Window (#471)
    let Some(window) = runtime.windows().active() else {
        return CommandResult::error("No active window");
    };
    let old_pos = Position::new(window.cursor.line, window.cursor.column);

    let count = args.count().unwrap_or(1);
    let motion = Motion::Word {
        direction,
        boundary,
        end,
    };

    // Calculate motion using with_buffer_read callback
    let motion_result = runtime.with_buffer_read(buffer_id, |buffer| {
        let cursor = Cursor::new(old_pos);
        MotionEngine::calculate(buffer, &cursor, motion, count)
    });

    let Some(Some(new_pos)) = motion_result else {
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

    // Move cursor via per-client Window (#471)
    if let Some(window) = runtime.windows_mut().active_mut() {
        window.cursor = new_pos.into();
    }

    // Record cursor move via ChangeTracker
    runtime.record_cursor_move(buffer_id);

    // Per #388: motions just return Success. The vim resolver stores motion
    // type info in VimSessionState BEFORE dispatching, then completes the
    // operator in its post-command hook.
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
        reovim_driver_session::{
            ClientId, ExtensionMap, Session, SessionRuntime, Window, WindowLayout,
            api::CommandExecutor,
        },
        reovim_kernel::api::{
            KernelContext, ModeId, ModeStack, ModuleId, ServiceRegistry,
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

    // =========================================================================
    // Test Infrastructure (#471)
    // =========================================================================

    /// Stub command executor for tests.
    struct StubExecutor;

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

    /// Explicit test setup for motion commands.
    ///
    /// All state is explicit - no hidden implicit state in helper functions.
    /// Per-client state is held as separate fields to avoid borrow conflicts (#471).
    struct TestSetup {
        ctx: KernelContext,
        session: Session,
        // Per-client state as separate fields (#471 borrow checker fix)
        mode_stack: ModeStack,
        windows: WindowLayout,
        extensions: ExtensionMap,
        buffer_id: BufferId,
    }

    impl TestSetup {
        /// Create test setup with buffer content.
        fn new(content: &str) -> Self {
            let ctx = KernelContext::new(
                Arc::new(EventBus::new()),
                Arc::new(TestBufferManager::new()),
                Arc::new(MotionEngine),
                Arc::new(TextObjectEngine),
                Arc::new(RwLock::new(RegisterBank::new())),
                Arc::new(RwLock::new(MarkBank::new())),
                Arc::new(OptionRegistry::default()),
                Arc::new(ServiceRegistry::new()),
            );

            let buffer = Buffer::from_string(content);
            let buffer_id = ctx.buffers.register(buffer);

            let home_mode = ModeId::new(ModuleId::new("test"), "normal");
            let session = Session::new(ClientId::new(1), home_mode.clone());

            // Per-client state as separate fields (#471)
            let mode_stack = ModeStack::new(home_mode);
            let mut windows = WindowLayout::empty();
            let extensions = ExtensionMap::new();

            // Create window with buffer
            let mut window = Window::new();
            window.buffer_id = Some(buffer_id);
            windows.add(window);

            Self {
                ctx,
                session,
                mode_stack,
                windows,
                extensions,
                buffer_id,
            }
        }

        /// Set cursor position explicitly.
        fn set_cursor(&mut self, pos: Position) {
            if let Some(window) = self.windows.active_mut() {
                window.cursor = pos.into();
            }
        }

        /// Get current cursor position.
        fn cursor(&self) -> Position {
            self.windows.active().map_or_else(
                || Position::new(0, 0),
                |w| Position::new(w.cursor.line, w.cursor.column),
            )
        }

        /// Create command args with `buffer_id` set.
        fn args(&self) -> CommandContext {
            let mut args = CommandContext::new();
            args.set_buffer_id(self.buffer_id);
            args
        }

        /// Run a command and return the result.
        fn run<C: CommandHandler>(&mut self, cmd: &C, args: &CommandContext) -> CommandResult {
            let executor = StubExecutor;
            let mut runtime = SessionRuntime::new(
                &mut self.session,
                &mut self.mode_stack,
                &mut self.windows,
                &mut self.extensions,
                &self.ctx,
                &executor,
            );
            cmd.execute(&mut runtime, args)
        }
    }

    /// Run command without buffer (for error tests).
    /// Creates a session with an empty window to satisfy `windows()` (#471).
    fn run_command_no_buffer<C: CommandHandler>(cmd: &C) -> CommandResult {
        let ctx = KernelContext::default();
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());

        // Per-client state as separate fields (#471)
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();

        // Add an empty window so windows() doesn't panic
        windows.add(Window::new());

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &ctx,
            &executor,
        );
        cmd.execute(&mut runtime, &CommandContext::new())
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
        let result = run_command_no_buffer(&WordForward);
        assert!(result.is_error());
    }

    #[test]
    fn test_word_forward_invalid_buffer_returns_error() {
        let result = run_command_no_buffer(&WordForward);
        assert!(result.is_error());
    }

    // =========================================================================
    // Word Forward (w) Tests
    // =========================================================================

    #[test]
    fn test_word_forward_basic() {
        let mut setup = TestSetup::new("hello world foo");
        setup.set_cursor(Position::new(0, 0));

        let args = setup.args();
        let result = setup.run(&WordForward, &args);

        assert!(result.is_success());
        assert_eq!(setup.cursor().column, 6); // 'w' of world
    }

    #[test]
    fn test_word_forward_with_count() {
        let mut setup = TestSetup::new("one two three four");
        setup.set_cursor(Position::new(0, 0));

        let mut args = setup.args();
        args.set("count", ArgValue::Count(2));

        let result = setup.run(&WordForward, &args);

        assert!(result.is_success());
        assert_eq!(setup.cursor().column, 8); // 't' of three
    }

    #[test]
    fn test_word_forward_across_lines() {
        let mut setup = TestSetup::new("hello\nworld");
        setup.set_cursor(Position::new(0, 4)); // End of first line

        let args = setup.args();
        let result = setup.run(&WordForward, &args);

        assert!(result.is_success());
        assert_eq!(setup.cursor().line, 1);
        assert_eq!(setup.cursor().column, 0); // Start of 'world'
    }

    // =========================================================================
    // Word Backward (b) Tests
    // =========================================================================

    #[test]
    fn test_word_backward_basic() {
        let mut setup = TestSetup::new("hello world foo");
        setup.set_cursor(Position::new(0, 12)); // At 'foo'

        let args = setup.args();
        let result = setup.run(&WordBackward, &args);

        assert!(result.is_success());
        assert_eq!(setup.cursor().column, 6); // 'w' of world
    }

    // =========================================================================
    // Word End (e) Tests
    // =========================================================================

    #[test]
    fn test_word_end_basic() {
        let mut setup = TestSetup::new("hello world foo");
        setup.set_cursor(Position::new(0, 0));

        let args = setup.args();
        let result = setup.run(&WordEnd, &args);

        assert!(result.is_success());
        assert_eq!(setup.cursor().column, 4); // 'o' of hello
    }

    // =========================================================================
    // BigWord (W/B/E) Tests
    // =========================================================================

    #[test]
    fn test_word_forward_big_skips_punctuation() {
        let mut setup = TestSetup::new("hello-world foo");
        setup.set_cursor(Position::new(0, 0));

        let args = setup.args();
        let result = setup.run(&WordForwardBig, &args);

        assert!(result.is_success());
        assert_eq!(setup.cursor().column, 12); // 'f' of foo (skips hello-world)
    }

    #[test]
    fn test_word_forward_small_stops_at_punctuation() {
        let mut setup = TestSetup::new("hello-world foo");
        setup.set_cursor(Position::new(0, 0));

        let args = setup.args();
        let result = setup.run(&WordForward, &args);

        assert!(result.is_success());
        assert_eq!(setup.cursor().column, 5); // '-' (punctuation is its own word)
    }

    // =========================================================================
    // Word End Backward (ge/gE) Tests
    // =========================================================================

    #[test]
    fn test_word_end_backward_basic() {
        let mut setup = TestSetup::new("hello world foo");
        setup.set_cursor(Position::new(0, 12)); // At 'foo'

        let args = setup.args();
        let result = setup.run(&WordEndBackward, &args);

        assert!(result.is_success());
        assert_eq!(setup.cursor().column, 10); // 'd' of world
    }

    // =========================================================================
    // Edge Cases
    // =========================================================================

    #[test]
    fn test_word_forward_at_buffer_end_is_noop() {
        let mut setup = TestSetup::new("hello");
        setup.set_cursor(Position::new(0, 4)); // At end

        let args = setup.args();
        let result = setup.run(&WordForward, &args);

        assert!(result.is_success());
        assert_eq!(setup.cursor().column, 4); // Stays at end
    }

    #[test]
    fn test_word_backward_at_buffer_start_is_noop() {
        let mut setup = TestSetup::new("hello world");
        setup.set_cursor(Position::new(0, 0)); // At start

        let args = setup.args();
        let result = setup.run(&WordBackward, &args);

        assert!(result.is_success());
        assert_eq!(setup.cursor().column, 0); // Stays at start
    }

    #[test]
    fn test_empty_buffer() {
        let mut setup = TestSetup::new("");
        setup.set_cursor(Position::new(0, 0));

        let args = setup.args();
        let result = setup.run(&WordForward, &args);

        assert!(result.is_success()); // No-op, no crash
    }
}
