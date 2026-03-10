//! Delete operator.
//!
//! Reference: lib/core/src/command/builtin/operator.rs (concept-extraction, not migration)

use {
    reovim_driver_undo::{UndoKey, UndoProviderRegistry},
    reovim_kernel::api::v1::{Edit, RegisterContent},
};

use super::{Operator, OperatorContext, OperatorError, Range, registers};

/// Delete operator - cuts text to register.
///
/// Behavior:
/// - Deletes text in the given range
/// - Stores deleted text in the unnamed register (or specified register)
/// - Linewise if the motion was linewise
///
/// # Example
///
/// ```ignore
/// let delete = DeleteOperator;
/// delete.execute(&mut ctx, range)?;
/// ```
#[derive(Debug, Clone, Copy)]
pub struct DeleteOperator;

impl Operator for DeleteOperator {
    fn id(&self) -> &'static str {
        "delete"
    }

    #[allow(clippy::too_many_lines, clippy::option_if_let_else)]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, ctx: &mut OperatorContext<'_>, range: Range) -> Result<(), OperatorError> {
        // Get the buffer via kernel's buffer manager
        let buffer_arc = ctx
            .kernel
            .buffers
            .get(ctx.buffer_id)
            .ok_or(OperatorError::BufferNotFound(ctx.buffer_id))?;

        let mut buffer = buffer_arc.write();

        // Get text before deleting
        let start = range.start;
        let end = range.end;

        // Build deleted text from lines
        // - register_text: for register storage (linewise = "line\n")
        // - deleted_text: for undo (exact bytes deleted)
        let mut register_text = String::new();
        let mut deleted_text = String::new();
        let lines = buffer.lines();

        if range.is_linewise {
            // Linewise deletion: delete entire lines from start.line to end.line (inclusive)
            // Ignore column values - always delete full lines
            let line_count = lines.len();

            // Clamp end.line to last valid line to handle counts exceeding buffer
            let clamped_end = end.line.min(line_count.saturating_sub(1));

            // Build register_text as "line\n" for each line (for paste to work correctly)
            for line_idx in start.line..=clamped_end {
                if let Some(line) = lines.get(line_idx) {
                    register_text.push_str(line);
                    register_text.push('\n');
                }
            }

            // For linewise, adjust the actual deletion range to cover full lines.
            // Three cases based on Vim semantics:
            //
            // Case 1: Deleting non-last lines (e.g., dd on line 0 of 3-line buffer)
            //   - Delete from start of first line through the newline
            //   - Range: (start.line, 0) to (clamped_end + 1, 0)
            //   - Removes: "line\n" leaving subsequent lines
            //   - deleted_text: "line\n"
            //
            // Case 2: Deleting last line(s) but not all (e.g., dd on line 1 of 2-line buffer)
            //   - Include the PRECEDING newline from previous line
            //   - Range: (start.line - 1, prev_len) to (clamped_end, last_len)
            //   - Removes: "\nline" leaving previous lines intact
            //   - deleted_text: "\nline" (for correct undo)
            //
            // Case 3: Deleting the only line (single-line buffer)
            //   - Delete entire content
            //   - Range: (0, 0) to (0, content_len)
            //   - Results in empty buffer
            //   - deleted_text: "line"
            let delete_start;
            let delete_end;

            if clamped_end + 1 < line_count {
                // Case 1: Deleting non-last lines - delete through newline to next line
                delete_start = reovim_kernel::api::v1::Position::new(start.line, 0);
                delete_end = reovim_kernel::api::v1::Position::new(clamped_end + 1, 0);
                // deleted_text matches what we're deleting: "line\n"
                deleted_text.clone_from(&register_text);
            } else if start.line > 0 {
                // Case 2: Deleting last line(s) but not all
                // Include the preceding newline (from end of previous line)
                // Use .chars().count() for UTF-8 safety
                let prev_line_len = lines.get(start.line - 1).map_or(0, |l| l.chars().count());
                delete_start = reovim_kernel::api::v1::Position::new(start.line - 1, prev_line_len);
                // clamped_end is always valid: end.line.min(line_count - 1) < lines.len()
                let last_line = &lines[clamped_end];
                delete_end =
                    reovim_kernel::api::v1::Position::new(clamped_end, last_line.chars().count());
                // Build deleted_text as "\nline" (preceding newline + content, no trailing newline)
                // This matches what we're actually deleting for correct undo
                for line_idx in start.line..=clamped_end {
                    deleted_text.push('\n');
                    if let Some(line) = lines.get(line_idx) {
                        deleted_text.push_str(line);
                    }
                }
            } else {
                // Case 3: Deleting all lines (start.line == 0 and clamped_end is last line)
                delete_start = reovim_kernel::api::v1::Position::new(0, 0);
                // clamped_end is always valid: end.line.min(line_count - 1) < lines.len()
                let last_line = &lines[clamped_end];
                delete_end =
                    reovim_kernel::api::v1::Position::new(clamped_end, last_line.chars().count());
                // deleted_text is just the content (no newlines - single line)
                if let Some(line) = lines.get(clamped_end) {
                    deleted_text.push_str(line);
                }
            }

            // Track which case for cursor positioning
            let is_deleting_last_line = (clamped_end + 1 >= line_count) && start.line > 0;

            // Store in register as linewise (handles +/* via ClipboardProvider)
            let content = RegisterContent::linewise(register_text);
            registers::store_and_sync(ctx.kernel, ctx.registers, ctx.register, &content);
            registers::push_to_history(ctx.clipboard_history, &content);

            // Use cursor position from context (passed from caller who has window access)
            let cursor_before = ctx.cursor_position;

            // Delete entire lines
            buffer.delete_range(delete_start, delete_end);

            // Cursor positioning after linewise delete follows Vim behavior:
            //
            // Case 1 (delete non-last lines): Cursor at column 0 of the line that
            //   takes the place of the deleted lines (i.e., the first remaining line).
            //
            // Case 2 (delete last lines but not all): Cursor at the last valid column
            //   of the new last line, since there's no line below to move to.
            //
            // Case 3 (delete all lines): Buffer is empty, cursor at (0, 0).
            let line_count = buffer.line_count();
            let final_line = start.line.min(line_count.saturating_sub(1));
            // Note: Buffer always maintains at least one line (even if empty),
            // so line_count is always >= 1 after delete_range.
            let final_col = if is_deleting_last_line {
                // Case 2: Cursor at last valid column of the new last line
                let line_len = buffer.line_len(final_line).unwrap_or(0);
                if line_len == 0 {
                    0
                } else {
                    line_len.saturating_sub(1)
                }
            } else {
                // Case 1: Cursor at column 0
                0
            };
            // Cursor position after delete — used for both undo tracking and
            // communicating desired cursor back to execute_operator (#552)
            let cursor_after = reovim_kernel::api::v1::Position::new(final_line, final_col);
            ctx.cursor_after = Some(cursor_after);

            // Record edit for undo
            if let Some(undo_registry) = ctx.kernel.services.get::<UndoProviderRegistry>()
                && let Some(undo_provider) = undo_registry.get(&UndoKey::Buffer)
            {
                let edit = Edit::Delete {
                    position: delete_start,
                    text: deleted_text.clone(),
                };
                undo_provider.record(ctx.buffer_id, vec![edit], cursor_before, cursor_after);
            }
        } else {
            // Characterwise deletion
            if start.line == end.line {
                // Single line deletion
                // start.line is valid: buffer exists and lines were just obtained from it
                let line = &lines[start.line];
                let start_col = start.column.min(line.len());
                let end_col = end.column.min(line.len());
                if start_col < end_col {
                    deleted_text.push_str(&line[start_col..end_col]);
                }
            } else {
                // Multi-line deletion
                // All indices in start.line..=end.line are valid: lines were obtained
                // from the same buffer snapshot and end.line <= last valid line
                for (line_idx, line) in lines.iter().enumerate().take(end.line + 1).skip(start.line)
                {
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
            }

            // Store in register as characterwise (handles +/* via ClipboardProvider)
            let content = RegisterContent::characterwise(deleted_text.clone());
            registers::store_and_sync(ctx.kernel, ctx.registers, ctx.register, &content);
            registers::push_to_history(ctx.clipboard_history, &content);

            // Use cursor position from context (passed from caller who has window access)
            let cursor_before = ctx.cursor_position;

            // Delete the text from buffer
            buffer.delete_range(start, end);

            // Cursor after characterwise delete is at the start position (#552)
            let cursor_after = start;
            ctx.cursor_after = Some(cursor_after);

            // Record edit for undo
            if let Some(undo_registry) = ctx.kernel.services.get::<UndoProviderRegistry>()
                && let Some(undo_provider) = undo_registry.get(&UndoKey::Buffer)
            {
                let edit = Edit::Delete {
                    position: start,
                    text: deleted_text,
                };
                undo_provider.record(ctx.buffer_id, vec![edit], cursor_before, cursor_after);
            }
        }

        drop(buffer);
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
            ClientId, ExtensionMap, Session, SessionRuntime, WindowLayout,
            api::{CommandExecutor, CommandHandle},
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
            fn get_handle(&self, _id: &CommandId) -> Option<std::sync::Arc<dyn CommandHandle>> {
                None
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
    fn test_delete_operator_id() {
        let delete = DeleteOperator;
        assert_eq!(delete.id(), "delete");
    }

    #[test]
    fn test_delete_is_text_modifying() {
        let delete = DeleteOperator;
        assert!(delete.is_text_modifying());
    }

    #[test]
    fn test_delete_is_not_linewise_by_default() {
        let delete = DeleteOperator;
        assert!(!delete.is_linewise());
    }

    // ========================================================================
    // Delete execute tests
    // ========================================================================

    #[test]
    fn test_delete_buffer_not_found() {
        let ctx = create_test_context();
        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        let range = super::super::Range::new(Position::new(0, 0), Position::new(0, 5));
        let result = delete.execute(&mut op_ctx, range);
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_characterwise_single_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        // Delete "hello" (columns 0..5)
        let range = super::super::Range::new(Position::new(0, 0), Position::new(0, 5));
        let result = delete.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        // Check buffer content
        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        assert_eq!(buf.lines(), &[" world"]);

        // Check register has deleted text
        drop(op_ctx);
        assert_eq!(registers.get().text, "hello");
    }

    #[test]
    fn test_delete_characterwise_partial() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut op_ctx = OperatorContext {
            kernel: &ctx,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            buffer_id,
            register: Register::Default,
            count: 1,
            cursor_position: Position::new(0, 6),
            cursor_after: None,
        };
        // Delete "world" (columns 6..11)
        let range = super::super::Range::new(Position::new(0, 6), Position::new(0, 11));
        let result = delete.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        assert_eq!(buf.lines(), &["hello "]);

        drop(op_ctx);
        assert_eq!(registers.get().text, "world");
    }

    #[test]
    fn test_delete_characterwise_multi_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld\nfoo");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        // Delete from (0,3) to (1,3) => "lo\nwor"
        let range = super::super::Range::new(Position::new(0, 3), Position::new(1, 3));
        let result = delete.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get().text, "lo\nwor");
    }

    #[test]
    fn test_delete_linewise_first_line_of_multiple() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("line1\nline2\nline3");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        // Delete line 0 (linewise) - Case 1: deleting non-last line
        let range = super::super::Range::linewise(Position::new(0, 0), Position::new(0, 0));
        let result = delete.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        assert_eq!(buf.lines(), &["line2", "line3"]);

        drop(op_ctx);
        assert_eq!(registers.get().text, "line1\n");
    }

    #[test]
    fn test_delete_linewise_last_line_not_only() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("line1\nline2");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        // Delete line 1 (linewise) - Case 2: deleting last line but not all
        let range = super::super::Range::linewise(Position::new(1, 0), Position::new(1, 0));
        let result = delete.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        assert_eq!(buf.lines(), &["line1"]);

        drop(op_ctx);
        assert_eq!(registers.get().text, "line2\n");
    }

    #[test]
    fn test_delete_linewise_only_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("only line");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        // Delete line 0 (linewise) - Case 3: deleting the only line
        let range = super::super::Range::linewise(Position::new(0, 0), Position::new(0, 0));
        let result = delete.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        // After deleting all content, buffer has one empty line
        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        assert_eq!(buf.lines(), &[""]);

        drop(op_ctx);
        assert_eq!(registers.get().text, "only line\n");
    }

    #[test]
    fn test_delete_linewise_multiple_lines() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("a\nb\nc\nd");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        // Delete lines 1-2 (linewise) - Case 1: not last lines
        let range = super::super::Range::linewise(Position::new(1, 0), Position::new(2, 0));
        let result = delete.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        assert_eq!(buf.lines(), &["a", "d"]);

        drop(op_ctx);
        assert_eq!(registers.get().text, "b\nc\n");
    }

    #[test]
    fn test_delete_to_named_register() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        let range = super::super::Range::new(Position::new(0, 0), Position::new(0, 5));
        let result = delete.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get_named('a').map(|r| r.text.as_str()), Some("hello"));
    }

    #[test]
    fn test_delete_linewise_clamped_end() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("a\nb");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        // Delete with end beyond buffer - should clamp to last line
        // This is Case 3: start.line == 0, clamped_end is last line
        let range = super::super::Range::linewise(Position::new(0, 0), Position::new(99, 0));
        let result = delete.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        // After deleting all lines, buffer has one empty line
        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        assert_eq!(buf.lines(), &[""]);
    }

    #[test]
    fn test_delete_characterwise_empty_range() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        // Delete same position
        let range = super::super::Range::new(Position::new(0, 3), Position::new(0, 3));
        let result = delete.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        // Buffer should be unchanged
        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        assert_eq!(buf.lines(), &["hello"]);
    }

    #[test]
    fn test_delete_linewise_last_two_of_three() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("a\nb\nc");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        // Delete lines 1-2 (the last two lines) - Case 2
        let range = super::super::Range::linewise(Position::new(1, 0), Position::new(2, 0));
        let result = delete.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        assert_eq!(buf.lines(), &["a"]);
    }

    #[test]
    fn test_delete_characterwise_three_lines() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("aaa\nbbb\nccc");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        // Delete from (0,1) to (2,2) => "aa\nbbb\ncc"
        let range = super::super::Range::new(Position::new(0, 1), Position::new(2, 2));
        let result = delete.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get().text, "aa\nbbb\ncc");
    }

    #[test]
    fn test_delete_linewise_middle_line_of_three() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("first\nsecond\nthird");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        // Delete middle line (linewise) - Case 1: not last line
        let range = super::super::Range::linewise(Position::new(1, 0), Position::new(1, 0));
        let result = delete.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        assert_eq!(buf.lines(), &["first", "third"]);

        drop(op_ctx);
        assert_eq!(registers.get().text, "second\n");
    }

    #[test]
    fn test_delete_characterwise_single_char() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        // Delete just 'h' (0,0) to (0,1)
        let range = super::super::Range::new(Position::new(0, 0), Position::new(0, 1));
        let result = delete.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        assert_eq!(buf.lines(), &["ello"]);

        drop(op_ctx);
        assert_eq!(registers.get().text, "h");
    }

    #[test]
    fn test_delete_to_register_b() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        let range = super::super::Range::new(Position::new(0, 0), Position::new(0, 5));
        let result = delete.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        drop(op_ctx);
        assert_eq!(registers.get_named('b').map(|r| r.text.as_str()), Some("hello"));
    }

    #[test]
    fn test_delete_linewise_all_lines_three() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("a\nb\nc");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        // Delete all lines
        let range = super::super::Range::linewise(Position::new(0, 0), Position::new(2, 0));
        let result = delete.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        assert_eq!(buf.lines(), &[""]);

        drop(op_ctx);
        assert_eq!(registers.get().text, "a\nb\nc\n");
    }

    #[test]
    fn test_delete_operator_clone() {
        let delete = DeleteOperator;
        let cloned = delete;
        assert_eq!(cloned.id(), "delete");
    }

    #[test]
    fn test_delete_operator_copy() {
        let delete = DeleteOperator;
        let copied: DeleteOperator = delete;
        assert_eq!(copied.id(), "delete");
    }

    #[test]
    fn test_delete_operator_debug() {
        let debug = format!("{:?}", DeleteOperator);
        assert!(debug.contains("DeleteOperator"));
    }

    #[test]
    fn test_delete_characterwise_entire_line_content() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        // Delete entire line content characterwise
        let range = super::super::Range::new(Position::new(0, 0), Position::new(0, 5));
        let result = delete.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        assert_eq!(buf.lines(), &[""]);

        drop(op_ctx);
        assert_eq!(registers.get().text, "hello");
    }

    #[test]
    fn test_delete_characterwise_column_beyond_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hi");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        // End column beyond line length - should be clamped
        let range = super::super::Range::new(Position::new(0, 0), Position::new(0, 100));
        let result = delete.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        assert_eq!(buf.lines(), &[""]);

        drop(op_ctx);
        assert_eq!(registers.get().text, "hi");
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
    fn test_delete_linewise_records_undo() {
        let (ctx, mock_undo) = create_test_context_with_undo();
        let buffer = Buffer::from_string("line1\nline2\nline3");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        let range = super::super::Range::linewise(Position::new(0, 0), Position::new(0, 0));
        let result = delete.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        let records = mock_undo.records.read();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].buffer_id, buffer_id);
        // Should be a Delete edit
        assert!(matches!(&records[0].edits[0], Edit::Delete { .. }));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_delete_characterwise_records_undo() {
        let (ctx, mock_undo) = create_test_context_with_undo();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        let range = super::super::Range::new(Position::new(0, 0), Position::new(0, 5));
        let result = delete.execute(&mut op_ctx, range);
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
    fn test_delete_linewise_last_line_empty_after_delete() {
        // Tests Case 2 cursor positioning when line_len == 0
        let (ctx, _mock_undo) = create_test_context_with_undo();
        let buffer = Buffer::from_string("\n");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        // Delete line 1 (last line, which is empty) - Case 2
        let range = super::super::Range::linewise(Position::new(1, 0), Position::new(1, 0));
        let result = delete.execute(&mut op_ctx, range);
        assert!(result.is_ok());
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_delete_linewise_case2_records_undo_with_newline_prefix() {
        let (ctx, mock_undo) = create_test_context_with_undo();
        let buffer = Buffer::from_string("first\nsecond");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        // Delete last line - Case 2 (not all lines, but last line)
        let range = super::super::Range::linewise(Position::new(1, 0), Position::new(1, 0));
        let result = delete.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        let records = mock_undo.records.read();
        assert_eq!(records.len(), 1);
        if let Edit::Delete { text, .. } = &records[0].edits[0] {
            // Deleted text should be "\nsecond" (preceding newline + content)
            assert_eq!(text, "\nsecond");
        } else {
            panic!("Expected Delete edit");
        }
    }

    #[test]
    fn test_delete_run_command_helper_with_noop() {
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

    #[test]
    fn test_delete_characterwise_register_is_characterwise() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        let range = super::super::Range::new(Position::new(0, 0), Position::new(0, 5));
        delete.execute(&mut op_ctx, range).unwrap();

        drop(op_ctx);
        assert!(registers.get().is_characterwise());
    }

    #[test]
    fn test_delete_linewise_register_is_linewise() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        let range = super::super::Range::linewise(Position::new(0, 0), Position::new(0, 0));
        delete.execute(&mut op_ctx, range).unwrap();

        drop(op_ctx);
        assert!(registers.get().is_linewise());
    }

    #[test]
    fn test_delete_undo_records_cursor_positions() {
        let (ctx, mock_undo) = create_test_context_with_undo();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        let range = super::super::Range::new(Position::new(0, 3), Position::new(0, 8));
        delete.execute(&mut op_ctx, range).unwrap();

        let records = mock_undo.records.read();
        assert_eq!(records.len(), 1);
        // cursor_before should be what we passed in
        assert_eq!(records[0].cursor_before, cursor_before);
        // cursor_after should be at start of range for characterwise delete
        assert_eq!(records[0].cursor_after, Position::new(0, 3));
    }

    #[test]
    fn test_delete_linewise_undo_records_cursor_positions() {
        let (ctx, mock_undo) = create_test_context_with_undo();
        let buffer = Buffer::from_string("line1\nline2\nline3");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
        let cursor_before = Position::new(1, 2);
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
            cursor_after: None,
        };
        // Delete middle line (linewise, Case 1: non-last line)
        let range = super::super::Range::linewise(Position::new(1, 0), Position::new(1, 0));
        delete.execute(&mut op_ctx, range).unwrap();

        let records = mock_undo.records.read();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].cursor_before, cursor_before);
        // cursor_after for Case 1: final_line=1, final_col=0 (column 0 for non-last line delete)
        assert_eq!(records[0].cursor_after, Position::new(1, 0));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_delete_characterwise_multiline_records_undo() {
        let (ctx, mock_undo) = create_test_context_with_undo();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        let range = super::super::Range::new(Position::new(0, 3), Position::new(1, 3));
        let result = delete.execute(&mut op_ctx, range);
        assert!(result.is_ok());

        let records = mock_undo.records.read();
        assert_eq!(records.len(), 1);
        if let Edit::Delete { text, .. } = &records[0].edits[0] {
            assert_eq!(text, "lo\nwor");
        } else {
            panic!("Expected Delete edit");
        }
    }

    // ========================================================================
    // cursor_after tests (#552)
    // ========================================================================

    #[test]
    fn test_delete_linewise_sets_cursor_after_case1() {
        // Case 1: delete non-last line → cursor at (start.line, 0)
        let ctx = create_test_context();
        let buffer = Buffer::from_string("aaa\nbbb\nccc");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        let range = super::super::Range::linewise(Position::new(0, 0), Position::new(0, 0));
        let result = delete.execute(&mut op_ctx, range);
        assert!(result.is_ok());
        assert_eq!(op_ctx.cursor_after, Some(Position::new(0, 0)));
    }

    #[test]
    fn test_delete_linewise_sets_cursor_after_case2() {
        // Case 2: delete last line(s) but not all → cursor at last valid line
        let ctx = create_test_context();
        let buffer = Buffer::from_string("aaa\nbbb");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        // Delete line 1 (last line)
        let range = super::super::Range::linewise(Position::new(1, 0), Position::new(1, 0));
        let result = delete.execute(&mut op_ctx, range);
        assert!(result.is_ok());
        // After delete, only "aaa" remains (line 0, len 3). Cursor at (0, 2) — last valid col
        assert_eq!(op_ctx.cursor_after, Some(Position::new(0, 2)));
    }

    #[test]
    fn test_delete_linewise_sets_cursor_after_case3() {
        // Case 3: delete all lines → cursor at (0, 0)
        let ctx = create_test_context();
        let buffer = Buffer::from_string("only");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        let range = super::super::Range::linewise(Position::new(0, 0), Position::new(0, 0));
        let result = delete.execute(&mut op_ctx, range);
        assert!(result.is_ok());
        assert_eq!(op_ctx.cursor_after, Some(Position::new(0, 0)));
    }

    #[test]
    fn test_delete_characterwise_sets_cursor_after() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let delete = DeleteOperator;
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
            cursor_after: None,
        };
        // Delete "hello" (0,0)-(0,5)
        let range = super::super::Range::new(Position::new(0, 0), Position::new(0, 5));
        let result = delete.execute(&mut op_ctx, range);
        assert!(result.is_ok());
        // Characterwise: cursor at start of deleted range
        assert_eq!(op_ctx.cursor_after, Some(Position::new(0, 0)));
    }
}
