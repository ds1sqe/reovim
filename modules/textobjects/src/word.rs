//! Word text object commands.
//!
//! Implements text objects: `iw`, `aw`, `iW`, `aW`.
//!
//! These commands calculate the range of a word under/around the cursor
//! for use with operators like delete, yank, and change.

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_kernel::api::v1::{
        CommandId, KernelContext, Position, TextObject, TextObjectEngine, WordBoundary,
    },
};

use crate::TEXTOBJECTS_MODULE;

// =============================================================================
// Helper function
// =============================================================================

/// Execute a word text object and return the range.
#[allow(clippy::significant_drop_tightening)]
fn execute_word_textobj(
    ctx: &KernelContext,
    args: &CommandContext,
    text_object: TextObject,
) -> CommandResult {
    let Some(buffer_id) = args.buffer_id() else {
        return CommandResult::error("No active buffer");
    };

    let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
        return CommandResult::error("Buffer not found");
    };

    let count = args.count().unwrap_or(1);

    // Hold the buffer lock only for the duration needed
    let range_result = {
        let buffer = buffer_arc.read();
        let pos = buffer.position();
        TextObjectEngine::range(&buffer, pos, text_object, count)
    };

    let Some((start, end)) = range_result else {
        return CommandResult::Success; // No-op if no range found
    };

    // Kernel returns inclusive end position, convert to exclusive for Range
    // end.column points to the last character, we need end.column + 1
    let end_exclusive = Position::new(end.line, end.column + 1);

    CommandResult::OperatorRange {
        start,
        end: end_exclusive,
        is_linewise: false,
    }
}

// =============================================================================
// Inner Word (iw)
// =============================================================================

/// Inner word text object.
///
/// Selects the word under the cursor without surrounding whitespace.
#[derive(Debug, Clone, Copy, Default)]
pub struct InnerWord;

impl Command for InnerWord {
    fn id(&self) -> CommandId {
        CommandId::new(TEXTOBJECTS_MODULE, "inner-word")
    }

    fn description(&self) -> &'static str {
        "Inner word text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of words",
        )]
    }
}

impl CommandHandler for InnerWord {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        execute_word_textobj(ctx, args, TextObject::InnerWord(WordBoundary::Word))
    }
}

// =============================================================================
// A Word (aw)
// =============================================================================

/// A word text object.
///
/// Selects the word under the cursor including trailing (or leading) whitespace.
#[derive(Debug, Clone, Copy, Default)]
pub struct AWord;

impl Command for AWord {
    fn id(&self) -> CommandId {
        CommandId::new(TEXTOBJECTS_MODULE, "around-word")
    }

    fn description(&self) -> &'static str {
        "A word text object (including whitespace)"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of words",
        )]
    }
}

impl CommandHandler for AWord {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        execute_word_textobj(ctx, args, TextObject::AWord(WordBoundary::Word))
    }
}

// =============================================================================
// Inner WORD (iW)
// =============================================================================

/// Inner WORD text object.
///
/// Selects the WORD (whitespace-delimited) under the cursor.
#[derive(Debug, Clone, Copy, Default)]
pub struct InnerWordBig;

impl Command for InnerWordBig {
    fn id(&self) -> CommandId {
        CommandId::new(TEXTOBJECTS_MODULE, "inner-word-big")
    }

    fn description(&self) -> &'static str {
        "Inner WORD text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of WORDs",
        )]
    }
}

impl CommandHandler for InnerWordBig {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        execute_word_textobj(ctx, args, TextObject::InnerWord(WordBoundary::BigWord))
    }
}

// =============================================================================
// A WORD (aW)
// =============================================================================

/// A WORD text object.
///
/// Selects the WORD (whitespace-delimited) including surrounding whitespace.
#[derive(Debug, Clone, Copy, Default)]
pub struct AWordBig;

impl Command for AWordBig {
    fn id(&self) -> CommandId {
        CommandId::new(TEXTOBJECTS_MODULE, "around-word-big")
    }

    fn description(&self) -> &'static str {
        "A WORD text object (including whitespace)"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of WORDs",
        )]
    }
}

impl CommandHandler for AWordBig {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        execute_word_textobj(ctx, args, TextObject::AWord(WordBoundary::BigWord))
    }
}

// =============================================================================
// Command Registration
// =============================================================================

