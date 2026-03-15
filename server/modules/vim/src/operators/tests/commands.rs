#![allow(clippy::significant_drop_tightening, clippy::uninlined_format_args)]
use {
    super::super::*,
    reovim_driver_command::{ArgValue, Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::SessionRuntime,
    reovim_kernel::api::v1::Position,
};

use {
    reovim_driver_session::{ClientId, ExtensionMap, Session, WindowLayout},
    reovim_kernel::{
        api::{
            ModeStack,
            v1::{
                Buffer, BufferId, HistoryRing, Jumplist, KernelContext, MarkBank, ModeId, ModuleId,
                RegisterBank, RwLock,
            },
        },
        testing::create_test_context,
    },
    std::sync::Arc,
};

use reovim_driver_session::{api::ModeApi, testing::StubExecutor};

struct TestState {
    session: Session,
    mode_stack: ModeStack,
    windows: WindowLayout,
    extensions: ExtensionMap,
    compositor: Option<Box<dyn reovim_driver_layout::RootCompositor>>,
    tabs: reovim_driver_session::TabPageSet,
    registers: RegisterBank,
    clipboard_history: HistoryRing,
    local_marks: MarkBank,
    jumplist: Jumplist,
    active_buffer: Option<BufferId>,
    terminal_size: (u16, u16),
}

impl TestState {
    fn with_buffer(buffer_id: Option<BufferId>) -> Self {
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let session = Session::new(ClientId::new(1), home_mode.clone());
        let mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let extensions = ExtensionMap::new();

        let mut window = reovim_driver_session::Window::new();
        if let Some(buffer_id) = buffer_id {
            window.buffer_id = Some(buffer_id);
        }
        windows.add(window);

        Self {
            session,
            mode_stack,
            windows,
            extensions,
            compositor: None,
            tabs: reovim_driver_session::TabPageSet::new(),
            registers: RegisterBank::new(),
            clipboard_history: HistoryRing::new(),
            local_marks: MarkBank::new(),
            jumplist: Jumplist::new(),
            active_buffer: None,
            terminal_size: (80, 24),
        }
    }

    fn runtime<'a>(&'a mut self, kernel: &'a KernelContext) -> SessionRuntime<'a> {
        SessionRuntime::new(
            &mut self.session,
            reovim_driver_session::ClientContext {
                mode_stack: &mut self.mode_stack,
                windows: &mut self.windows,
                extensions: &mut self.extensions,
                compositor: &mut self.compositor,
                tabs: &mut self.tabs,
                registers: &mut self.registers,
                clipboard_history: &mut self.clipboard_history,
                local_marks: &mut self.local_marks,
                jumplist: &mut self.jumplist,
                active_buffer: &mut self.active_buffer,
                terminal_size: &mut self.terminal_size,
            },
            kernel,
            &StubExecutor,
        )
    }
}

// ========================================================================
// Metadata tests
// ========================================================================

#[test]
fn test_delete_command_id() {
    let cmd = DeleteCommand;
    assert_eq!(cmd.id().module().as_str(), "vim");
    assert_eq!(cmd.id().name(), "delete");
}

#[test]
fn test_yank_command_id() {
    let cmd = YankCommand;
    assert_eq!(cmd.id().module().as_str(), "vim");
    assert_eq!(cmd.id().name(), "yank");
}

#[test]
fn test_change_command_id() {
    let cmd = ChangeCommand;
    assert_eq!(cmd.id().module().as_str(), "vim");
    assert_eq!(cmd.id().name(), "change");
}

#[test]
fn test_operator_commands_count() {
    let cmds = operator_commands();
    assert_eq!(cmds.len(), 3);
}

#[test]
fn test_delete_command_description() {
    let cmd = DeleteCommand;
    assert!(cmd.description().contains("Delete"));
}

#[test]
fn test_yank_command_description() {
    let cmd = YankCommand;
    assert!(cmd.description().contains("Yank"));
}

#[test]
fn test_change_command_description() {
    let cmd = ChangeCommand;
    assert!(cmd.description().contains("Change"));
}

#[test]
fn test_delete_command_debug() {
    let debug = format!("{:?}", DeleteCommand);
    assert!(debug.contains("DeleteCommand"));
}

#[test]
fn test_yank_command_debug() {
    let debug = format!("{:?}", YankCommand);
    assert!(debug.contains("YankCommand"));
}

#[test]
fn test_change_command_debug() {
    let debug = format!("{:?}", ChangeCommand);
    assert!(debug.contains("ChangeCommand"));
}

#[test]
fn test_all_operator_commands_default() {
    let _ = DeleteCommand;
    let _ = YankCommand;
    let _ = ChangeCommand;
}

#[test]
fn test_operator_commands_have_correct_ids() {
    let cmds = operator_commands();
    assert_eq!(cmds.len(), 3);
}

// ========================================================================
// Execute tests for operator commands
// ========================================================================

#[test]
fn test_delete_command_no_buffer() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);
    let result = DeleteCommand.execute(&mut runtime, &args);
    assert!(matches!(result, CommandResult::Error(_)));
}

