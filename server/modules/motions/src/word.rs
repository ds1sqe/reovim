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
#[cfg_attr(coverage_nightly, coverage(off))]
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
            api::{CommandExecutor, CommandHandle},
        },
        reovim_kernel::api::{
            KernelContext, ModeId, ModeStack, ModuleId, ServiceRegistry,
            v1::{
                Buffer, BufferError, BufferId, BufferManager, EventBus, HistoryRing, MarkBank,
                OptionRegistry, Position, RegisterBank, RwLock, TextObjectEngine,
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

        #[cfg_attr(coverage_nightly, coverage(off))]
        fn register(&self, buffer: Buffer) -> BufferId {
            let id = BufferId::new();
            let buffer = Arc::new(RwLock::new(buffer));
            self.buffers.write().insert(id, buffer);
            id
        }

        #[cfg_attr(coverage_nightly, coverage(off))]
        fn unregister(&self, id: BufferId) -> Result<Buffer, BufferError> {
            self.buffers
                .write()
                .remove(&id)
                .map_or(Err(BufferError::NotFound(id)), |arc_buffer| {
                    Arc::try_unwrap(arc_buffer)
                        .map_or_else(|arc| Ok(arc.read().clone()), |rwlock| Ok(rwlock.into_inner()))
                })
        }

        #[cfg_attr(coverage_nightly, coverage(off))]
        fn list(&self) -> Vec<BufferId> {
            self.buffers.read().keys().copied().collect()
        }

        #[cfg_attr(coverage_nightly, coverage(off))]
        fn count(&self) -> usize {
            self.buffers.read().len()
        }
    }

    // =========================================================================
    // Test Infrastructure (#471)
    // =========================================================================

    /// Stub command executor for tests.
    struct StubExecutor;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl CommandExecutor for StubExecutor {
        fn get_handle(&self, _id: &CommandId) -> Option<std::sync::Arc<dyn CommandHandle>> {
            None
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
        compositor: Option<Box<dyn reovim_driver_display::layout::RootCompositor>>,
        registers: RegisterBank,
        clipboard_history: HistoryRing,
        local_marks: MarkBank,
        active_buffer: Option<BufferId>,
        terminal_size: (u16, u16),
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
                Arc::new(RwLock::new(MarkBank::new())),
                Arc::new(OptionRegistry::default()),
                Arc::new(ServiceRegistry::new()),
            );

            let buffer = Buffer::from_string(content);
            let buffer_id = ctx.buffers.register(buffer);

            let home_mode = ModeId::new(ModuleId::new("test"), "normal");
            let session = Session::new(ClientId::new(1), home_mode.clone()); // #491

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
                compositor: None,
                registers: RegisterBank::new(),
                clipboard_history: HistoryRing::new(),
                local_marks: MarkBank::new(),
                active_buffer: None,
                terminal_size: (80, 24),
                buffer_id,
            }
        }

        /// Set cursor position explicitly.
        #[cfg_attr(coverage_nightly, coverage(off))]
        fn set_cursor(&mut self, pos: Position) {
            if let Some(window) = self.windows.active_mut() {
                window.cursor = pos.into();
            }
        }

        /// Get current cursor position.
        #[cfg_attr(coverage_nightly, coverage(off))]
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
            let mut tabs = reovim_driver_session::TabPageSet::new();
            let mut runtime = SessionRuntime::new(
                &mut self.session,
                reovim_driver_session::ClientContext {
                    mode_stack: &mut self.mode_stack,
                    windows: &mut self.windows,
                    extensions: &mut self.extensions,
                    compositor: &mut self.compositor,
                    tabs: &mut tabs,
                    registers: &mut self.registers,
                    clipboard_history: &mut self.clipboard_history,
                    local_marks: &mut self.local_marks,
                    active_buffer: &mut self.active_buffer,
                    terminal_size: &mut self.terminal_size,
                },
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
        let mut session = Session::new(ClientId::new(1), home_mode.clone()); // #491

        // Per-client state as separate fields (#471)
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();

        // Add an empty window so windows() doesn't panic
        windows.add(Window::new());

        let executor = StubExecutor;
        let mut compositor = None;
        let mut tabs = reovim_driver_session::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);
        let mut runtime = SessionRuntime::new(
            &mut session,
            reovim_driver_session::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
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

    // =========================================================================
    // Description Tests
    // =========================================================================

    #[test]
    fn test_word_command_descriptions() {
        assert_eq!(WordForward.description(), "Move to next word");
        assert_eq!(WordBackward.description(), "Move to previous word");
        assert_eq!(WordEnd.description(), "Move to end of word");
        assert_eq!(WordForwardBig.description(), "Move to next WORD");
        assert_eq!(WordBackwardBig.description(), "Move to previous WORD");
        assert_eq!(WordEndBig.description(), "Move to end of WORD");
        assert_eq!(WordEndBackward.description(), "Move to end of previous word");
        assert_eq!(WordEndBackwardBig.description(), "Move to end of previous WORD");
    }

    // =========================================================================
    // Additional Word Backward (b) Tests
    // =========================================================================

    #[test]
    fn test_word_backward_with_count() {
        let mut setup = TestSetup::new("one two three four");
        setup.set_cursor(Position::new(0, 14)); // At 'four'

        let mut args = setup.args();
        args.set("count", ArgValue::Count(2));

        let result = setup.run(&WordBackward, &args);
        assert!(result.is_success());
        assert_eq!(setup.cursor().column, 4); // 't' of two
    }

    #[test]
    fn test_word_backward_across_lines() {
        let mut setup = TestSetup::new("hello\nworld");
        setup.set_cursor(Position::new(1, 0)); // Start of 'world'

        let args = setup.args();
        let result = setup.run(&WordBackward, &args);

        assert!(result.is_success());
        assert_eq!(setup.cursor().line, 0);
        assert_eq!(setup.cursor().column, 0); // 'h' of hello
    }

    // =========================================================================
    // Additional Word End (e) Tests
    // =========================================================================

    #[test]
    fn test_word_end_with_count() {
        let mut setup = TestSetup::new("one two three");
        setup.set_cursor(Position::new(0, 0));

        let mut args = setup.args();
        args.set("count", ArgValue::Count(2));

        let result = setup.run(&WordEnd, &args);
        assert!(result.is_success());
        assert_eq!(setup.cursor().column, 6); // 'o' of two
    }

    #[test]
    fn test_word_end_across_lines() {
        let mut setup = TestSetup::new("hi\nthere");
        setup.set_cursor(Position::new(0, 1)); // At end of 'hi'

        let args = setup.args();
        let result = setup.run(&WordEnd, &args);

        assert!(result.is_success());
        assert_eq!(setup.cursor().line, 1);
        assert_eq!(setup.cursor().column, 4); // 'e' of there
    }

    // =========================================================================
    // Additional BigWord (W/B/E) Tests
    // =========================================================================

    #[test]
    fn test_word_backward_big_skips_punctuation() {
        let mut setup = TestSetup::new("hello-world foo");
        setup.set_cursor(Position::new(0, 12)); // At 'foo'

        let args = setup.args();
        let result = setup.run(&WordBackwardBig, &args);

        assert!(result.is_success());
        assert_eq!(setup.cursor().column, 0); // Start of 'hello-world'
    }

    #[test]
    fn test_word_end_big_basic() {
        let mut setup = TestSetup::new("hello-world foo");
        setup.set_cursor(Position::new(0, 0));

        let args = setup.args();
        let result = setup.run(&WordEndBig, &args);

        assert!(result.is_success());
        assert_eq!(setup.cursor().column, 10); // 'd' of hello-world
    }

    #[test]
    fn test_word_end_backward_big_basic() {
        let mut setup = TestSetup::new("hello-world foo bar");
        setup.set_cursor(Position::new(0, 16)); // At 'bar'

        let args = setup.args();
        let result = setup.run(&WordEndBackwardBig, &args);

        assert!(result.is_success());
        // gE from 'bar' should go to end of previous WORD 'foo'
        assert_eq!(setup.cursor().column, 14); // 'o' of foo
    }

    // =========================================================================
    // Whitespace-Only Tests
    // =========================================================================

    #[test]
    fn test_word_forward_whitespace_only() {
        let mut setup = TestSetup::new("   ");
        setup.set_cursor(Position::new(0, 0));

        let args = setup.args();
        let result = setup.run(&WordForward, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_word_backward_whitespace_only() {
        let mut setup = TestSetup::new("   ");
        setup.set_cursor(Position::new(0, 2));

        let args = setup.args();
        let result = setup.run(&WordBackward, &args);
        assert!(result.is_success());
    }

    // =========================================================================
    // Empty Buffer Tests for All Commands
    // =========================================================================

    #[test]
    fn test_word_backward_empty_buffer() {
        let mut setup = TestSetup::new("");
        let args = setup.args();
        let result = setup.run(&WordBackward, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_word_end_empty_buffer() {
        let mut setup = TestSetup::new("");
        let args = setup.args();
        let result = setup.run(&WordEnd, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_word_forward_big_empty_buffer() {
        let mut setup = TestSetup::new("");
        let args = setup.args();
        let result = setup.run(&WordForwardBig, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_word_backward_big_empty_buffer() {
        let mut setup = TestSetup::new("");
        let args = setup.args();
        let result = setup.run(&WordBackwardBig, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_word_end_big_empty_buffer() {
        let mut setup = TestSetup::new("");
        let args = setup.args();
        let result = setup.run(&WordEndBig, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_word_end_backward_empty_buffer() {
        let mut setup = TestSetup::new("");
        let args = setup.args();
        let result = setup.run(&WordEndBackward, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_word_end_backward_big_empty_buffer() {
        let mut setup = TestSetup::new("");
        let args = setup.args();
        let result = setup.run(&WordEndBackwardBig, &args);
        assert!(result.is_success());
    }

    // =========================================================================
    // Error Tests for All Commands
    // =========================================================================

    #[test]
    fn test_word_backward_no_buffer_returns_error() {
        let result = run_command_no_buffer(&WordBackward);
        assert!(result.is_error());
    }

    #[test]
    fn test_word_end_no_buffer_returns_error() {
        let result = run_command_no_buffer(&WordEnd);
        assert!(result.is_error());
    }

    #[test]
    fn test_word_forward_big_no_buffer_returns_error() {
        let result = run_command_no_buffer(&WordForwardBig);
        assert!(result.is_error());
    }

    #[test]
    fn test_word_backward_big_no_buffer_returns_error() {
        let result = run_command_no_buffer(&WordBackwardBig);
        assert!(result.is_error());
    }

    #[test]
    fn test_word_end_big_no_buffer_returns_error() {
        let result = run_command_no_buffer(&WordEndBig);
        assert!(result.is_error());
    }

    #[test]
    fn test_word_end_backward_no_buffer_returns_error() {
        let result = run_command_no_buffer(&WordEndBackward);
        assert!(result.is_error());
    }

    #[test]
    fn test_word_end_backward_big_no_buffer_returns_error() {
        let result = run_command_no_buffer(&WordEndBackwardBig);
        assert!(result.is_error());
    }

    // =========================================================================
    // Default Trait Tests
    // =========================================================================

    #[test]
    fn test_word_commands_default_trait() {
        let _: WordForward = WordForward;
        let _: WordBackward = WordBackward;
        let _: WordEnd = WordEnd;
        let _: WordForwardBig = WordForwardBig;
        let _: WordBackwardBig = WordBackwardBig;
        let _: WordEndBig = WordEndBig;
        let _: WordEndBackward = WordEndBackward;
        let _: WordEndBackwardBig = WordEndBackwardBig;
    }

    // =========================================================================
    // Module ID Tests (remaining commands)
    // =========================================================================

    #[test]
    fn test_word_backward_big_id() {
        let cmd = WordBackwardBig;
        assert_eq!(cmd.id().module(), &ids::MODULE);
        assert_eq!(cmd.id().name(), "word-backward-big");
    }

    #[test]
    fn test_word_end_big_id() {
        let cmd = WordEndBig;
        assert_eq!(cmd.id().module(), &ids::MODULE);
        assert_eq!(cmd.id().name(), "word-end-big");
    }

    // =========================================================================
    // Test infrastructure coverage helpers
    // =========================================================================

    #[test]
    fn test_buffer_manager_create() {
        let setup = TestSetup::new("hello");
        let bid = setup.ctx.buffers.create();
        assert!(setup.ctx.buffers.get(bid).is_some());
        assert_eq!(setup.ctx.buffers.count(), 2); // original + new
    }

    #[test]
    fn test_stub_executor_returns_none() {
        let executor = StubExecutor;
        let cmd_id = ids::WORD_FORWARD;
        let result = executor.get_handle(&cmd_id);
        assert!(result.is_none());
    }

    // =========================================================================
    // Additional motion execution edge cases
    // =========================================================================

    #[test]
    fn test_word_forward_with_invalid_buffer_id() {
        // Tests the `motion_result.is_none()` -> "Buffer not found" path
        let ctx = KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(RwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::default()),
            Arc::new(ServiceRegistry::new()),
        );
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = reovim_driver_session::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);
        windows.add(Window::new());

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            reovim_driver_session::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &ctx,
            &executor,
        );
        let mut args = CommandContext::new();
        args.set("buffer_id", ArgValue::BufferId(999));
        let result = WordForward.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_word_forward_no_window() {
        let ctx = KernelContext::default();
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty(); // No windows
        let mut extensions = ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = reovim_driver_session::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            reovim_driver_session::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &ctx,
            &executor,
        );
        let mut args = CommandContext::new();
        args.set("buffer_id", ArgValue::BufferId(1));
        let result = WordForward.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_word_backward_with_count_three() {
        let mut setup = TestSetup::new("alpha beta gamma delta");
        setup.set_cursor(Position::new(0, 20));

        let mut args = setup.args();
        args.set("count", ArgValue::Count(3));

        let result = setup.run(&WordBackward, &args);
        assert!(result.is_success());
        assert_eq!(setup.cursor().column, 6);
    }

    #[test]
    fn test_word_end_backward_with_count() {
        let mut setup = TestSetup::new("alpha beta gamma delta");
        setup.set_cursor(Position::new(0, 20));

        let mut args = setup.args();
        args.set("count", ArgValue::Count(2));

        let result = setup.run(&WordEndBackward, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_word_end_backward_big_with_count() {
        let mut setup = TestSetup::new("hello-world foo bar");
        setup.set_cursor(Position::new(0, 16));

        let mut args = setup.args();
        args.set("count", ArgValue::Count(2));

        let result = setup.run(&WordEndBackwardBig, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_word_forward_big_with_count() {
        let mut setup = TestSetup::new("hello-world foo bar baz");
        setup.set_cursor(Position::new(0, 0));

        let mut args = setup.args();
        args.set("count", ArgValue::Count(2));

        let result = setup.run(&WordForwardBig, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_word_backward_big_with_count() {
        let mut setup = TestSetup::new("hello world foo bar");
        setup.set_cursor(Position::new(0, 16));

        let mut args = setup.args();
        args.set("count", ArgValue::Count(2));

        let result = setup.run(&WordBackwardBig, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_word_end_big_with_count() {
        let mut setup = TestSetup::new("hello world foo bar");
        setup.set_cursor(Position::new(0, 0));

        let mut args = setup.args();
        args.set("count", ArgValue::Count(2));

        let result = setup.run(&WordEndBig, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_word_forward_single_char_buffer() {
        let mut setup = TestSetup::new("a");
        setup.set_cursor(Position::new(0, 0));

        let args = setup.args();
        let result = setup.run(&WordForward, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_word_backward_single_char_buffer() {
        let mut setup = TestSetup::new("a");
        setup.set_cursor(Position::new(0, 0));

        let args = setup.args();
        let result = setup.run(&WordBackward, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_word_end_at_last_char() {
        let mut setup = TestSetup::new("hello");
        setup.set_cursor(Position::new(0, 4));

        let args = setup.args();
        let result = setup.run(&WordEnd, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_all_commands_count_is_eight() {
        let cmds = all_commands();
        assert_eq!(cmds.len(), 8);
    }

    #[test]
    fn test_word_forward_multiline_many_words() {
        let mut setup = TestSetup::new("one\ntwo\nthree\nfour");
        setup.set_cursor(Position::new(0, 0));

        let mut args = setup.args();
        args.set("count", ArgValue::Count(3));

        let result = setup.run(&WordForward, &args);
        assert!(result.is_success());
        assert_eq!(setup.cursor().line, 3);
    }

    #[test]
    fn test_word_commands_debug() {
        assert!(!format!("{WordForward:?}").is_empty());
        assert!(!format!("{WordBackward:?}").is_empty());
        assert!(!format!("{WordEnd:?}").is_empty());
        assert!(!format!("{WordForwardBig:?}").is_empty());
        assert!(!format!("{WordBackwardBig:?}").is_empty());
        assert!(!format!("{WordEndBig:?}").is_empty());
        assert!(!format!("{WordEndBackward:?}").is_empty());
        assert!(!format!("{WordEndBackwardBig:?}").is_empty());
    }
}
