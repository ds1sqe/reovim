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
    reovim_kernel::api::v1::{
        CommandId, Cursor, KernelContext, LinePosition, Motion, MotionEngine, events::CursorMoved,
    },
};

use super::MOTIONS_MODULE;

// =============================================================================
// Helper functions
// =============================================================================

/// Execute a line position motion and update cursor position.
///
/// In operator-pending mode, returns an `OperatorRange` instead of moving the cursor.
/// Line position motions (`0`, `$`, `^`) are characterwise.
#[allow(clippy::cast_possible_truncation)]
fn execute_line_position(
    ctx: &KernelContext,
    args: &CommandContext,
    position: LinePosition,
) -> CommandResult {
    let Some(buffer_id) = args.buffer_id() else {
        return CommandResult::error("No active buffer");
    };

    let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
        return CommandResult::error("Buffer not found");
    };

    let buffer = buffer_arc.read();
    let cursor = Cursor::new(buffer.position());
    let old_pos = cursor.position;

    let motion = Motion::LinePosition(position);
    let Some(new_pos) = MotionEngine::calculate(&buffer, &cursor, motion, 1) else {
        drop(buffer);
        return CommandResult::Success; // No-op if motion fails
    };

    if new_pos == old_pos {
        drop(buffer);
        return CommandResult::Success; // No movement
    }

    drop(buffer);

    // In operator-pending mode, return range for the operator
    if args.is_operator_pending() {
        // Determine range direction - start should be before end
        let (start, range_end) = if new_pos.column >= old_pos.column {
            (old_pos, new_pos)
        } else {
            (new_pos, old_pos)
        };
        // Line position motions are characterwise
        return CommandResult::operator_range(start, range_end, false);
    }

    // Normal mode: move cursor
    {
        let mut buffer = buffer_arc.write();
        buffer.set_position(new_pos);
    }

    ctx.event_bus.emit(CursorMoved {
        buffer_id: buffer_id.as_usize() as u64,
        from: (old_pos.line as u32, old_pos.column as u32),
        to: (new_pos.line as u32, new_pos.column as u32),
    });

    CommandResult::Success
}

/// Execute a jump line motion and update cursor position.
///
/// In operator-pending mode, returns an `OperatorRange` instead of moving the cursor.
/// Document motions (`gg`, `G`) are linewise.
#[allow(clippy::cast_possible_truncation)]
fn execute_jump_line(
    ctx: &KernelContext,
    args: &CommandContext,
    target_line: Option<usize>,
) -> CommandResult {
    let Some(buffer_id) = args.buffer_id() else {
        return CommandResult::error("No active buffer");
    };

    let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
        return CommandResult::error("Buffer not found");
    };

    let buffer = buffer_arc.read();
    let cursor = Cursor::new(buffer.position());
    let old_pos = cursor.position;

    let motion = Motion::JumpLine(target_line);
    let Some(new_pos) = MotionEngine::calculate(&buffer, &cursor, motion, 1) else {
        drop(buffer);
        return CommandResult::Success; // No-op if motion fails
    };

    if new_pos == old_pos {
        drop(buffer);
        return CommandResult::Success; // No movement
    }

    drop(buffer);

    // In operator-pending mode, return range for the operator
    if args.is_operator_pending() {
        // Determine range direction - start line should be before end line
        let (start, range_end) = if new_pos.line >= old_pos.line {
            (old_pos, new_pos)
        } else {
            (new_pos, old_pos)
        };
        // Document motions are linewise
        return CommandResult::operator_range(start, range_end, true);
    }

    // Normal mode: move cursor
    {
        let mut buffer = buffer_arc.write();
        buffer.set_position(new_pos);
    }

    ctx.event_bus.emit(CursorMoved {
        buffer_id: buffer_id.as_usize() as u64,
        from: (old_pos.line as u32, old_pos.column as u32),
        to: (new_pos.line as u32, new_pos.column as u32),
    });

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
        CommandId::new(MOTIONS_MODULE, "line-start")
    }

    fn description(&self) -> &'static str {
        "Move to start of line"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![]
    }
}

impl CommandHandler for LineStart {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        execute_line_position(ctx, args, LinePosition::Start)
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
        CommandId::new(MOTIONS_MODULE, "line-end")
    }

    fn description(&self) -> &'static str {
        "Move to end of line"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![]
    }
}

impl CommandHandler for LineEnd {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        execute_line_position(ctx, args, LinePosition::End)
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
        CommandId::new(MOTIONS_MODULE, "first-non-blank")
    }

    fn description(&self) -> &'static str {
        "Move to first non-blank character"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![]
    }
}

