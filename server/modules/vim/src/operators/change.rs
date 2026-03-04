//! Change operator.
//!
//! Reference: lib/core/src/command/builtin/operator.rs (concept-extraction, not migration)

use {
    reovim_driver_undo::{UndoKey, UndoProviderRegistry},
    reovim_kernel::api::v1::{Edit, RegisterContent},
};

use super::{Operator, OperatorContext, OperatorError, Range, registers};

/// Change operator - cuts text and signals insert mode.
///
/// Behavior:
/// - Deletes text in the given range
/// - Stores deleted text in the unnamed register (or specified register)
/// - Signals that insert mode should be entered (via return value or event)
///
/// Note: The actual mode change is handled by the caller (server/display driver).
/// The operator just deletes the text.
///
/// # Example
///
/// ```ignore
/// let change = ChangeOperator;
/// change.execute(&mut ctx, range)?;
/// // Caller should now enter insert mode
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ChangeOperator;

impl Operator for ChangeOperator {
    fn id(&self) -> &'static str {
        "change"
    }

    #[allow(clippy::option_if_let_else)]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, ctx: &mut OperatorContext<'_>, range: Range) -> Result<(), OperatorError> {
        // Get the buffer via kernel's buffer manager
        let buffer_arc = ctx
            .kernel
            .buffers
            .get(ctx.buffer_id)
            .ok_or(OperatorError::BufferNotFound(ctx.buffer_id))?;

        let mut buffer = buffer_arc.write();

        // Use cursor position from context (passed from caller who has window access)
        let cursor_before = ctx.cursor_position;

        // Get text before deleting
        let start = range.start;
        let end = range.end;

        // Build deleted text from lines
        let mut deleted_text = String::new();
        let lines = buffer.lines();
        let line_count = lines.len();

        // Track what was actually deleted for undo
        let delete_pos;

        if range.is_linewise {
            // Linewise change: delete content of lines from start.line to end.line (inclusive)
            // Unlike delete, change keeps ONE line for insertion (Vim behavior)
            // Clamp end.line to last valid line to handle counts exceeding buffer
            let clamped_end = end.line.min(line_count.saturating_sub(1));

            for line_idx in start.line..=clamped_end {
                if let Some(line) = lines.get(line_idx) {
                    deleted_text.push_str(line);
                    deleted_text.push('\n');
                }
            }

            // For linewise change (cc):
            // - Delete content of all affected lines
            // - Keep ONE empty line at start.line for insertion
            // This means: delete from (start.line, 0) to (clamped_end, end_of_content),
            // then if multiple lines, delete the extra newlines to leave just one line
            let delete_start = reovim_kernel::api::v1::Position::new(start.line, 0);
            let delete_end = if clamped_end + 1 < line_count {
                // Not the last line - delete content but preserve start.line's newline
                // So delete from (start.line, 0) to (clamped_end + 1, 0), then we're on next line
                // Actually for cc on middle line, we want to replace lines with one empty line
                // Delete everything from start to clamped_end (including their newlines except last)
                // Let's delete to end of clamped_end, then the newline stays
                // clamped_end is always valid: end.line.min(line_count - 1) < lines.len()
                {
                    let end_line_content = &lines[clamped_end];
                    // Delete all lines but keep start.line as empty (with its newline)
                    // Delete from start.line to end of clamped_end content, plus all intermediate newlines
                    reovim_kernel::api::v1::Position::new(
                        clamped_end,
                        end_line_content.chars().count(),
                    )
                }
            } else {
                // End line is last line - delete to end of content (keep line structure)
                // clamped_end is always valid: end.line.min(line_count - 1) < lines.len()
                let last_line = &lines[clamped_end];
                reovim_kernel::api::v1::Position::new(clamped_end, last_line.chars().count())
            };

            delete_pos = delete_start;
            buffer.delete_range(delete_start, delete_end);
        } else if start.line == end.line {
            // Single line characterwise change
            // start.line is valid: buffer exists and lines were just obtained from it
            let line = &lines[start.line];
            let start_col = start.column.min(line.len());
            let end_col = end.column.min(line.len());
            if start_col < end_col {
                deleted_text.push_str(&line[start_col..end_col]);
            }
            delete_pos = start;
            buffer.delete_range(start, end);
        } else {
            // Multi-line characterwise change
            // All indices in start.line..=end.line are valid: lines were obtained
            // from the same buffer snapshot and end.line <= last valid line
            for (line_idx, line) in lines.iter().enumerate().take(end.line + 1).skip(start.line) {
                if line_idx == start.line {
                    let start_col = start.column.min(line.len());
                    deleted_text.push_str(&line[start_col..]);
                    deleted_text.push('\n');
                } else if line_idx == end.line {
                    let end_col = end.column.min(line.len());
                    deleted_text.push_str(&line[..end_col]);
                } else {
                    deleted_text.push_str(line);
                    deleted_text.push('\n');
                }
            }
            delete_pos = start;
            buffer.delete_range(start, end);
        }

        // Cursor after change is at delete_pos (where insertion will happen)
        let cursor_after = delete_pos;

        drop(buffer);

        // Record edit for undo
        if !deleted_text.is_empty()
            && let Some(undo_registry) = ctx.kernel.services.get::<UndoProviderRegistry>()
            && let Some(undo_provider) = undo_registry.get(&UndoKey::Buffer)
        {
            let edit = Edit::Delete {
                position: delete_pos,
                text: deleted_text.clone(),
            };
            undo_provider.record(ctx.buffer_id, vec![edit], cursor_before, cursor_after);
        }

        // Store in register - linewise if the range was linewise (handles +/* via ClipboardProvider)
        let content = if range.is_linewise {
            RegisterContent::linewise(deleted_text)
        } else {
            RegisterContent::characterwise(deleted_text)
        };

        registers::store_and_sync(ctx.kernel, ctx.registers, ctx.register, &content);
        registers::push_to_history(ctx.clipboard_history, &content);

        // Note: Insert mode transition is handled by the caller
        // The server/display driver should check operator id and enter insert mode

        Ok(())
    }

    fn is_linewise(&self) -> bool {
        false // Default; actual linewise-ness is determined by motion
    }

    fn is_text_modifying(&self) -> bool {
        true
    }
}

