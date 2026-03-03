//! Yank operator.
//!
//! Reference: lib/core/src/command/builtin/operator.rs (concept-extraction, not migration)

use reovim_kernel::api::v1::RegisterContent;

use super::{Operator, OperatorContext, OperatorError, Range, registers};

/// Yank operator - copies text to register.
///
/// Behavior:
/// - Copies text in the given range to register
/// - Does NOT modify the buffer
/// - Linewise if the motion was linewise
///
/// # Example
///
/// ```ignore
/// let yank = YankOperator;
/// yank.execute(&mut ctx, range)?;
/// ```
#[derive(Debug, Clone, Copy)]
pub struct YankOperator;

impl Operator for YankOperator {
    fn id(&self) -> &'static str {
        "yank"
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, ctx: &mut OperatorContext<'_>, range: Range) -> Result<(), OperatorError> {
        // Get the buffer via kernel's buffer manager
        let buffer_arc = ctx
            .kernel
            .buffers
            .get(ctx.buffer_id)
            .ok_or(OperatorError::BufferNotFound(ctx.buffer_id))?;

        let buffer = buffer_arc.read();

        // Build yanked text from lines
        let start = range.start;
        let end = range.end;
        let mut yanked_text = String::new();
        let lines = buffer.lines();

        if range.is_linewise {
            // Linewise yank: copy entire lines from start.line to end.line (inclusive)
            // Ignore column values - always yank full lines
            // Clamp end.line to last valid line to handle counts exceeding buffer
            let line_count = lines.len();
            let clamped_end = end.line.min(line_count.saturating_sub(1));

            for line_idx in start.line..=clamped_end {
                if let Some(line) = lines.get(line_idx) {
                    yanked_text.push_str(line);
                    yanked_text.push('\n');
                }
            }
        } else if start.line == end.line {
            // Single line characterwise yank
            // start.line is valid: buffer exists and lines were just obtained from it
            let line = &lines[start.line];
            let start_col = start.column.min(line.len());
            let end_col = end.column.min(line.len());
            if start_col < end_col {
                yanked_text.push_str(&line[start_col..end_col]);
            }
        } else {
            // Multi-line characterwise yank
            // All indices in start.line..=end.line are valid: lines were obtained
            // from the same buffer snapshot and end.line <= last valid line
            for (line_idx, line) in lines.iter().enumerate().take(end.line + 1).skip(start.line) {
                if line_idx == start.line {
                    let start_col = start.column.min(line.len());
                    yanked_text.push_str(&line[start_col..]);
                    yanked_text.push('\n');
                } else if line_idx == end.line {
                    let end_col = end.column.min(line.len());
                    yanked_text.push_str(&line[..end_col]);
                } else {
                    yanked_text.push_str(line);
                    yanked_text.push('\n');
                }
            }
        }

        // Release the buffer lock before register operations
        drop(buffer);

        // Store in register - linewise if the range was linewise
        let content = if range.is_linewise {
            RegisterContent::linewise(yanked_text)
        } else {
            RegisterContent::characterwise(yanked_text)
        };

        // Store to the specified register (handles +, *, a-z, etc.)
        // Uses ClipboardProvider for +/* registers, per-client RegisterBank for others (#515)
        registers::store_and_sync(ctx.kernel, ctx.registers, ctx.register, &content);

        // Push to per-client history for numbered registers (0-9) (#515)
        // This happens on every yank regardless of target register
        registers::push_to_history(ctx.clipboard_history, &content);

        Ok(())
    }

    fn is_linewise(&self) -> bool {
        false // Default; actual linewise-ness is determined by motion
    }

    fn is_text_modifying(&self) -> bool {
        false // Yank does NOT modify text
    }
}

#[cfg(test)]
#[allow(
    clippy::uninlined_format_args,
    clippy::significant_drop_tightening,
    clippy::drop_non_drop
)]
mod tests {
    use {
        super::*,
        reovim_driver_command::{CommandContext, CommandHandler, CommandResult},
        reovim_driver_session::{
            ClientId, ExtensionMap, Session, SessionRuntime, WindowLayout, api::CommandExecutor,
        },
        reovim_kernel::api::{
            ModeStack,
            v1::{
                Buffer, BufferError, BufferId, BufferManager, CommandId, EventBus, HistoryRing,
                KernelContext, MarkBank, ModeId, ModuleId, MotionEngine, OptionRegistry, Position,
                Register, RegisterBank, RwLock, ServiceRegistry, TextObjectEngine,
            },
        },
        std::{collections::HashMap, sync::Arc},
    };