impl CommandHandler for FirstNonBlank {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        execute_line_position(ctx, args, LinePosition::FirstNonBlank)
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
        CommandId::new(MOTIONS_MODULE, "document-start")
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
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        // gg without count goes to line 0
        // With count, go to that line (1-indexed in vim, convert to 0-indexed)
        let target_line = args.count().map(|c| c.saturating_sub(1));
        execute_jump_line(ctx, args, target_line.or(Some(0)))
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
        CommandId::new(MOTIONS_MODULE, "document-end")
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
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        // G without count goes to last line (None)
        // With count, go to that line (1-indexed in vim, convert to 0-indexed)
        let target_line = args.count().map(|c| c.saturating_sub(1));
        execute_jump_line(ctx, args, target_line)
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
        reovim_kernel::api::v1::{
            Buffer, BufferError, BufferId, BufferManager, EventBus, MarkBank, OptionRegistry,
            Position, RegisterBank, RwLock, TextObjectEngine,
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

    // =========================================================================
    // Command ID Tests
    // =========================================================================

    #[test]
    fn test_line_start_id() {
        let cmd = LineStart;
        assert_eq!(cmd.id().module(), &MOTIONS_MODULE);
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
        let mut ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "  hello world");

        // Position in middle of line
        {
            let buffer = ctx.buffers.get(buffer_id).unwrap();
            buffer.write().set_position(Position::new(0, 5));
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = LineStart.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.column, 0);
    }

    #[test]
    fn test_line_start_on_empty_line() {
        let mut ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "hello\n\nworld");

        // Position on empty line
        {
            let buffer = ctx.buffers.get(buffer_id).unwrap();
            buffer.write().set_position(Position::new(1, 0));
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = LineStart.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.line, 1);
        assert_eq!(pos.column, 0);
    }

    // =========================================================================
    // Line End ($) Tests
    // =========================================================================

    #[test]
    fn test_line_end_basic() {
        let mut ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "hello world");

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = LineEnd.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.column, 10); // Last char 'd' at index 10
    }

    #[test]
    fn test_line_end_on_empty_line() {
        let mut ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "hello\n\nworld");

        // Position on empty line
        {
            let buffer = ctx.buffers.get(buffer_id).unwrap();
            buffer.write().set_position(Position::new(1, 0));
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = LineEnd.execute(&mut ctx, &args);
        assert!(result.is_success());

        // Empty line - $ should stay at 0 or go to 0
        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.line, 1);
    }

    // =========================================================================
    // First Non-Blank (^) Tests
    // =========================================================================

    #[test]
    fn test_first_non_blank_basic() {
        let mut ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "  hello world");

        // Position at end of line
        {
            let buffer = ctx.buffers.get(buffer_id).unwrap();
            buffer.write().set_position(Position::new(0, 10));
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = FirstNonBlank.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.column, 2); // 'h' is at column 2
    }

    #[test]
    fn test_first_non_blank_no_leading_whitespace() {
        let mut ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "hello world");

        // Position in middle
        {
            let buffer = ctx.buffers.get(buffer_id).unwrap();
            buffer.write().set_position(Position::new(0, 5));
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = FirstNonBlank.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.column, 0); // 'h' is at column 0
    }

    // =========================================================================
    // Document Start (gg) Tests
    // =========================================================================

    #[test]
    fn test_document_start_basic() {
        let mut ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "line one\nline two\nline three");

        // Position on last line
        {
            let buffer = ctx.buffers.get(buffer_id).unwrap();
            buffer.write().set_position(Position::new(2, 3));
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = DocumentStart.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.line, 0);
        assert_eq!(pos.column, 0); // First non-blank (or 0 if no leading whitespace)
    }

    #[test]
    fn test_document_start_with_count() {
        let mut ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "line one\nline two\nline three");

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(2)); // Go to line 2 (0-indexed: line 1)

        let result = DocumentStart.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.line, 1); // Line 2 in vim is line 1 in 0-indexed
    }

    // =========================================================================
    // Document End (G) Tests
    // =========================================================================

    #[test]
    fn test_document_end_basic() {
        let mut ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "line one\nline two\nline three");

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = DocumentEnd.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.line, 2); // Last line
    }

    #[test]
    fn test_document_end_with_count() {
        let mut ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "line one\nline two\nline three");

        // Position at start
        {
            let buffer = ctx.buffers.get(buffer_id).unwrap();
            buffer.write().set_position(Position::new(0, 0));
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(2)); // Go to line 2 (0-indexed: line 1)

        let result = DocumentEnd.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.line, 1); // Line 2 in vim is line 1 in 0-indexed
    }

    #[test]
    fn test_document_end_single_line_buffer() {
        let mut ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "only line");

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = DocumentEnd.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.line, 0); // Only line
    }

    // =========================================================================
    // Error Handling Tests
    // =========================================================================

    #[test]
    fn test_line_motion_no_buffer_returns_error() {
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();
        let result = LineStart.execute(&mut ctx, &args);
        assert!(result.is_error());
    }

    // =========================================================================
    // Command Count Tests
    // =========================================================================

    #[test]
    fn test_all_commands_count() {
        let cmds = all_commands();
        assert_eq!(cmds.len(), 5); // 0, $, ^, gg, G
    }
}
