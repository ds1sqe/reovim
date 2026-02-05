//! Line motion commands.
//!
//! Implements vim line motions: `0`, `$`, `^`, `gg`, `G`.
//!
//! These commands wire to the kernel's `MotionEngine::calculate()` which
//! already implements all the motion logic.

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{ChangeTracker, SessionRuntime},
    reovim_kernel::api::v1::{CommandId, Cursor, LinePosition, Motion, MotionEngine, Position},
};

use crate::ids;

// =============================================================================
// Helper functions
// =============================================================================

/// Execute a line position motion and update cursor position.
///
/// In operator-pending mode, returns an `OperatorRange` instead of moving the cursor.
/// Line position motions (`0`, `$`, `^`) are characterwise.
#[allow(clippy::cast_possible_truncation)]
fn execute_line_position(
    runtime: &mut SessionRuntime<'_>,
    args: &CommandContext,
    position: LinePosition,
) -> CommandResult {
    let Some(buffer_id) = args.buffer_id() else {
        return CommandResult::error("No active buffer");
    };

    // Get cursor from per-client Window (#471)
    let Some(window) = runtime.windows().active() else {
        return CommandResult::error("No active window");
    };
    let old_pos = Position::new(window.cursor.line, window.cursor.column);

    // Calculate motion using with_buffer_read callback
    let motion = Motion::LinePosition(position);
    let motion_result = runtime.with_buffer_read(buffer_id, |buffer| {
        let cursor = Cursor::new(old_pos);
        MotionEngine::calculate(buffer, &cursor, motion, 1)
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

/// Execute a jump line motion and update cursor position.
///
/// In operator-pending mode, returns an `OperatorRange` instead of moving the cursor.
/// Document motions (`gg`, `G`) are linewise.
#[allow(clippy::cast_possible_truncation)]
fn execute_jump_line(
    runtime: &mut SessionRuntime<'_>,
    args: &CommandContext,
    target_line: Option<usize>,
) -> CommandResult {
    let Some(buffer_id) = args.buffer_id() else {
        return CommandResult::error("No active buffer");
    };

    // Get cursor from per-client Window (#471)
    let Some(window) = runtime.windows().active() else {
        return CommandResult::error("No active window");
    };
    let old_pos = Position::new(window.cursor.line, window.cursor.column);

    // Calculate motion using with_buffer_read callback
    let motion = Motion::JumpLine(target_line);
    let motion_result = runtime.with_buffer_read(buffer_id, |buffer| {
        let cursor = Cursor::new(old_pos);
        MotionEngine::calculate(buffer, &cursor, motion, 1)
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
// Line Start (0)
// =============================================================================

/// Move cursor to start of line (column 0).
#[derive(Debug, Clone, Copy, Default)]
pub struct LineStart;

impl Command for LineStart {
    fn id(&self) -> CommandId {
        ids::LINE_START
    }

    fn description(&self) -> &'static str {
        "Move to start of line"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![]
    }
}

impl CommandHandler for LineStart {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_line_position(runtime, args, LinePosition::Start)
    }
}

// =============================================================================
// Line End ($)
// =============================================================================

/// Move cursor to end of line.
#[derive(Debug, Clone, Copy, Default)]
pub struct LineEnd;

impl Command for LineEnd {
    fn id(&self) -> CommandId {
        ids::LINE_END
    }

    fn description(&self) -> &'static str {
        "Move to end of line"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![]
    }
}

impl CommandHandler for LineEnd {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_line_position(runtime, args, LinePosition::End)
    }
}

// =============================================================================
// First Non-Blank (^)
// =============================================================================

/// Move cursor to first non-blank character on line.
#[derive(Debug, Clone, Copy, Default)]
pub struct FirstNonBlank;

impl Command for FirstNonBlank {
    fn id(&self) -> CommandId {
        ids::FIRST_NON_BLANK
    }

    fn description(&self) -> &'static str {
        "Move to first non-blank character"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![]
    }
}

impl CommandHandler for FirstNonBlank {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_line_position(runtime, args, LinePosition::FirstNonBlank)
    }
}

// =============================================================================
// Document Start (gg)
// =============================================================================

/// Move cursor to start of document (first line).
#[derive(Debug, Clone, Copy, Default)]
pub struct DocumentStart;

impl Command for DocumentStart {
    fn id(&self) -> CommandId {
        ids::DOCUMENT_START
    }

    fn description(&self) -> &'static str {
        "Move to start of document"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Line number to jump to",
        )]
    }
}