    fn run_command<C: CommandHandler>(
        cmd: &C,
        ctx: &KernelContext,
        args: &CommandContext,
    ) -> CommandResult {
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
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone()); // #491
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = reovim_driver_session::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
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
            },
            ctx,
            &executor,
        );
        cmd.execute(&mut runtime, args)
    }

    /// Test buffer manager that actually stores buffers.
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

    /// Create a `KernelContext` with a real buffer manager for testing.
    fn create_test_context() -> KernelContext {
        KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(RwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::default()),
            Arc::new(ServiceRegistry::new()),
        )
    }

    #[test]
    fn test_yank_operator_id() {
        let yank = YankOperator;
        assert_eq!(yank.id(), "yank");
    }

    #[test]
    fn test_yank_is_not_text_modifying() {
        let yank = YankOperator;
        assert!(!yank.is_text_modifying());
    }

    #[test]
    fn test_yank_is_not_linewise_by_default() {
        let yank = YankOperator;
        assert!(!yank.is_linewise());
    }

    // ========================================================================
    // Yank execute tests
    // ========================================================================

    #[test]
    fn test_yank_buffer_not_found() {
        let ctx = create_test_context();
        let yank = YankOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id: BufferId::from_raw(999),
            register: Register::Default,
            count: 1,
            cursor_position: Position::new(0, 0),
        };
        let range = super::super::Range::new(Position::new(0, 0), Position::new(0, 5));
        let result = yank.execute(&mut op_ctx, range);
        assert!(result.is_err());
    }

    #[test]
    fn test_yank_characterwise_single_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let yank = YankOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Default,
            count: 1,
            cursor_position: Position::new(0, 0),
        };
        // Yank "hello" (columns 0..5)
        let range = super::super::Range::new(Position::new(0, 0), Position::new(0, 5));
        let result = yank.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        // Check unnamed register has "hello"
        drop(op_ctx);
        assert_eq!(registers.get().text, "hello");
    }

    #[test]
    fn test_yank_characterwise_partial_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let yank = YankOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Default,
            count: 1,
            cursor_position: Position::new(0, 0),
        };
        // Yank "world" (columns 6..11)
        let range = super::super::Range::new(Position::new(0, 6), Position::new(0, 11));
        let result = yank.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get().text, "world");
    }

    #[test]
    fn test_yank_characterwise_multi_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld\nfoo");
        let buffer_id = ctx.buffers.register(buffer);

        let yank = YankOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Default,
            count: 1,
            cursor_position: Position::new(0, 0),
        };
        // Yank from (0,3) to (1,3) => "lo\nwor"
        let range = super::super::Range::new(Position::new(0, 3), Position::new(1, 3));
        let result = yank.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get().text, "lo\nwor");
    }

    #[test]
    fn test_yank_linewise_single_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let yank = YankOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Default,
            count: 1,
            cursor_position: Position::new(0, 0),
        };
        // Yank line 0 (linewise)
        let range = super::super::Range::linewise(Position::new(0, 0), Position::new(0, 0));
        let result = yank.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get().text, "hello\n");
    }

    #[test]
    fn test_yank_linewise_multiple_lines() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("line1\nline2\nline3");
        let buffer_id = ctx.buffers.register(buffer);

        let yank = YankOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Default,
            count: 1,
            cursor_position: Position::new(0, 0),
        };
        // Yank lines 0-1 (linewise)
        let range = super::super::Range::linewise(Position::new(0, 0), Position::new(1, 0));
        let result = yank.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get().text, "line1\nline2\n");
    }

    #[test]
    fn test_yank_linewise_clamped_end() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("line1\nline2");
        let buffer_id = ctx.buffers.register(buffer);

        let yank = YankOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Default,
            count: 1,
            cursor_position: Position::new(0, 0),
        };
        // Yank beyond buffer end (end line 99, should be clamped to 1)
        let range = super::super::Range::linewise(Position::new(0, 0), Position::new(99, 0));
        let result = yank.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get().text, "line1\nline2\n");
    }

    #[test]
    fn test_yank_to_named_register() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let yank = YankOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Slot('a'),
            count: 1,
            cursor_position: Position::new(0, 0),
        };
        let range = super::super::Range::new(Position::new(0, 0), Position::new(0, 5));
        let result = yank.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get_named('a').map(|r| r.text.as_str()), Some("hello"));
    }

    #[test]
    fn test_yank_does_not_modify_buffer() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let yank = YankOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Default,
            count: 1,
            cursor_position: Position::new(0, 0),
        };
        let range = super::super::Range::new(Position::new(0, 0), Position::new(0, 5));
        yank.execute(&mut op_ctx, range).unwrap();

        // Buffer content should be unchanged
        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        assert_eq!(buf.lines(), &["hello world"]);
    }

    #[test]
    fn test_yank_empty_range() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let yank = YankOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Default,
            count: 1,
            cursor_position: Position::new(0, 0),
        };
        // Empty range (same start/end column)
        let range = super::super::Range::new(Position::new(0, 3), Position::new(0, 3));
        let result = yank.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get().text, "");
    }

    #[test]
    fn test_yank_characterwise_three_lines() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("aaa\nbbb\nccc");
        let buffer_id = ctx.buffers.register(buffer);

        let yank = YankOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Default,
            count: 1,
            cursor_position: Position::new(0, 0),
        };
        // Yank from (0,1) to (2,2) => "aa\nbbb\ncc"
        let range = super::super::Range::new(Position::new(0, 1), Position::new(2, 2));
        let result = yank.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get().text, "aa\nbbb\ncc");
    }

    #[test]
    fn test_yank_linewise_all_lines() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("a\nb\nc");
        let buffer_id = ctx.buffers.register(buffer);

        let yank = YankOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Default,
            count: 1,
            cursor_position: Position::new(0, 0),
        };
        let range = super::super::Range::linewise(Position::new(0, 0), Position::new(2, 0));
        let result = yank.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get().text, "a\nb\nc\n");
    }

    #[test]
    fn test_yank_column_clamped_to_line_length() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hi");
        let buffer_id = ctx.buffers.register(buffer);

        let yank = YankOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Default,
            count: 1,
            cursor_position: Position::new(0, 0),
        };
        // End column beyond line length should be clamped
        let range = super::super::Range::new(Position::new(0, 0), Position::new(0, 100));
        let result = yank.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get().text, "hi");
    }

    #[test]
    fn test_yank_linewise_single_line_only() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("only line");
        let buffer_id = ctx.buffers.register(buffer);

        let yank = YankOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Default,
            count: 1,
            cursor_position: Position::new(0, 0),
        };
        // Yank the only line
        let range = super::super::Range::linewise(Position::new(0, 0), Position::new(0, 0));
        let result = yank.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get().text, "only line\n");
    }

    #[test]
    fn test_yank_to_register_z() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let yank = YankOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Slot('z'),
            count: 1,
            cursor_position: Position::new(0, 0),
        };
        let range = super::super::Range::new(Position::new(0, 6), Position::new(0, 11));
        let result = yank.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get_named('z').map(|r| r.text.as_str()), Some("world"));
    }

    #[test]
    fn test_yank_linewise_last_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("first\nlast");
        let buffer_id = ctx.buffers.register(buffer);

        let yank = YankOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Default,
            count: 1,
            cursor_position: Position::new(1, 0),
        };
        let range = super::super::Range::linewise(Position::new(1, 0), Position::new(1, 0));
        let result = yank.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get().text, "last\n");
    }

    #[test]
    fn test_yank_characterwise_single_char() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let yank = YankOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Default,
            count: 1,
            cursor_position: Position::new(0, 0),
        };
        // Yank just 'h'
        let range = super::super::Range::new(Position::new(0, 0), Position::new(0, 1));
        let result = yank.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get().text, "h");
    }

    #[test]
    fn test_yank_operator_clone() {
        let yank = YankOperator;
        let cloned = yank;
        assert_eq!(cloned.id(), "yank");
    }

    #[test]
    fn test_yank_operator_debug() {
        let debug = format!("{:?}", YankOperator);
        assert!(debug.contains("YankOperator"));
    }

    #[test]
    fn test_yank_characterwise_middle_of_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("abcdefgh");
        let buffer_id = ctx.buffers.register(buffer);

        let yank = YankOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Default,
            count: 1,
            cursor_position: Position::new(0, 2),
        };
        // Yank "cde" (columns 2..5)
        let range = super::super::Range::new(Position::new(0, 2), Position::new(0, 5));
        let result = yank.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get().text, "cde");
    }

    #[test]
    fn test_yank_linewise_middle_lines() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("a\nb\nc\nd\ne");
        let buffer_id = ctx.buffers.register(buffer);

        let yank = YankOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Default,
            count: 1,
            cursor_position: Position::new(1, 0),
        };
        // Yank lines 1-3 (b, c, d)
        let range = super::super::Range::linewise(Position::new(1, 0), Position::new(3, 0));
        let result = yank.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get().text, "b\nc\nd\n");
    }

    #[test]
    fn test_yank_multiline_two_lines() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let yank = YankOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Default,
            count: 1,
            cursor_position: Position::new(0, 2),
        };
        // Yank from (0,2) to (1,3) => "llo\nwor"
        let range = super::super::Range::new(Position::new(0, 2), Position::new(1, 3));
        let result = yank.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get().text, "llo\nwor");
    }

    // ========================================================================
    // run_command helper exercise tests
    // ========================================================================

    #[test]
    fn test_run_command_helper_with_noop() {
        // Exercise the run_command helper to cover its SessionRuntime setup code
        struct NoopCmd;
        #[cfg_attr(coverage_nightly, coverage(off))]
        impl reovim_driver_command::Command for NoopCmd {
            fn id(&self) -> CommandId {
                CommandId::new(ModuleId::new("test"), "noop")
            }
            fn description(&self) -> &'static str {
                "noop"
            }
        }
        #[cfg_attr(coverage_nightly, coverage(off))]
        impl CommandHandler for NoopCmd {
            fn execute(&self, _: &mut SessionRuntime<'_>, _: &CommandContext) -> CommandResult {
                CommandResult::Success
            }
        }
        let ctx = create_test_context();
        let args = CommandContext::new();
        let result = run_command(&NoopCmd, &ctx, &args);
        assert_eq!(result, CommandResult::Success);
    }

    // ========================================================================
    // Register type verification tests
    // ========================================================================

    #[test]
    fn test_yank_characterwise_register_is_characterwise() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let yank = YankOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Default,
            count: 1,
            cursor_position: Position::new(0, 0),
        };
        let range = super::super::Range::new(Position::new(0, 0), Position::new(0, 5));
        yank.execute(&mut op_ctx, range).unwrap();

        drop(op_ctx);
        assert!(registers.get().is_characterwise());
    }

    #[test]
    fn test_yank_linewise_register_is_linewise() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let yank = YankOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Default,
            count: 1,
            cursor_position: Position::new(0, 0),
        };
        let range = super::super::Range::linewise(Position::new(0, 0), Position::new(0, 0));
        yank.execute(&mut op_ctx, range).unwrap();

        drop(op_ctx);
        assert!(registers.get().is_linewise());
    }

    #[test]
    fn test_yank_linewise_all_lines_register_is_linewise() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("a\nb\nc");
        let buffer_id = ctx.buffers.register(buffer);

        let yank = YankOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Default,
            count: 1,
            cursor_position: Position::new(0, 0),
        };
        let range = super::super::Range::linewise(Position::new(0, 0), Position::new(2, 0));
        yank.execute(&mut op_ctx, range).unwrap();

        drop(op_ctx);
        assert!(registers.get().is_linewise());
        assert_eq!(registers.get().text, "a\nb\nc\n");
    }

    #[test]
    fn test_yank_multiline_characterwise_register_is_characterwise() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let yank = YankOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Default,
            count: 1,
            cursor_position: Position::new(0, 2),
        };
        let range = super::super::Range::new(Position::new(0, 2), Position::new(1, 3));
        yank.execute(&mut op_ctx, range).unwrap();

        drop(op_ctx);
        assert!(registers.get().is_characterwise());
    }
}