#[cfg(test)]
#[allow(
    clippy::significant_drop_tightening,
    clippy::uninlined_format_args,
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
    fn test_change_operator_id() {
        let change = ChangeOperator;
        assert_eq!(change.id(), "change");
    }

    #[test]
    fn test_change_is_text_modifying() {
        let change = ChangeOperator;
        assert!(change.is_text_modifying());
    }

    #[test]
    fn test_change_is_not_linewise_by_default() {
        let change = ChangeOperator;
        assert!(!change.is_linewise());
    }

    // ========================================================================
    // Change execute tests
    // ========================================================================

    #[test]
    fn test_change_buffer_not_found() {
        let ctx = create_test_context();
        let change = ChangeOperator;
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
        let result = change.execute(&mut op_ctx, range);
        assert!(result.is_err());
    }

    #[test]
    fn test_change_characterwise_single_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let change = ChangeOperator;
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
        // Change "hello" (columns 0..5) - deletes text, leaves buffer ready for insert
        let range = super::super::Range::new(Position::new(0, 0), Position::new(0, 5));
        let result = change.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        assert_eq!(buf.lines(), &[" world"]);

        drop(op_ctx);
        assert_eq!(registers.get().text, "hello");
    }

    #[test]
    fn test_change_characterwise_multi_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld\nfoo");
        let buffer_id = ctx.buffers.register(buffer);

        let change = ChangeOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Default,
            count: 1,
            cursor_position: Position::new(0, 3),
        };
        // Change from (0,3) to (1,3) => deletes "lo\nwor"
        let range = super::super::Range::new(Position::new(0, 3), Position::new(1, 3));
        let result = change.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get().text, "lo\nwor");
    }

    #[test]
    fn test_change_linewise_single_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let change = ChangeOperator;
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
        // cc on first line - should clear content but keep line for insertion
        let range = super::super::Range::linewise(Position::new(0, 0), Position::new(0, 0));
        let result = change.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get().text, "hello\n");
    }

    #[test]
    fn test_change_linewise_last_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let change = ChangeOperator;
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
        // cc on last line
        let range = super::super::Range::linewise(Position::new(1, 0), Position::new(1, 0));
        let result = change.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get().text, "world\n");
    }

    #[test]
    fn test_change_to_named_register() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let change = ChangeOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Slot('b'),
            count: 1,
            cursor_position: Position::new(0, 0),
        };
        let range = super::super::Range::new(Position::new(0, 0), Position::new(0, 5));
        let result = change.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get_named('b').map(|r| r.text.as_str()), Some("hello"));
    }

    #[test]
    fn test_change_characterwise_empty_range() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let change = ChangeOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Default,
            count: 1,
            cursor_position: Position::new(0, 3),
        };
        // Empty range
        let range = super::super::Range::new(Position::new(0, 3), Position::new(0, 3));
        let result = change.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        // Buffer should be unchanged for empty range
        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        assert_eq!(buf.lines(), &["hello"]);
    }

    #[test]
    fn test_change_linewise_clamped_end() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("line1\nline2");
        let buffer_id = ctx.buffers.register(buffer);

        let change = ChangeOperator;
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
        // End beyond buffer should be clamped
        let range = super::super::Range::linewise(Position::new(0, 0), Position::new(99, 0));
        let result = change.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get().text, "line1\nline2\n");
    }

    #[test]
    fn test_change_characterwise_three_lines() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("aaa\nbbb\nccc");
        let buffer_id = ctx.buffers.register(buffer);

        let change = ChangeOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Default,
            count: 1,
            cursor_position: Position::new(0, 1),
        };
        // Change from (0,1) to (2,2) => "aa\nbbb\ncc"
        let range = super::super::Range::new(Position::new(0, 1), Position::new(2, 2));
        let result = change.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get().text, "aa\nbbb\ncc");
    }

    #[test]
    fn test_change_characterwise_single_char() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let change = ChangeOperator;
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
        // Change just 'h' (0,0) to (0,1)
        let range = super::super::Range::new(Position::new(0, 0), Position::new(0, 1));
        let result = change.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        assert_eq!(buf.lines(), &["ello"]);

        drop(op_ctx);
        assert_eq!(registers.get().text, "h");
    }

    #[test]
    fn test_change_linewise_multiple_lines() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("a\nb\nc\nd");
        let buffer_id = ctx.buffers.register(buffer);

        let change = ChangeOperator;
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
        // Change lines 1-2 (linewise)
        let range = super::super::Range::linewise(Position::new(1, 0), Position::new(2, 0));
        let result = change.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get().text, "b\nc\n");
    }

    #[test]
    fn test_change_operator_clone() {
        let change = ChangeOperator;
        let cloned = change;
        assert_eq!(cloned.id(), "change");
    }

    #[test]
    fn test_change_operator_copy() {
        let change = ChangeOperator;
        let copied: ChangeOperator = change;
        assert_eq!(copied.id(), "change");
    }

    #[test]
    fn test_change_operator_debug() {
        let debug = format!("{:?}", ChangeOperator);
        assert!(debug.contains("ChangeOperator"));
    }

    #[test]
    fn test_change_characterwise_column_beyond_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hi");
        let buffer_id = ctx.buffers.register(buffer);

        let change = ChangeOperator;
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
        // End column beyond line - should be clamped
        let range = super::super::Range::new(Position::new(0, 0), Position::new(0, 100));
        let result = change.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get().text, "hi");
    }

    #[test]
    fn test_change_linewise_only_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("only");
        let buffer_id = ctx.buffers.register(buffer);

        let change = ChangeOperator;
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
        let result = change.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get().text, "only\n");
    }

    #[test]
    fn test_change_multiline_characterwise_two_lines() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let change = ChangeOperator;
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
        // Change from (0,2) to (1,3) => "llo\nwor"
        let range = super::super::Range::new(Position::new(0, 2), Position::new(1, 3));
        let result = change.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get().text, "llo\nwor");
    }

    #[test]
    fn test_change_to_register_z() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let change = ChangeOperator;
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
        let result = change.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get_named('z').map(|r| r.text.as_str()), Some("world"));
    }

    // ========================================================================
    // Undo recording tests (exercises UndoProviderRegistry paths)
    // ========================================================================

    use {
        reovim_driver_undo::{
            UndoKey, UndoPersistError, UndoProvider, UndoProviderRegistry, UndoRecord,
        },
        reovim_driver_vfs::VfsDriver,
        reovim_kernel::api::v1::{Edit, UndoResult, UndoTree},
    };

    struct MockUndoProvider {
        records: RwLock<Vec<UndoRecord>>,
    }

    impl MockUndoProvider {
        fn new() -> Self {
            Self {
                records: RwLock::new(Vec::new()),
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl UndoProvider for MockUndoProvider {
        fn undo(&self, _: BufferId) -> Option<UndoResult> {
            None
        }
        fn redo(&self, _: BufferId) -> Option<UndoResult> {
            None
        }
        fn redo_branch(&self, _: BufferId, _: usize) -> Option<UndoResult> {
            None
        }
        fn record(
            &self,
            buffer_id: BufferId,
            edits: Vec<Edit>,
            cursor_before: Position,
            cursor_after: Position,
        ) {
            self.records.write().push(UndoRecord {
                buffer_id,
                edits,
                cursor_before,
                cursor_after,
            });
        }
        fn has_history(&self, _: BufferId) -> bool {
            false
        }
        fn remove(&self, _: BufferId) {}
        fn buffer_count(&self) -> usize {
            0
        }
        fn get_tree(&self, _: BufferId) -> Option<UndoTree> {
            None
        }
        fn begin_batch(&self, _: BufferId, _: Position) {}
        fn end_batch(&self, _: BufferId, _: Position) {}
        fn is_batching(&self, _: BufferId) -> bool {
            false
        }
        fn persist(&self, _: BufferId, _: &str, _: &dyn VfsDriver) -> Result<(), UndoPersistError> {
            Ok(())
        }
        fn load(&self, _: BufferId, _: &str, _: &dyn VfsDriver) -> Result<bool, UndoPersistError> {
            Ok(false)
        }
    }

    fn create_test_context_with_undo() -> (KernelContext, Arc<MockUndoProvider>) {
        let services = Arc::new(ServiceRegistry::new());
        let mock_undo = Arc::new(MockUndoProvider::new());
        let undo_registry = Arc::new(UndoProviderRegistry::new());
        undo_registry.register(UndoKey::Buffer, mock_undo.clone() as Arc<dyn UndoProvider>);
        services.register(undo_registry);

        let ctx = KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(RwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::default()),
            services,
        );
        (ctx, mock_undo)
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_change_characterwise_records_undo() {
        let (ctx, mock_undo) = create_test_context_with_undo();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let change = ChangeOperator;
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
        let result = change.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        let records = mock_undo.records.read();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].buffer_id, buffer_id);
        if let Edit::Delete { position, text } = &records[0].edits[0] {
            assert_eq!(*position, Position::new(0, 0));
            assert_eq!(text, "hello");
        } else {
            panic!("Expected Delete edit");
        }
    }

    #[test]
    fn test_change_linewise_records_undo() {
        let (ctx, mock_undo) = create_test_context_with_undo();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let change = ChangeOperator;
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
        let result = change.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        let records = mock_undo.records.read();
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn test_change_empty_range_no_undo() {
        let (ctx, mock_undo) = create_test_context_with_undo();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let change = ChangeOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Default,
            count: 1,
            cursor_position: Position::new(0, 3),
        };
        // Empty range - no deleted text, so no undo recording
        let range = super::super::Range::new(Position::new(0, 3), Position::new(0, 3));
        let result = change.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        // No undo should be recorded for empty delete
        let records = mock_undo.records.read();
        assert_eq!(records.len(), 0);
    }

    #[test]
    fn test_change_linewise_fallback_branch() {
        // Test linewise change where lines.get(clamped_end) returns None
        // by using a buffer with empty content at the end line
        let (ctx, _mock_undo) = create_test_context_with_undo();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let change = ChangeOperator;
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
        // Linewise change of last line in buffer
        let range = super::super::Range::linewise(Position::new(1, 0), Position::new(1, 0));
        let result = change.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get().text, "world\n");
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_change_multiline_characterwise_records_undo() {
        let (ctx, mock_undo) = create_test_context_with_undo();
        let buffer = Buffer::from_string("aaa\nbbb\nccc");
        let buffer_id = ctx.buffers.register(buffer);

        let change = ChangeOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Default,
            count: 1,
            cursor_position: Position::new(0, 1),
        };
        let range = super::super::Range::new(Position::new(0, 1), Position::new(2, 2));
        let result = change.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        let records = mock_undo.records.read();
        assert_eq!(records.len(), 1);
        if let Edit::Delete { text, .. } = &records[0].edits[0] {
            assert_eq!(text, "aa\nbbb\ncc");
        } else {
            panic!("Expected Delete edit");
        }
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
    // Linewise change with multiple lines at end of buffer
    // ========================================================================

    #[test]
    fn test_change_linewise_all_lines_in_buffer() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("a\nb\nc");
        let buffer_id = ctx.buffers.register(buffer);

        let change = ChangeOperator;
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
        // Linewise change of all lines in buffer (Case 3: last line in buffer)
        let range = super::super::Range::linewise(Position::new(0, 0), Position::new(2, 0));
        let result = change.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        // All lines should be in register as linewise
        assert_eq!(registers.get().text, "a\nb\nc\n");
        assert!(registers.get().is_linewise());
    }

    #[test]
    fn test_change_characterwise_register_is_characterwise() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let change = ChangeOperator;
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
        change.execute(&mut op_ctx, range).unwrap();

        drop(op_ctx);
        assert!(registers.get().is_characterwise());
    }

    #[test]
    fn test_change_linewise_register_is_linewise() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let change = ChangeOperator;
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
        change.execute(&mut op_ctx, range).unwrap();

        drop(op_ctx);
        assert!(registers.get().is_linewise());
    }

    #[test]
    fn test_change_undo_records_cursor_positions() {
        let (ctx, mock_undo) = create_test_context_with_undo();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let change = ChangeOperator;
        let cursor_before = Position::new(0, 3);
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Default,
            count: 1,
            cursor_position: cursor_before,
        };
        let range = super::super::Range::new(Position::new(0, 3), Position::new(0, 8));
        change.execute(&mut op_ctx, range).unwrap();

        let records = mock_undo.records.read();
        assert_eq!(records.len(), 1);
        // cursor_before should be what we passed in
        assert_eq!(records[0].cursor_before, cursor_before);
        // cursor_after should be at delete_pos (start of range)
        assert_eq!(records[0].cursor_after, Position::new(0, 3));
    }
}