impl CommandHandler for DocumentStart {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        // gg without count goes to line 0
        // With count, go to that line (1-indexed in vim, convert to 0-indexed)
        let target_line = args.count().map(|c| c.saturating_sub(1));
        execute_jump_line(runtime, args, target_line.or(Some(0)))
    }
}

// =============================================================================
// Document End (G)
// =============================================================================

/// Move cursor to end of document (last line) or specific line with count.
#[derive(Debug, Clone, Copy, Default)]
pub struct DocumentEnd;

impl Command for DocumentEnd {
    fn id(&self) -> CommandId {
        ids::DOCUMENT_END
    }

    fn description(&self) -> &'static str {
        "Move to end of document or line N"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Line number to jump to",
        )]
    }
}

impl CommandHandler for DocumentEnd {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        // G without count goes to last line (None)
        // With count, go to that line (1-indexed in vim, convert to 0-indexed)
        let target_line = args.count().map(|c| c.saturating_sub(1));
        execute_jump_line(runtime, args, target_line)
    }
}

// =============================================================================
// Whole Line Motion (for operator doubling: dd, yy, cc)
// =============================================================================

/// Whole line motion for operator doubling (dd, yy, cc).
///
/// In operator-pending mode, this returns the current line as a linewise range.
/// This enables vim's pattern where pressing the operator key twice operates
/// on the current line (e.g., 'd' enters operator-pending, then 'd' again
/// provides the "whole line" motion).
///
/// In normal mode, this is a no-op since there's no operator to apply.
#[derive(Debug, Clone, Copy, Default)]
pub struct WholeLine;

impl Command for WholeLine {
    fn id(&self) -> CommandId {
        ids::WHOLE_LINE
    }

    fn description(&self) -> &'static str {
        "Whole line motion (for operator doubling)"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of lines",
        )]
    }
}

impl CommandHandler for WholeLine {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // WholeLine is used for operator doubling (dd, yy, cc).
        // Per #388: motions just return Success. The vim resolver already
        // knows dd/yy/cc are linewise and handles this in is_line_operator().
        CommandResult::Success
    }
}

// =============================================================================
// Command Registration
// =============================================================================