/// Get all word text object commands as boxed trait objects.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(InnerWord),
        Box::new(AWord),
        Box::new(InnerWordBig),
        Box::new(AWordBig),
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
            Buffer, BufferError, BufferId, BufferManager, EventBus, MarkBank, MotionEngine,
            OptionRegistry, RegisterBank, RwLock,
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

    // =========================================================================
    // Command ID Tests
    // =========================================================================

    #[test]
    fn test_inner_word_id() {
        let cmd = InnerWord;
        assert_eq!(cmd.id().module(), &TEXTOBJECTS_MODULE);
        assert_eq!(cmd.id().name(), "inner-word");
    }

    #[test]
    fn test_around_word_id() {
        let cmd = AWord;
        assert_eq!(cmd.id().name(), "around-word");
    }

    #[test]
    fn test_inner_word_big_id() {
        let cmd = InnerWordBig;
        assert_eq!(cmd.id().name(), "inner-word-big");
    }

    #[test]
    fn test_around_word_big_id() {
        let cmd = AWordBig;
        assert_eq!(cmd.id().name(), "around-word-big");
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
    fn test_inner_word_no_buffer_returns_error() {
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();
        let result = InnerWord.execute(&mut ctx, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_inner_word_invalid_buffer_returns_error() {
        let mut ctx = KernelContext::default();
        let mut args = CommandContext::new();
        args.set("buffer_id", ArgValue::BufferId(999));
        let result = InnerWord.execute(&mut ctx, &args);
        assert!(result.is_error());
    }

    // =========================================================================
    // Inner Word (iw) Tests
    // =========================================================================

    #[test]
    fn test_inner_word_basic() {
        let mut ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "hello world foo");

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = InnerWord.execute(&mut ctx, &args);

        // Should return OperatorRange for "hello" (columns 0-4 inclusive -> 0-5 exclusive)
        match result {
            CommandResult::OperatorRange {
                start,
                end,
                is_linewise,
            } => {
                assert_eq!(start.line, 0);
                assert_eq!(start.column, 0);
                assert_eq!(end.line, 0);
                assert_eq!(end.column, 5); // Exclusive end
                assert!(!is_linewise);
            }
            _ => panic!("Expected OperatorRange, got {result:?}"),
        }
    }

    #[test]
    fn test_inner_word_middle_of_word() {
        let mut ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "hello world");

        // Position cursor at 'l' in "hello"
        {
            let buffer = ctx.buffers.get(buffer_id).unwrap();
            buffer.write().set_position(Position::new(0, 2));
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = InnerWord.execute(&mut ctx, &args);

        match result {
            CommandResult::OperatorRange { start, end, .. } => {
                assert_eq!(start.column, 0); // Start of "hello"
                assert_eq!(end.column, 5); // End exclusive
            }
            _ => panic!("Expected OperatorRange"),
        }
    }

    // =========================================================================
    // A Word (aw) Tests
    // =========================================================================

    #[test]
    fn test_a_word_includes_trailing_whitespace() {
        let mut ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "hello world");

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = AWord.execute(&mut ctx, &args);

        match result {
            CommandResult::OperatorRange { start, end, .. } => {
                assert_eq!(start.column, 0);
                // "hello " (6 chars) -> columns 0-5 inclusive, exclusive end = 6
                assert_eq!(end.column, 6);
            }
            _ => panic!("Expected OperatorRange"),
        }
    }

    // =========================================================================
    // BigWord Tests
    // =========================================================================

    #[test]
    fn test_inner_word_big_skips_punctuation() {
        let mut ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "hello-world foo");

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = InnerWordBig.execute(&mut ctx, &args);

        match result {
            CommandResult::OperatorRange { start, end, .. } => {
                assert_eq!(start.column, 0);
                // "hello-world" (11 chars) -> columns 0-10 inclusive, exclusive end = 11
                assert_eq!(end.column, 11);
            }
            _ => panic!("Expected OperatorRange"),
        }
    }

    #[test]
    fn test_inner_word_small_stops_at_punctuation() {
        let mut ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "hello-world foo");

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = InnerWord.execute(&mut ctx, &args);

        match result {
            CommandResult::OperatorRange { start, end, .. } => {
                assert_eq!(start.column, 0);
                // "hello" (5 chars) -> columns 0-4 inclusive, exclusive end = 5
                assert_eq!(end.column, 5);
            }
            _ => panic!("Expected OperatorRange"),
        }
    }

    // =========================================================================
    // Edge Cases
    // =========================================================================

    #[test]
    fn test_empty_buffer() {
        let mut ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "");

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = InnerWord.execute(&mut ctx, &args);
        // Empty buffer should return Success (no-op) since there's no word
        assert!(matches!(result, CommandResult::Success | CommandResult::OperatorRange { .. }));
    }

    #[test]
    fn test_whitespace_only() {
        let mut ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, "   ");

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = InnerWord.execute(&mut ctx, &args);
        // On whitespace, inner word selects the whitespace run
        match result {
            CommandResult::OperatorRange { start, end, .. } => {
                assert_eq!(start.column, 0);
                assert_eq!(end.column, 3); // "   " -> 3 chars
            }
            CommandResult::Success => {} // Also acceptable for edge case
            _ => panic!("Unexpected result: {result:?}"),
        }
    }
}