#[test]
fn test_delete_command_execute() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("range_start", ArgValue::Position(0, 0));
    args.set("range_end", ArgValue::Position(0, 5));

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = DeleteCommand.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Check buffer was modified
    let buf = ctx.buffers.get(buffer_id).unwrap();
    let buf = buf.read();
    assert_eq!(buf.lines(), &[" world"]);
}

#[test]
fn test_yank_command_no_buffer() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);
    let result = YankCommand.execute(&mut runtime, &args);
    assert!(matches!(result, CommandResult::Error(_)));
}

#[test]
fn test_yank_command_execute() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("range_start", ArgValue::Position(0, 0));
    args.set("range_end", ArgValue::Position(0, 5));

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = YankCommand.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Buffer should be unchanged (yank does not modify)
    let buf = ctx.buffers.get(buffer_id).unwrap();
    let buf = buf.read();
    assert_eq!(buf.lines(), &["hello world"]);

    // Register should have yanked text
    drop(runtime);
    assert_eq!(state.registers.get().text, "hello");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_yank_command_restores_cursor() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("range_start", ArgValue::Position(0, 0));
    args.set("range_end", ArgValue::Position(0, 5));

    let mut state = TestState::with_buffer(Some(buffer_id));
    // Set cursor at col 3
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(0, 3).into();
    }
    let mut runtime = state.runtime(&ctx);
    let result = YankCommand.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // After yank, cursor should be restored to range start (0, 0)
    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 0);
}

#[test]
fn test_change_command_no_buffer() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);
    let result = ChangeCommand.execute(&mut runtime, &args);
    assert!(matches!(result, CommandResult::Error(_)));
}

#[test]
fn test_change_command_execute() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("range_start", ArgValue::Position(0, 0));
    args.set("range_end", ArgValue::Position(0, 5));

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = ChangeCommand.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Should enter insert mode
    assert_eq!(runtime.current_mode().name(), crate::modes::VimMode::INSERT_ID.name());

    // Buffer should have text deleted
    let buf = ctx.buffers.get(buffer_id).unwrap();
    let buf = buf.read();
    assert_eq!(buf.lines(), &[" world"]);
}

#[test]
fn test_delete_command_linewise() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("line1\nline2\nline3");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("range_start", ArgValue::Position(0, 0));
    args.set("range_end", ArgValue::Position(0, 0));
    args.set("linewise", ArgValue::Bool(true));

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = DeleteCommand.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let buf = ctx.buffers.get(buffer_id).unwrap();
    let buf = buf.read();
    assert_eq!(buf.lines(), &["line2", "line3"]);
}

