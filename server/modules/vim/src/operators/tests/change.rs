#![allow(
clippy::significant_drop_tightening,
clippy::uninlined_format_args,
clippy::drop_non_drop
)]
use super::super::*;

use {
    reovim_kernel::testing::create_test_context,
    reovim_driver_command::{CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{
        testing::StubExecutor,
        ClientId, ExtensionMap, Session, SessionRuntime, WindowLayout,
    },
    reovim_kernel::api::{
        ModeStack,
        v1::{
            Buffer, BufferId, CommandId, HistoryRing,
            KernelContext, MarkBank, ModeId, ModuleId, Position,
            Register, RegisterBank, RwLock,
        },
    },
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
        cursor_after: None,
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
        cursor_after: None,
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
        cursor_after: None,
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
        cursor_after: None,
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
        cursor_after: None,
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
        cursor_after: None,
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
        cursor_after: None,
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
        cursor_after: None,
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
        cursor_after: None,
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
        cursor_after: None,
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
        cursor_after: None,
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
        cursor_after: None,
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
        cursor_after: None,
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
        cursor_after: None,
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
        cursor_after: None,
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
    let ctx = create_test_context();
    let mock_undo = Arc::new(MockUndoProvider::new());
    let undo_registry = Arc::new(UndoProviderRegistry::new());
    undo_registry.register(UndoKey::Buffer, mock_undo.clone() as Arc<dyn UndoProvider>);
    ctx.services.register(undo_registry);
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
        cursor_after: None,
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
        cursor_after: None,
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
        cursor_after: None,
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
        cursor_after: None,
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
        cursor_after: None,
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
        cursor_after: None,
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
        cursor_after: None,
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
        cursor_after: None,
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
        cursor_after: None,
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

// ========================================================================
// cursor_after tests (#552)
// ========================================================================

#[test]
fn test_change_sets_cursor_after() {
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
        cursor_after: None,
    };
    let range = super::super::Range::new(Position::new(0, 0), Position::new(0, 5));
    let result = change.execute(&mut op_ctx, range);
    assert!(result.is_ok());
    // cursor_after = delete_pos = start of range
    assert_eq!(op_ctx.cursor_after, Some(Position::new(0, 0)));
}