/// Get all line motion commands as boxed trait objects.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(LineStart),
        Box::new(LineEnd),
        Box::new(FirstNonBlank),
        Box::new(DocumentStart),
        Box::new(DocumentEnd),
        Box::new(WholeLine),
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
            KernelContext, ModeStack, ServiceRegistry,
            v1::{
                Buffer, BufferError, BufferId, BufferManager, EventBus, MarkBank, ModeId, ModuleId,
                OptionRegistry, Position, RegisterBank, RwLock, TextObjectEngine,
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
            let session = Session::new(ClientId::new(1));

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
        let mut session = Session::new(ClientId::new(1));

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
    fn test_line_start_id() {
        let cmd = LineStart;
        assert_eq!(cmd.id().module(), &ids::MODULE);
        assert_eq!(cmd.id().name(), "line-start");
    }

    #[test]
    fn test_line_end_id() {
        let cmd = LineEnd;
        assert_eq!(cmd.id().name(), "line-end");
    }

    #[test]
    fn test_first_non_blank_id() {
        let cmd = FirstNonBlank;
        assert_eq!(cmd.id().name(), "first-non-blank");
    }

    #[test]
    fn test_document_start_id() {
        let cmd = DocumentStart;
        assert_eq!(cmd.id().name(), "document-start");
    }

    #[test]
    fn test_document_end_id() {
        let cmd = DocumentEnd;
        assert_eq!(cmd.id().name(), "document-end");
    }

    // =========================================================================
    // Line Start (0) Tests
    // =========================================================================

    #[test]
    fn test_line_start_basic() {
        let mut setup = TestSetup::new("  hello world");
        setup.set_cursor(Position::new(0, 5)); // Middle of line

        let args = setup.args();
        let result = setup.run(&LineStart, &args);

        assert!(result.is_success());
        assert_eq!(setup.cursor().column, 0);
    }

    #[test]
    fn test_line_start_on_empty_line() {
        let mut setup = TestSetup::new("hello\n\nworld");
        setup.set_cursor(Position::new(1, 0)); // On empty line

        let args = setup.args();
        let result = setup.run(&LineStart, &args);

        assert!(result.is_success());
        assert_eq!(setup.cursor().line, 1);
        assert_eq!(setup.cursor().column, 0);
    }

    // =========================================================================
    // Line End ($) Tests
    // =========================================================================

    #[test]
    fn test_line_end_basic() {
        let mut setup = TestSetup::new("hello world");
        setup.set_cursor(Position::new(0, 0));

        let args = setup.args();
        let result = setup.run(&LineEnd, &args);

        assert!(result.is_success());
        assert_eq!(setup.cursor().column, 10); // Last char 'd' at index 10
    }

    #[test]
    fn test_line_end_on_empty_line() {
        let mut setup = TestSetup::new("hello\n\nworld");
        setup.set_cursor(Position::new(1, 0)); // On empty line

        let args = setup.args();
        let result = setup.run(&LineEnd, &args);

        assert!(result.is_success());
        assert_eq!(setup.cursor().line, 1);
        // Empty line - $ should stay at 0 or go to 0
    }

    // =========================================================================
    // First Non-Blank (^) Tests
    // =========================================================================

    #[test]
    fn test_first_non_blank_basic() {
        let mut setup = TestSetup::new("  hello world");
        setup.set_cursor(Position::new(0, 10)); // At end of line

        let args = setup.args();
        let result = setup.run(&FirstNonBlank, &args);

        assert!(result.is_success());
        assert_eq!(setup.cursor().column, 2); // 'h' is at column 2
    }

    #[test]
    fn test_first_non_blank_no_leading_whitespace() {
        let mut setup = TestSetup::new("hello world");
        setup.set_cursor(Position::new(0, 5)); // In middle

        let args = setup.args();
        let result = setup.run(&FirstNonBlank, &args);

        assert!(result.is_success());
        assert_eq!(setup.cursor().column, 0); // 'h' is at column 0
    }

    // =========================================================================
    // Document Start (gg) Tests
    // =========================================================================

    #[test]
    fn test_document_start_basic() {
        let mut setup = TestSetup::new("line one\nline two\nline three");
        setup.set_cursor(Position::new(2, 3)); // On last line

        let args = setup.args();
        let result = setup.run(&DocumentStart, &args);

        assert!(result.is_success());
        assert_eq!(setup.cursor().line, 0);
        assert_eq!(setup.cursor().column, 0);
    }

    #[test]
    fn test_document_start_with_count() {
        let mut setup = TestSetup::new("line one\nline two\nline three");
        setup.set_cursor(Position::new(0, 0));

        let mut args = setup.args();
        args.set("count", ArgValue::Count(2)); // Go to line 2 (0-indexed: line 1)

        let result = setup.run(&DocumentStart, &args);

        assert!(result.is_success());
        assert_eq!(setup.cursor().line, 1); // Line 2 in vim is line 1 in 0-indexed
    }

    // =========================================================================
    // Document End (G) Tests
    // =========================================================================

    #[test]
    fn test_document_end_basic() {
        let mut setup = TestSetup::new("line one\nline two\nline three");
        setup.set_cursor(Position::new(0, 0));

        let args = setup.args();
        let result = setup.run(&DocumentEnd, &args);

        assert!(result.is_success());
        assert_eq!(setup.cursor().line, 2); // Last line
    }

    #[test]
    fn test_document_end_with_count() {
        let mut setup = TestSetup::new("line one\nline two\nline three");
        setup.set_cursor(Position::new(0, 0)); // At start

        let mut args = setup.args();
        args.set("count", ArgValue::Count(2)); // Go to line 2 (0-indexed: line 1)

        let result = setup.run(&DocumentEnd, &args);

        assert!(result.is_success());
        assert_eq!(setup.cursor().line, 1); // Line 2 in vim is line 1 in 0-indexed
    }

    #[test]
    fn test_document_end_single_line_buffer() {
        let mut setup = TestSetup::new("only line");
        setup.set_cursor(Position::new(0, 0));

        let args = setup.args();
        let result = setup.run(&DocumentEnd, &args);

        assert!(result.is_success());
        assert_eq!(setup.cursor().line, 0); // Only line
    }

    // =========================================================================
    // Error Handling Tests
    // =========================================================================

    #[test]
    fn test_line_motion_no_buffer_returns_error() {
        let result = run_command_no_buffer(&LineStart);
        assert!(result.is_error());
    }

    // =========================================================================
    // Command Count Tests
    // =========================================================================

    #[test]
    fn test_all_commands_count() {
        let cmds = all_commands();
        assert_eq!(cmds.len(), 6); // 0, $, ^, gg, G, whole-line
    }
}
