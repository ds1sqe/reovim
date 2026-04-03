#![allow(
    clippy::significant_drop_tightening,
    clippy::uninlined_format_args,
    clippy::drop_non_drop
)]
use super::super::*;

use {
    reovim_driver_command::{CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{
        ClientId, ExtensionMap, Jumplist, Session, SessionRuntime, WindowLayout,
        testing::StubExecutor,
    },
    reovim_kernel::{
        api::{
            ModeStack,
            v1::{BufferId, CommandId, KernelContext, MarkBank, ModeId, ModuleId, RwLock},
        },
        testing::create_test_context,
    },
    reovim_provider_text::Buffer,
    reovim_types_text::{HistoryRing, Position, Register, RegisterBank},
    std::sync::Arc,
};

fn run_command<C: CommandHandler>(
    cmd: &C,
    ctx: &KernelContext,
    args: &CommandContext,
) -> CommandResult {
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
    let mut jumplist = Jumplist::new();
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        ctx,
        &executor,
    );
    cmd.execute(&mut runtime, args)
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
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
    assert_eq!(buf.content(), " world");

    // Check register has deleted text
    drop(op_ctx);
    assert_eq!(registers.get().text, "hello");
}

#[test]
fn test_delete_characterwise_partial() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
    assert_eq!(buf.content(), "hello ");

    drop(op_ctx);
    assert_eq!(registers.get().text, "world");
}

#[test]
fn test_delete_characterwise_multi_line() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld\nfoo");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
    assert_eq!(buf.content(), "line2\nline3");

    drop(op_ctx);
    assert_eq!(registers.get().text, "line1\n");
}

#[test]
fn test_delete_linewise_last_line_not_only() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("line1\nline2");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
    assert_eq!(buf.content(), "line1");

    drop(op_ctx);
    assert_eq!(registers.get().text, "line2\n");
}

#[test]
fn test_delete_linewise_only_line() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("only line");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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

    // After deleting all content, buffer is empty
    let buf = ctx.buffers.get(buffer_id).unwrap();
    let buf = buf.read();
    assert_eq!(buf.content(), "");

    drop(op_ctx);
    assert_eq!(registers.get().text, "only line\n");
}

#[test]
fn test_delete_linewise_multiple_lines() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("a\nb\nc\nd");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
    assert_eq!(buf.content(), "a\nd");

    drop(op_ctx);
    assert_eq!(registers.get().text, "b\nc\n");
}

#[test]
fn test_delete_to_named_register() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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

    // After deleting all lines, buffer is empty
    let buf = ctx.buffers.get(buffer_id).unwrap();
    let buf = buf.read();
    assert_eq!(buf.content(), "");
}

#[test]
fn test_delete_characterwise_empty_range() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
    assert_eq!(buf.content(), "hello");
}

#[test]
fn test_delete_linewise_last_two_of_three() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("a\nb\nc");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
    assert_eq!(buf.content(), "a");
}

#[test]
fn test_delete_characterwise_three_lines() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("aaa\nbbb\nccc");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
    assert_eq!(buf.content(), "first\nthird");

    drop(op_ctx);
    assert_eq!(registers.get().text, "second\n");
}

#[test]
fn test_delete_characterwise_single_char() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
    assert_eq!(buf.content(), "ello");

    drop(op_ctx);
    assert_eq!(registers.get().text, "h");
}

#[test]
fn test_delete_to_register_b() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
    assert_eq!(buf.content(), "");

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
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
    assert_eq!(buf.content(), "");

    drop(op_ctx);
    assert_eq!(registers.get().text, "hello");
}

#[test]
fn test_delete_characterwise_column_beyond_line() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hi");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
    assert_eq!(buf.content(), "");

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
    reovim_types_text::{Edit, UndoResult, UndoTree},
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
    let ctx = create_test_context();
    let mock_undo = Arc::new(MockUndoProvider::new());
    let undo_registry = Arc::new(UndoProviderRegistry::new());
    undo_registry.register(UndoKey::Buffer, mock_undo.clone() as Arc<dyn UndoProvider>);
    ctx.services.register(undo_registry);
    (ctx, mock_undo)
}

#[test]
fn test_delete_linewise_records_undo() {
    let (ctx, mock_undo) = create_test_context_with_undo();
    let buffer = Buffer::from_string("line1\nline2\nline3");
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
    let buffer_id = ctx.buffers.register(Arc::new(RwLock::new(buffer)));

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