#[test]
fn test_yank_command_linewise() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("line1\nline2\nline3");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("range_start", ArgValue::Position(0, 0));
    args.set("range_end", ArgValue::Position(0, 0));
    args.set("linewise", ArgValue::Bool(true));

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = YankCommand.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Buffer should be unchanged
    let buf = ctx.buffers.get(buffer_id).unwrap();
    let buf = buf.read();
    assert_eq!(buf.lines(), &["line1", "line2", "line3"]);

    // Register should contain yanked line
    drop(runtime);
    assert_eq!(state.registers.get().text, "line1\n");
}

#[test]
fn test_change_command_linewise() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("line1\nline2\nline3");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("range_start", ArgValue::Position(0, 0));
    args.set("range_end", ArgValue::Position(0, 0));
    args.set("linewise", ArgValue::Bool(true));

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = ChangeCommand.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Should enter insert mode
    assert_eq!(runtime.current_mode().name(), crate::modes::VimMode::INSERT_ID.name());
}

#[test]
fn test_delete_command_with_register() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("range_start", ArgValue::Position(0, 0));
    args.set("range_end", ArgValue::Position(0, 5));
    args.set("register", ArgValue::Register('a'));

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = DeleteCommand.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    drop(runtime);
    assert_eq!(state.registers.get_named('a').map(|r| r.text.as_str()), Some("hello"));
}

#[test]
fn test_delete_command_cursor_after_execute() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("range_start", ArgValue::Position(0, 3));
    args.set("range_end", ArgValue::Position(0, 8));

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = DeleteCommand.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Cursor should be at the start of the deleted range
    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 3);
}

#[test]
fn test_change_command_cursor_at_start() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("range_start", ArgValue::Position(0, 2));
    args.set("range_end", ArgValue::Position(0, 7));

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = ChangeCommand.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 2);
}

#[test]
fn test_operator_commands_clone() {
    let del = DeleteCommand;
    let _: DeleteCommand = del;

    let yank = YankCommand;
    let _: YankCommand = yank;

    let change = ChangeCommand;
    let _: ChangeCommand = change;
}

#[test]
fn test_yank_command_no_range_defaults() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    // No range set - should default to (0,0) to (0,0) which is an empty range

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = YankCommand.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

// ========================================================================
// ChangeCommand undo batching tests
// ========================================================================

use {
    reovim_driver_undo::{UndoKey, UndoPersistError, UndoProvider, UndoProviderRegistry},
    reovim_driver_vfs::VfsDriver,
    reovim_kernel::api::v1::{Edit, UndoResult, UndoTree},
};

struct MockUndoProvider {
    batch_begins: RwLock<Vec<(BufferId, Position)>>,
    records: RwLock<Vec<BufferId>>,
}

impl MockUndoProvider {
    fn new() -> Self {
        Self {
            batch_begins: RwLock::new(Vec::new()),
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
    fn record(&self, buffer_id: BufferId, _: Vec<Edit>, _: Position, _: Position) {
        self.records.write().push(buffer_id);
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
    fn begin_batch(&self, buffer_id: BufferId, cursor_before: Position) {
        self.batch_begins.write().push((buffer_id, cursor_before));
    }
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
fn test_change_command_calls_begin_batch() {
    let (ctx, mock_undo) = create_test_context_with_undo();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("range_start", ArgValue::Position(0, 0));
    args.set("range_end", ArgValue::Position(0, 5));

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = ChangeCommand.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // begin_batch should have been called before the delete
    let begins = mock_undo.batch_begins.read();
    assert_eq!(begins.len(), 1);
    assert_eq!(begins[0].0, buffer_id);
}

#[test]
fn test_change_command_enters_insert_mode_with_undo() {
    let (ctx, _mock_undo) = create_test_context_with_undo();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("range_start", ArgValue::Position(0, 0));
    args.set("range_end", ArgValue::Position(0, 3));

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = ChangeCommand.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    assert_eq!(runtime.current_mode().name(), crate::modes::VimMode::INSERT_ID.name());
}

#[test]
fn test_change_command_records_undo_for_delete() {
    let (ctx, mock_undo) = create_test_context_with_undo();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("range_start", ArgValue::Position(0, 0));
    args.set("range_end", ArgValue::Position(0, 5));

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    ChangeCommand.execute(&mut runtime, &args);

    // The ChangeOperator should have recorded an undo entry
    let records = mock_undo.records.read();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0], buffer_id);
}

// ========================================================================
// Additional execute_operator tests - edge cases
// ========================================================================

#[test]
fn test_delete_command_with_count() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("range_start", ArgValue::Position(0, 0));
    args.set("range_end", ArgValue::Position(0, 5));
    args.set("count", ArgValue::Count(2));

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = DeleteCommand.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

#[test]
fn test_yank_command_with_register_and_count() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("range_start", ArgValue::Position(0, 0));
    args.set("range_end", ArgValue::Position(0, 5));
    args.set("register", ArgValue::Register('z'));
    args.set("count", ArgValue::Count(3));

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = YankCommand.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    drop(runtime);
    assert_eq!(state.registers.get_named('z').map(|r| r.text.as_str()), Some("hello"));
}

#[test]
fn test_execute_operator_no_active_window_uses_origin() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("range_start", ArgValue::Position(0, 0));
    args.set("range_end", ArgValue::Position(0, 5));

    // Create state with no windows - execute_operator should use Position::origin()
    let home_mode = reovim_kernel::api::v1::ModeId::new(
        reovim_kernel::api::v1::ModuleId::new("test"),
        "normal",
    );
    let mut session = Session::new(ClientId::new(1), home_mode.clone());
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

    // Add a window without buffer to have an active window
    let window = reovim_driver_session::Window::new();
    windows.add(window);

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
        &ctx,
        &StubExecutor,
    );

    // Execute with window that has no buffer_id but command has buffer_id
    let result = DeleteCommand.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

#[test]
fn test_change_command_no_buffer_skips_undo_batching() {
    let (ctx, mock_undo) = create_test_context_with_undo();
    let args = CommandContext::new(); // No buffer_id

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);
    let result = ChangeCommand.execute(&mut runtime, &args);
    // Should error due to no buffer
    assert!(matches!(result, CommandResult::Error(_)));

    // No undo batching should have occurred
    let begins = mock_undo.batch_begins.read();
    assert_eq!(begins.len(), 0);
}

#[test]
fn test_yank_command_no_window_cursor_defaults() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("range_start", ArgValue::Position(0, 2));
    args.set("range_end", ArgValue::Position(0, 7));

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = YankCommand.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Cursor should be restored to range_start
    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 2);
}

#[test]
fn test_change_command_linewise_enters_insert_mode() {
    let (ctx, mock_undo) = create_test_context_with_undo();
    let buffer = Buffer::from_string("line1\nline2\nline3");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("range_start", ArgValue::Position(1, 0));
    args.set("range_end", ArgValue::Position(1, 0));
    args.set("linewise", ArgValue::Bool(true));

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = ChangeCommand.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    assert_eq!(runtime.current_mode().name(), crate::modes::VimMode::INSERT_ID.name());

    // Undo batch should have started
    let begins = mock_undo.batch_begins.read();
    assert_eq!(begins.len(), 1);
}

// ========================================================================
// Coverage: operator error path and tracing closure
// ========================================================================

#[test]
fn test_execute_operator_buffer_not_found_returns_error() {
    // Exercise the Err(e) path at line 240 in execute_operator.
    // Provide a buffer_id that doesn't exist in the kernel's buffer manager,
    // so operator.execute() returns Err(BufferNotFound).
    let ctx = create_test_context();
    let fake_id = BufferId::from_raw(9999);

    let mut args = CommandContext::new();
    args.set_buffer_id(fake_id);
    args.set("range_start", ArgValue::Position(0, 0));
    args.set("range_end", ArgValue::Position(0, 5));

    let mut state = TestState::with_buffer(Some(fake_id));
    let mut runtime = state.runtime(&ctx);
    let result = DeleteCommand.execute(&mut runtime, &args);
    assert!(matches!(result, CommandResult::Error(_)));
}
