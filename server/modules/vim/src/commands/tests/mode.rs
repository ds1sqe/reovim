#![allow(clippy::significant_drop_tightening, clippy::uninlined_format_args)]
use {
    super::super::*,
    crate::modes::VimMode,
    reovim_module_cmdline::{CmdlinePrompt, CmdlineState},
};

use {
    crate::ids,
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{
        ClientId, ExtensionMap, Session, SessionRuntime, WindowLayout,
        api::{CommandExecutor, CommandHandle},
        testing::StubExecutor,
    },
    reovim_kernel::{
        api::{
            ModeStack,
            v1::{
                Buffer, BufferId, CommandId, HistoryRing, Jumplist, KernelContext, MarkBank,
                Position, RegisterBank, RwLock,
            },
        },
        testing::create_test_context,
    },
    std::{collections::HashMap, sync::Arc},
};

use reovim_driver_session::api::ModeApi;

// ========================================================================
// Test infrastructure
// ========================================================================

/// Test executor that wraps `CommandHandler` instances for name-based dispatch tests.
struct TestExecutor {
    handlers: HashMap<CommandId, Arc<dyn CommandHandler>>,
}

impl TestExecutor {
    fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    fn register(&mut self, handler: Arc<dyn CommandHandler>) {
        self.handlers.insert(handler.id(), handler);
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandExecutor for TestExecutor {
    fn get_handle(&self, id: &CommandId) -> Option<std::sync::Arc<dyn CommandHandle>> {
        self.handlers.get(id).map(|h| {
            let handler = Arc::clone(h);
            Arc::new(TestHandleBridge(handler)) as Arc<dyn CommandHandle>
        })
    }
}

struct TestHandleBridge(Arc<dyn CommandHandler>);

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandle for TestHandleBridge {
    fn execute(
        &self,
        runtime: &mut SessionRuntime<'_>,
        ctx: &reovim_driver_command::CommandContext,
    ) -> CommandResult {
        self.0.execute(runtime, ctx)
    }
}

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
        let home_mode = VimMode::NORMAL_ID;
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
        self.runtime_with_executor(kernel, &StubExecutor)
    }

    fn runtime_with_executor<'a>(
        &'a mut self,
        kernel: &'a KernelContext,
        executor: &'a dyn CommandExecutor,
    ) -> SessionRuntime<'a> {
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
            executor,
        )
    }
}

// ========================================================================
// Metadata tests
// ========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_mode_id() {
    let cmd = EnterInsertMode;
    assert_eq!(cmd.id(), ids::ENTER_INSERT);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_mode_description() {
    let cmd = EnterInsertMode;
    assert!(cmd.description().contains("insert"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_mode_append_id() {
    let cmd = EnterInsertModeAppend;
    assert_eq!(cmd.id(), ids::ENTER_INSERT_AFTER);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_mode_append_description() {
    let cmd = EnterInsertModeAppend;
    assert!(cmd.description().contains("append"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_to_normal_id() {
    let cmd = ExitToNormal;
    assert_eq!(cmd.id(), ids::EXIT_INSERT);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_to_normal_description() {
    let cmd = ExitToNormal;
    assert!(cmd.description().contains("normal"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_window_mode_id() {
    let cmd = EnterWindowMode;
    assert_eq!(cmd.id(), ids::ENTER_WINDOW_MODE);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_window_mode_description() {
    let cmd = EnterWindowMode;
    assert!(cmd.description().contains("window"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cancel_to_normal_id() {
    let cmd = CancelToNormal;
    assert_eq!(cmd.id(), ids::CANCEL_TO_NORMAL);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cancel_to_normal_description() {
    let cmd = CancelToNormal;
    assert!(cmd.description().contains("normal"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_commandline_mode_id() {
    let cmd = EnterCommandLineMode;
    assert_eq!(cmd.id(), ids::ENTER_COMMANDLINE);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_commandline_mode_description() {
    let cmd = EnterCommandLineMode;
    assert!(cmd.description().contains("command-line"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_commandline_mode_id() {
    let cmd = ExitCommandLineMode;
    assert_eq!(cmd.id(), ids::EXIT_COMMANDLINE);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_commandline_mode_description() {
    let cmd = ExitCommandLineMode;
    assert!(cmd.description().contains("command-line"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cancel_commandline_mode_id() {
    let cmd = CancelCommandLineMode;
    assert_eq!(cmd.id(), ids::CANCEL_COMMANDLINE);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cancel_commandline_mode_description() {
    let cmd = CancelCommandLineMode;
    assert!(cmd.description().contains("Cancel"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_search_forward_id() {
    let cmd = EnterSearchForward;
    assert_eq!(cmd.id(), ids::ENTER_SEARCH_FORWARD);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_search_forward_description() {
    let cmd = EnterSearchForward;
    assert!(cmd.description().contains("search"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_search_backward_id() {
    let cmd = EnterSearchBackward;
    assert_eq!(cmd.id(), ids::ENTER_SEARCH_BACKWARD);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_search_backward_description() {
    let cmd = EnterSearchBackward;
    assert!(cmd.description().contains("search"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_all_mode_commands_debug() {
    let debug_insert = format!("{:?}", EnterInsertMode);
    assert!(debug_insert.contains("EnterInsertMode"));

    let debug_append = format!("{:?}", EnterInsertModeAppend);
    assert!(debug_append.contains("EnterInsertModeAppend"));

    let debug_exit = format!("{:?}", ExitToNormal);
    assert!(debug_exit.contains("ExitToNormal"));

    let debug_window = format!("{:?}", EnterWindowMode);
    assert!(debug_window.contains("EnterWindowMode"));

    let debug_cancel = format!("{:?}", CancelToNormal);
    assert!(debug_cancel.contains("CancelToNormal"));

    let debug_cmdline = format!("{:?}", EnterCommandLineMode);
    assert!(debug_cmdline.contains("EnterCommandLineMode"));

    let debug_exit_cmd = format!("{:?}", ExitCommandLineMode);
    assert!(debug_exit_cmd.contains("ExitCommandLineMode"));

    let debug_cancel_cmd = format!("{:?}", CancelCommandLineMode);
    assert!(debug_cancel_cmd.contains("CancelCommandLineMode"));

    let debug_search_forward = format!("{:?}", EnterSearchForward);
    assert!(debug_search_forward.contains("EnterSearchForward"));

    let debug_search_backward = format!("{:?}", EnterSearchBackward);
    assert!(debug_search_backward.contains("EnterSearchBackward"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_all_mode_commands_default() {
    let _ = EnterInsertMode;
    let _ = EnterInsertModeAppend;
    let _ = ExitToNormal;
    let _ = EnterWindowMode;
    let _ = CancelToNormal;
    let _ = EnterCommandLineMode;
    let _ = ExitCommandLineMode;
    let _ = CancelCommandLineMode;
    let _ = EnterSearchForward;
    let _ = EnterSearchBackward;
}

// ========================================================================
// Execute tests - mode transitions
// ========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_mode_execute() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = EnterInsertMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.current_mode(), &VimMode::INSERT_ID);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_mode_without_buffer() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);
    let result = EnterInsertMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.current_mode(), &VimMode::INSERT_ID);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_mode_append_execute() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = EnterInsertModeAppend.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.current_mode(), &VimMode::INSERT_ID);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_mode_append_moves_cursor() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    // Set cursor to position (0, 2) - on 'l'
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(0, 2).into();
    }
    let mut runtime = state.runtime(&ctx);
    let result = EnterInsertModeAppend.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Cursor should move right by 1
    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.column, 3);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_to_normal_execute() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    // Set cursor at column 3 so exiting insert moves it to 2
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(0, 3).into();
    }
    let mut runtime = state.runtime(&ctx);
    let result = ExitToNormal.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.current_mode(), &VimMode::NORMAL_ID);

    // Cursor should move left by 1 (Vim behavior on exit insert)
    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.column, 2);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_to_normal_cursor_at_col_zero() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    // Cursor at column 0 should stay at 0
    let mut runtime = state.runtime(&ctx);
    let result = ExitToNormal.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.column, 0);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_window_mode_execute() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);
    let result = EnterWindowMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.current_mode(), &VimMode::WINDOW_ID);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cancel_to_normal_execute() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);
    let result = CancelToNormal.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.current_mode(), &VimMode::NORMAL_ID);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_commandline_mode_execute() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);
    let result = EnterCommandLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.current_mode(), &VimMode::COMMANDLINE_ID);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_commandline_mode_execute() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);
    let result = ExitCommandLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.current_mode(), &VimMode::NORMAL_ID);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cancel_commandline_mode_execute() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);
    let result = CancelCommandLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.current_mode(), &VimMode::NORMAL_ID);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_search_forward_execute() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);
    let result = EnterSearchForward.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.current_mode(), &VimMode::COMMANDLINE_ID);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_search_backward_execute() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);
    let result = EnterSearchBackward.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.current_mode(), &VimMode::COMMANDLINE_ID);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_to_normal_without_buffer() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);
    let result = ExitToNormal.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.current_mode(), &VimMode::NORMAL_ID);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_append_at_end_of_line() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    // Cursor at end of line (column 4 = last char 'o', line_len = 5)
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(0, 4).into();
    }
    let mut runtime = state.runtime(&ctx);
    let result = EnterInsertModeAppend.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Should move to column 5 (past end of line for insert)
    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.column, 5);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_append_without_buffer() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);
    let result = EnterInsertModeAppend.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.current_mode(), &VimMode::INSERT_ID);
}

// ========================================================================
// Additional execute tests - edge cases
// ========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_mode_append_at_start_of_line() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    // Cursor at column 0
    let mut runtime = state.runtime(&ctx);
    let result = EnterInsertModeAppend.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.column, 1); // Moved right
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_mode_append_empty_line() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = EnterInsertModeAppend.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Cursor stays at 0 (line_len is 0, so not < line_len)
    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.column, 0);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_to_normal_from_middle_of_line() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(0, 5).into();
    }
    let mut runtime = state.runtime(&ctx);
    let result = ExitToNormal.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.current_mode(), &VimMode::NORMAL_ID);

    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.column, 4); // Moved left by 1
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_to_normal_from_column_one() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("ab");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(0, 1).into();
    }
    let mut runtime = state.runtime(&ctx);
    let result = ExitToNormal.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.column, 0);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_commandline_mode_returns_to_normal() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);
    // First enter commandline mode
    EnterCommandLineMode.execute(&mut runtime, &args);
    assert_eq!(runtime.current_mode(), &VimMode::COMMANDLINE_ID);

    // Then exit
    let result = ExitCommandLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.current_mode(), &VimMode::NORMAL_ID);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cancel_commandline_mode_returns_to_normal() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);
    // First enter commandline mode
    EnterCommandLineMode.execute(&mut runtime, &args);

    // Cancel
    let result = CancelCommandLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.current_mode(), &VimMode::NORMAL_ID);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_mode_on_second_line() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(1, 2).into();
    }
    let mut runtime = state.runtime(&ctx);
    let result = EnterInsertMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.current_mode(), &VimMode::INSERT_ID);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_mode_append_on_second_line() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(1, 2).into();
    }
    let mut runtime = state.runtime(&ctx);
    let result = EnterInsertModeAppend.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.line, 1);
    assert_eq!(window.cursor.column, 3); // Moved right
}

// ========================================================================
// Undo batching tests - exercises begin_insert_batch / end_insert_batch
// ========================================================================

use {
    reovim_driver_undo::{UndoKey, UndoPersistError, UndoProvider, UndoProviderRegistry},
    reovim_driver_vfs::VfsDriver,
    reovim_kernel::api::v1::{Edit, UndoResult, UndoTree},
};

/// Minimal mock undo provider that tracks `begin_batch`/`end_batch` calls.
struct MockUndoProvider {
    batch_begins: RwLock<Vec<(BufferId, Position)>>,
    batch_ends: RwLock<Vec<(BufferId, Position)>>,
    records: RwLock<Vec<BufferId>>,
}

impl MockUndoProvider {
    fn new() -> Self {
        Self {
            batch_begins: RwLock::new(Vec::new()),
            batch_ends: RwLock::new(Vec::new()),
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
    fn end_batch(&self, buffer_id: BufferId, cursor_after: Position) {
        self.batch_ends.write().push((buffer_id, cursor_after));
    }
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

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_mode_calls_begin_batch() {
    let (ctx, mock_undo) = create_test_context_with_undo();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = EnterInsertMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // begin_batch should have been called
    let begins = mock_undo.batch_begins.read();
    assert_eq!(begins.len(), 1);
    assert_eq!(begins[0].0, buffer_id);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_append_calls_begin_batch() {
    let (ctx, mock_undo) = create_test_context_with_undo();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = EnterInsertModeAppend.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let begins = mock_undo.batch_begins.read();
    assert_eq!(begins.len(), 1);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_to_normal_calls_end_batch() {
    let (ctx, mock_undo) = create_test_context_with_undo();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(0, 3).into();
    }
    let mut runtime = state.runtime(&ctx);
    let result = ExitToNormal.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // end_batch should have been called
    let ends = mock_undo.batch_ends.read();
    assert_eq!(ends.len(), 1);
    assert_eq!(ends[0].0, buffer_id);
}

// ========================================================================
// VimSessionState dot-repeat recording tests
// ========================================================================

use reovim_driver_session::api::ExtensionApi;

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_mode_clears_insert_buffer() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    // Pre-populate insert_buffer via VimSessionState
    {
        let vim = runtime.ext_mut::<crate::VimSessionState>();
        vim.insert_buffer.push_str("previous text");
    }

    EnterInsertMode.execute(&mut runtime, &args);

    // insert_buffer should be cleared
    let vim = runtime.ext_mut::<crate::VimSessionState>();
    assert!(vim.insert_buffer.is_empty());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_to_normal_records_dot_repeat_for_insert() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    // Simulate insert buffer with typed text
    {
        let vim = runtime.ext_mut::<crate::VimSessionState>();
        vim.insert_buffer.push_str("world");
    }

    ExitToNormal.execute(&mut runtime, &args);

    // last_change should record the insert text
    let vim = runtime.ext_mut::<crate::VimSessionState>();
    assert!(vim.last_change.is_some());
    let last_change = vim.last_change.as_ref().unwrap();
    match &last_change.change_type {
        crate::session_state::ChangeType::Insert { text } => {
            assert_eq!(text, "world");
        }
        _ => panic!("Expected Insert change type"),
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_to_normal_does_not_record_empty_insert() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    // insert_buffer is empty (no text typed)
    ExitToNormal.execute(&mut runtime, &args);

    // last_change should NOT be set for empty insert
    let vim = runtime.ext_mut::<crate::VimSessionState>();
    assert!(vim.last_change.is_none());
}

// ========================================================================
// ExitCommandLineMode search handling tests
// ========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_commandline_mode_search_forward_with_pending() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world hello");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    // Set up search forward mode: pending search + cmdline input
    {
        use reovim_driver_session::api::SearchState;
        runtime
            .ext_mut::<SearchState>()
            .start_pending_search(reovim_driver_search::Direction::Forward);
        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::SearchForward);
        for ch in "world".chars() {
            runtime.ext_mut::<CmdlineState>().insert_char(ch);
        }
    }

    let result = ExitCommandLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.current_mode(), &VimMode::NORMAL_ID);

    // Search state should have stored pattern/direction
    let search_state = runtime.ext_mut::<reovim_driver_session::api::SearchState>();
    assert!(search_state.pattern_for_repeat().is_some());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_commandline_mode_search_backward_with_pending() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world hello");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    // Set up search backward mode
    {
        use reovim_driver_session::api::SearchState;
        runtime
            .ext_mut::<SearchState>()
            .start_pending_search(reovim_driver_search::Direction::Backward);
        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::SearchBackward);
        for ch in "hello".chars() {
            runtime.ext_mut::<CmdlineState>().insert_char(ch);
        }
    }

    let result = ExitCommandLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_commandline_mode_search_empty_cmdline() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    // Set up search forward but with empty cmdline
    {
        use reovim_driver_session::api::SearchState;
        runtime
            .ext_mut::<SearchState>()
            .start_pending_search(reovim_driver_search::Direction::Forward);
        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::SearchForward);
        // Don't set cmdline input - it will be empty
    }

    let result = ExitCommandLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_commandline_mode_command_with_input() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);

    // Enter command mode with some input (no CommandNameIndex registered)
    runtime
        .ext_mut::<CmdlineState>()
        .enter(CmdlinePrompt::Command);
    for ch in "w".chars() {
        runtime.ext_mut::<CmdlineState>().insert_char(ch);
    }

    let result = ExitCommandLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_commandline_mode_command_empty_input() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);

    // Enter command mode with empty input
    runtime
        .ext_mut::<CmdlineState>()
        .enter(CmdlinePrompt::Command);

    let result = ExitCommandLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

// ========================================================================
// execute_search edge cases
// ========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_commandline_search_no_buffer_id() {
    let ctx = create_test_context();
    let args = CommandContext::new(); // no buffer_id

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);

    // Search forward pending with cmdline, but no buffer_id
    {
        use reovim_driver_session::api::SearchState;
        runtime
            .ext_mut::<SearchState>()
            .start_pending_search(reovim_driver_search::Direction::Forward);
        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::SearchForward);
        for ch in "test".chars() {
            runtime.ext_mut::<CmdlineState>().insert_char(ch);
        }
    }

    let result = ExitCommandLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_commandline_search_no_pending() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    // Enter search forward mode but no pending search set
    runtime
        .ext_mut::<CmdlineState>()
        .enter(CmdlinePrompt::SearchForward);
    for ch in "test".chars() {
        runtime.ext_mut::<CmdlineState>().insert_char(ch);
    }

    let result = ExitCommandLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

// ========================================================================
// execute_search with SearchProviderRegistry tests
// ========================================================================

use reovim_driver_search::{
    SearchError, SearchKey, SearchMatch, SearchProvider, SearchProviderRegistry,
};

/// Mock search provider that can be configured to return specific results.
struct MockSearchProvider {
    result: RwLock<Option<Result<Option<SearchMatch>, SearchError>>>,
}

impl MockSearchProvider {
    fn with_match(m: SearchMatch) -> Self {
        Self {
            result: RwLock::new(Some(Ok(Some(m)))),
        }
    }

    fn with_no_match() -> Self {
        Self {
            result: RwLock::new(Some(Ok(None))),
        }
    }

    fn with_error() -> Self {
        Self {
            result: RwLock::new(Some(Err(SearchError::InvalidPattern("test".to_string())))),
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl SearchProvider for MockSearchProvider {
    fn find_next(
        &self,
        _buffer: &Buffer,
        _cursor: Position,
        _pattern: &str,
        _direction: reovim_driver_search::Direction,
        _wrap: bool,
    ) -> Result<Option<SearchMatch>, SearchError> {
        self.result.read().clone().unwrap_or(Ok(None))
    }

    fn find_all(&self, _buffer: &Buffer, _pattern: &str) -> Result<Vec<SearchMatch>, SearchError> {
        Ok(vec![])
    }

    fn word_at_cursor(&self, _buffer: &Buffer, _cursor: Position) -> Option<String> {
        None
    }

    fn find_next_source(
        &self,
        _source: &dyn reovim_driver_search::LineSource,
        _cursor: Position,
        _pattern: &str,
        _direction: reovim_driver_search::Direction,
        _wrap: bool,
    ) -> Result<Option<SearchMatch>, SearchError> {
        self.result.read().clone().unwrap_or(Ok(None))
    }

    fn find_all_source(
        &self,
        _source: &dyn reovim_driver_search::LineSource,
        _pattern: &str,
    ) -> Result<Vec<SearchMatch>, SearchError> {
        Ok(vec![])
    }

    fn word_at_cursor_source(
        &self,
        _source: &dyn reovim_driver_search::LineSource,
        _cursor: Position,
    ) -> Option<String> {
        None
    }
}

fn create_test_context_with_search(provider: Arc<dyn SearchProvider>) -> KernelContext {
    let ctx = create_test_context();
    let search_registry = Arc::new(SearchProviderRegistry::new());
    search_registry.register(SearchKey::Regex, provider);
    ctx.services.register(search_registry);
    ctx
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_commandline_search_with_match_found() {
    let mock = Arc::new(MockSearchProvider::with_match(SearchMatch {
        start: Position::new(0, 6),
        end: Position::new(0, 11),
    }));
    let ctx = create_test_context_with_search(mock);
    let buffer = Buffer::from_string("hello world hello");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    // Set up search forward with pattern
    {
        runtime
            .ext_mut::<reovim_driver_session::api::SearchState>()
            .start_pending_search(reovim_driver_search::Direction::Forward);
        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::SearchForward);
        for ch in "world".chars() {
            runtime.ext_mut::<CmdlineState>().insert_char(ch);
        }
    }

    let result = ExitCommandLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Cursor should have moved to match position
    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 6);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_commandline_search_no_match() {
    let mock = Arc::new(MockSearchProvider::with_no_match());
    let ctx = create_test_context_with_search(mock);
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    {
        runtime
            .ext_mut::<reovim_driver_session::api::SearchState>()
            .start_pending_search(reovim_driver_search::Direction::Forward);
        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::SearchForward);
        for ch in "zzz".chars() {
            runtime.ext_mut::<CmdlineState>().insert_char(ch);
        }
    }

    let result = ExitCommandLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Cursor should remain at original position
    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 0);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_commandline_search_invalid_pattern() {
    let mock = Arc::new(MockSearchProvider::with_error());
    let ctx = create_test_context_with_search(mock);
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    {
        runtime
            .ext_mut::<reovim_driver_session::api::SearchState>()
            .start_pending_search(reovim_driver_search::Direction::Forward);
        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::SearchForward);
        for ch in "[invalid".chars() {
            runtime.ext_mut::<CmdlineState>().insert_char(ch);
        }
    }

    let result = ExitCommandLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Cursor should remain at original position despite error
    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 0);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_commandline_search_backward_with_match() {
    let mock = Arc::new(MockSearchProvider::with_match(SearchMatch {
        start: Position::new(0, 0),
        end: Position::new(0, 5),
    }));
    let ctx = create_test_context_with_search(mock);
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(0, 8).into();
    }
    let mut runtime = state.runtime(&ctx);

    {
        runtime
            .ext_mut::<reovim_driver_session::api::SearchState>()
            .start_pending_search(reovim_driver_search::Direction::Backward);
        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::SearchBackward);
        for ch in "hello".chars() {
            runtime.ext_mut::<CmdlineState>().insert_char(ch);
        }
    }

    let result = ExitCommandLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Cursor should have moved to match
    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 0);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_commandline_search_no_cursor_position() {
    // Test execute_search early return when no active window (no cursor)
    let mock = Arc::new(MockSearchProvider::with_no_match());
    let ctx = create_test_context_with_search(mock);
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    // Create state with no windows (no active window)
    let home_mode = VimMode::NORMAL_ID;
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

    {
        runtime
            .ext_mut::<reovim_driver_session::api::SearchState>()
            .start_pending_search(reovim_driver_search::Direction::Forward);
        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::SearchForward);
        for ch in "test".chars() {
            runtime.ext_mut::<CmdlineState>().insert_char(ch);
        }
    }

    // Should succeed without error even though no cursor available
    let result = ExitCommandLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

// ========================================================================
// execute_ex_command with CommandNameIndex tests (#547)
// ========================================================================

use reovim_driver_command::CommandNameIndex;

/// Simple test command for ex-command dispatch tests.
struct ExTestCmd {
    cmd_id: CommandId,
    result: CommandResult,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl ExTestCmd {
    fn success(name: &'static str) -> Self {
        Self {
            cmd_id: CommandId::new(reovim_kernel::api::v1::ModuleId::new("test"), name),
            result: CommandResult::Success,
        }
    }

    fn failing(name: &'static str) -> Self {
        Self {
            cmd_id: CommandId::new(reovim_kernel::api::v1::ModuleId::new("test"), name),
            result: CommandResult::error("test failure"),
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Command for ExTestCmd {
    fn id(&self) -> CommandId {
        self.cmd_id.clone()
    }
    fn description(&self) -> &'static str {
        "test"
    }
    fn names(&self) -> &[&'static str] {
        // Determined by the actual test setup (name_index controls resolution)
        &[]
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for ExTestCmd {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        self.result.clone()
    }
}

fn create_test_context_with_name_index(name_index: CommandNameIndex) -> KernelContext {
    let ctx = create_test_context();
    ctx.services.register(Arc::new(name_index));
    ctx
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_commandline_ex_command_success() {
    let cmd = Arc::new(ExTestCmd::success("test-cmd"));
    let cmd_handler: Arc<dyn CommandHandler> = Arc::clone(&cmd) as Arc<dyn CommandHandler>;

    // Build name index
    let mut name_index = CommandNameIndex::new();
    let cmd_as_command: Arc<dyn Command> = Arc::clone(&cmd) as Arc<dyn Command>;
    name_index.insert("test".to_string(), cmd.id(), Arc::clone(&cmd_as_command));
    name_index.insert("t".to_string(), cmd.id(), cmd_as_command);

    // Build executor
    let mut executor = TestExecutor::new();
    executor.register(cmd_handler);

    let ctx = create_test_context_with_name_index(name_index);
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime_with_executor(&ctx, &executor);

    runtime
        .ext_mut::<CmdlineState>()
        .enter(CmdlinePrompt::Command);
    for ch in "test".chars() {
        runtime.ext_mut::<CmdlineState>().insert_char(ch);
    }

    let result = ExitCommandLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_commandline_ex_command_not_found() {
    // Empty name index — no commands registered
    let name_index = CommandNameIndex::new();
    let ctx = create_test_context_with_name_index(name_index);
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);

    runtime
        .ext_mut::<CmdlineState>()
        .enter(CmdlinePrompt::Command);
    for ch in "nonexistent".chars() {
        runtime.ext_mut::<CmdlineState>().insert_char(ch);
    }

    // Should succeed but set error message on CmdlineState (#558)
    let result = ExitCommandLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_commandline_ex_command_error() {
    let cmd = Arc::new(ExTestCmd::failing("fail-cmd"));
    let cmd_handler: Arc<dyn CommandHandler> = Arc::clone(&cmd) as Arc<dyn CommandHandler>;

    let mut name_index = CommandNameIndex::new();
    let cmd_as_command: Arc<dyn Command> = Arc::clone(&cmd) as Arc<dyn Command>;
    name_index.insert("fail".to_string(), cmd.id(), cmd_as_command);

    let mut executor = TestExecutor::new();
    executor.register(cmd_handler);

    let ctx = create_test_context_with_name_index(name_index);
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime_with_executor(&ctx, &executor);

    runtime
        .ext_mut::<CmdlineState>()
        .enter(CmdlinePrompt::Command);
    for ch in "fail".chars() {
        runtime.ext_mut::<CmdlineState>().insert_char(ch);
    }

    // Should succeed but set error message on CmdlineState (#558)
    let result = ExitCommandLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_commandline_ex_command_whitespace_only() {
    // Whitespace-only cmdline: passes !is_empty() but parse_cmdline returns None
    let name_index = CommandNameIndex::new();
    let ctx = create_test_context_with_name_index(name_index);
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);

    runtime
        .ext_mut::<CmdlineState>()
        .enter(CmdlinePrompt::Command);
    runtime.ext_mut::<CmdlineState>().insert_char(' ');

    let result = ExitCommandLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_commandline_ex_command_with_bang() {
    let cmd = Arc::new(ExTestCmd::success("test-bang"));
    let cmd_handler: Arc<dyn CommandHandler> = Arc::clone(&cmd) as Arc<dyn CommandHandler>;

    let mut name_index = CommandNameIndex::new();
    let cmd_as_command: Arc<dyn Command> = Arc::clone(&cmd) as Arc<dyn Command>;
    name_index.insert("test".to_string(), cmd.id(), cmd_as_command);

    let mut executor = TestExecutor::new();
    executor.register(cmd_handler);

    let ctx = create_test_context_with_name_index(name_index);
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime_with_executor(&ctx, &executor);

    runtime
        .ext_mut::<CmdlineState>()
        .enter(CmdlinePrompt::Command);
    for ch in "test!".chars() {
        runtime.ext_mut::<CmdlineState>().insert_char(ch);
    }

    let result = ExitCommandLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_commandline_ex_command_with_args() {
    let cmd = Arc::new(ExTestCmd::success("test-args"));
    let cmd_handler: Arc<dyn CommandHandler> = Arc::clone(&cmd) as Arc<dyn CommandHandler>;

    let mut name_index = CommandNameIndex::new();
    let cmd_as_command: Arc<dyn Command> = Arc::clone(&cmd) as Arc<dyn Command>;
    name_index.insert("test".to_string(), cmd.id(), cmd_as_command);

    let mut executor = TestExecutor::new();
    executor.register(cmd_handler);

    let ctx = create_test_context_with_name_index(name_index);
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime_with_executor(&ctx, &executor);

    runtime
        .ext_mut::<CmdlineState>()
        .enter(CmdlinePrompt::Command);
    for ch in "test filename.txt".chars() {
        runtime.ext_mut::<CmdlineState>().insert_char(ch);
    }

    let result = ExitCommandLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_commandline_ex_command_propagates_buffer_id_and_vfs() {
    let cmd = Arc::new(ExTestCmd::success("test-ctx"));
    let cmd_handler: Arc<dyn CommandHandler> = Arc::clone(&cmd) as Arc<dyn CommandHandler>;

    let mut name_index = CommandNameIndex::new();
    let cmd_as_command: Arc<dyn Command> = Arc::clone(&cmd) as Arc<dyn Command>;
    name_index.insert("test".to_string(), cmd.id(), cmd_as_command);

    let mut executor = TestExecutor::new();
    executor.register(cmd_handler);

    let ctx = create_test_context_with_name_index(name_index);
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let mock_vfs = Arc::new(reovim_driver_vfs::MockVfs::new());
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set_vfs(Arc::clone(&mock_vfs) as Arc<dyn reovim_driver_vfs::VfsDriver>);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime_with_executor(&ctx, &executor);

    runtime
        .ext_mut::<CmdlineState>()
        .enter(CmdlinePrompt::Command);
    for ch in "test".chars() {
        runtime.ext_mut::<CmdlineState>().insert_char(ch);
    }

    let result = ExitCommandLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_commandline_search_no_search_registry() {
    // No SearchProviderRegistry registered in services
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    {
        runtime
            .ext_mut::<reovim_driver_session::api::SearchState>()
            .start_pending_search(reovim_driver_search::Direction::Forward);
        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::SearchForward);
        for ch in "world".chars() {
            runtime.ext_mut::<CmdlineState>().insert_char(ch);
        }
    }

    // Should succeed even without search registry (logs warning)
    let result = ExitCommandLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Cursor should not have moved
    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.column, 0);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_commandline_search_no_search_provider_for_key() {
    // Register an empty SearchProviderRegistry (no Regex key)
    let ctx = create_test_context();
    let search_registry = Arc::new(SearchProviderRegistry::new());
    // Don't register any provider
    ctx.services.register(search_registry);

    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    {
        runtime
            .ext_mut::<reovim_driver_session::api::SearchState>()
            .start_pending_search(reovim_driver_search::Direction::Forward);
        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::SearchForward);
        for ch in "test".chars() {
            runtime.ext_mut::<CmdlineState>().insert_char(ch);
        }
    }

    // Should succeed even without search provider (logs warning)
    let result = ExitCommandLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_commandline_search_buffer_not_found() {
    // Register a search provider but use a buffer_id that doesn't exist
    let mock = Arc::new(MockSearchProvider::with_no_match());
    let ctx = create_test_context_with_search(mock);
    // Don't register a buffer - use a fake buffer_id
    let fake_buffer_id = BufferId::from_raw(999);

    let mut args = CommandContext::new();
    args.set_buffer_id(fake_buffer_id);

    let mut state = TestState::with_buffer(Some(fake_buffer_id));
    let mut runtime = state.runtime(&ctx);

    {
        runtime
            .ext_mut::<reovim_driver_session::api::SearchState>()
            .start_pending_search(reovim_driver_search::Direction::Forward);
        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::SearchForward);
        for ch in "test".chars() {
            runtime.ext_mut::<CmdlineState>().insert_char(ch);
        }
    }

    // Should succeed (buffer not found for search is handled gracefully)
    let result = ExitCommandLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

// ========================================================================
// Command-line editing commands (#451)
// ========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_cursor_left_id() {
    assert_eq!(CmdlineCursorLeft.id(), ids::CMDLINE_CURSOR_LEFT);
    assert!(CmdlineCursorLeft.description().contains("left"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_cursor_right_id() {
    assert_eq!(CmdlineCursorRight.id(), ids::CMDLINE_CURSOR_RIGHT);
    assert!(CmdlineCursorRight.description().contains("right"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_cursor_home_id() {
    assert_eq!(CmdlineCursorHome.id(), ids::CMDLINE_CURSOR_HOME);
    assert!(CmdlineCursorHome.description().contains("start"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_cursor_end_id() {
    assert_eq!(CmdlineCursorEnd.id(), ids::CMDLINE_CURSOR_END);
    assert!(CmdlineCursorEnd.description().contains("end"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_delete_char_id() {
    assert_eq!(CmdlineDeleteChar.id(), ids::CMDLINE_DELETE_CHAR);
    assert!(CmdlineDeleteChar.description().contains("cursor"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_backspace_id() {
    assert_eq!(CmdlineBackspace.id(), ids::CMDLINE_BACKSPACE);
    assert!(CmdlineBackspace.description().contains("before"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_delete_word_id() {
    assert_eq!(CmdlineDeleteWord.id(), ids::CMDLINE_DELETE_WORD);
    assert!(CmdlineDeleteWord.description().contains("word"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_delete_to_start_id() {
    assert_eq!(CmdlineDeleteToStart.id(), ids::CMDLINE_DELETE_TO_START);
    assert!(CmdlineDeleteToStart.description().contains("start"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_editing_debug_and_default() {
    let _ = format!("{:?}", CmdlineCursorLeft);
    let _ = format!("{:?}", CmdlineCursorRight);
    let _ = format!("{:?}", CmdlineCursorHome);
    let _ = format!("{:?}", CmdlineCursorEnd);
    let _ = format!("{:?}", CmdlineDeleteChar);
    let _ = format!("{:?}", CmdlineBackspace);
    let _ = format!("{:?}", CmdlineDeleteWord);
    let _ = format!("{:?}", CmdlineDeleteToStart);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_cursor_left_execute() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);

    // Enter cmdline mode and type some chars
    runtime
        .ext_mut::<CmdlineState>()
        .enter(CmdlinePrompt::Command);
    runtime.ext_mut::<CmdlineState>().insert_char('a');
    runtime.ext_mut::<CmdlineState>().insert_char('b');
    assert_eq!(runtime.ext_mut::<CmdlineState>().cursor(), 2);

    let result = CmdlineCursorLeft.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.ext_mut::<CmdlineState>().cursor(), 1);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_cursor_right_execute() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);

    runtime
        .ext_mut::<CmdlineState>()
        .enter(CmdlinePrompt::Command);
    runtime.ext_mut::<CmdlineState>().insert_char('a');
    runtime.ext_mut::<CmdlineState>().move_cursor_left();

    let result = CmdlineCursorRight.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.ext_mut::<CmdlineState>().cursor(), 1);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_cursor_home_execute() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);

    runtime
        .ext_mut::<CmdlineState>()
        .enter(CmdlinePrompt::Command);
    runtime.ext_mut::<CmdlineState>().insert_char('a');

    let result = CmdlineCursorHome.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.ext_mut::<CmdlineState>().cursor(), 0);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_cursor_end_execute() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);

    runtime
        .ext_mut::<CmdlineState>()
        .enter(CmdlinePrompt::Command);
    runtime.ext_mut::<CmdlineState>().insert_char('a');
    runtime.ext_mut::<CmdlineState>().move_to_start();

    let result = CmdlineCursorEnd.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.ext_mut::<CmdlineState>().cursor(), 1);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_delete_char_execute() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);

    runtime
        .ext_mut::<CmdlineState>()
        .enter(CmdlinePrompt::Command);
    runtime.ext_mut::<CmdlineState>().insert_char('a');
    runtime.ext_mut::<CmdlineState>().insert_char('b');
    runtime.ext_mut::<CmdlineState>().move_to_start();

    let result = CmdlineDeleteChar.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.ext_mut::<CmdlineState>().input(), "b");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_backspace_execute() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);

    runtime
        .ext_mut::<CmdlineState>()
        .enter(CmdlinePrompt::Command);
    runtime.ext_mut::<CmdlineState>().insert_char('a');
    runtime.ext_mut::<CmdlineState>().insert_char('b');

    let result = CmdlineBackspace.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.ext_mut::<CmdlineState>().input(), "a");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_delete_word_execute() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);

    runtime
        .ext_mut::<CmdlineState>()
        .enter(CmdlinePrompt::Command);
    for ch in "hello world".chars() {
        runtime.ext_mut::<CmdlineState>().insert_char(ch);
    }

    let result = CmdlineDeleteWord.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.ext_mut::<CmdlineState>().input(), "hello ");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_history_up_id() {
    assert_eq!(CmdlineHistoryUp.id(), ids::CMDLINE_HISTORY_UP);
    assert!(CmdlineHistoryUp.description().contains("older"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_history_down_id() {
    assert_eq!(CmdlineHistoryDown.id(), ids::CMDLINE_HISTORY_DOWN);
    assert!(CmdlineHistoryDown.description().contains("newer"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_history_debug_and_default() {
    let _ = format!("{:?}", CmdlineHistoryUp);
    let _ = format!("{:?}", CmdlineHistoryDown);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_history_up_execute() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);

    // Enter and add history
    runtime
        .ext_mut::<CmdlineState>()
        .enter(CmdlinePrompt::Command);
    runtime.ext_mut::<CmdlineState>().insert_char('w');
    runtime.ext_mut::<CmdlineState>().push_to_history();

    // New cmdline session
    runtime
        .ext_mut::<CmdlineState>()
        .enter(CmdlinePrompt::Command);

    let result = CmdlineHistoryUp.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.ext_mut::<CmdlineState>().input(), "w");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_history_down_execute() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);

    // Enter and add history
    runtime
        .ext_mut::<CmdlineState>()
        .enter(CmdlinePrompt::Command);
    runtime.ext_mut::<CmdlineState>().insert_char('w');
    runtime.ext_mut::<CmdlineState>().push_to_history();

    // New cmdline, navigate up then down
    runtime
        .ext_mut::<CmdlineState>()
        .enter(CmdlinePrompt::Command);
    for ch in "new".chars() {
        runtime.ext_mut::<CmdlineState>().insert_char(ch);
    }
    runtime.ext_mut::<CmdlineState>().history_up();
    assert_eq!(runtime.ext_mut::<CmdlineState>().input(), "w");

    let result = CmdlineHistoryDown.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.ext_mut::<CmdlineState>().input(), "new");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_delete_to_start_execute() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);

    runtime
        .ext_mut::<CmdlineState>()
        .enter(CmdlinePrompt::Command);
    for ch in "hello".chars() {
        runtime.ext_mut::<CmdlineState>().insert_char(ch);
    }

    let result = CmdlineDeleteToStart.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert_eq!(runtime.ext_mut::<CmdlineState>().input(), "");
    assert_eq!(runtime.ext_mut::<CmdlineState>().cursor(), 0);
}

// ========================================================================
// Command-line completion commands (#451)
// ========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_complete_next_id() {
    assert_eq!(CmdlineCompleteNext.id(), ids::CMDLINE_COMPLETE_NEXT);
    assert!(CmdlineCompleteNext.description().contains("next"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_complete_prev_id() {
    assert_eq!(CmdlineCompletePrev.id(), ids::CMDLINE_COMPLETE_PREV);
    assert!(CmdlineCompletePrev.description().contains("previous"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_completion_debug_and_default() {
    let _ = format!("{:?}", CmdlineCompleteNext);
    let _ = format!("{:?}", CmdlineCompletePrev);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_complete_next_with_name_index() {
    struct WriteCmd;
    impl Command for WriteCmd {
        fn id(&self) -> CommandId {
            CommandId::new(reovim_kernel::api::v1::ModuleId::new("test"), "write")
        }
        fn description(&self) -> &'static str {
            "Write"
        }
        fn names(&self) -> &[&'static str] {
            &["w", "write"]
        }
    }

    struct WqCmd;
    impl Command for WqCmd {
        fn id(&self) -> CommandId {
            CommandId::new(reovim_kernel::api::v1::ModuleId::new("test"), "wq")
        }
        fn description(&self) -> &'static str {
            "Write and quit"
        }
        fn names(&self) -> &[&'static str] {
            &["wq"]
        }
    }

    // Build a CommandNameIndex with "w", "write", "wq" names
    let mut name_index = CommandNameIndex::new();
    let write_cmd: Arc<dyn Command> = Arc::new(WriteCmd);
    let wq_cmd: Arc<dyn Command> = Arc::new(WqCmd);

    name_index.insert("w".to_string(), write_cmd.id(), Arc::clone(&write_cmd));
    name_index.insert("write".to_string(), write_cmd.id(), write_cmd);
    name_index.insert("wq".to_string(), wq_cmd.id(), wq_cmd);

    let ctx = create_test_context_with_name_index(name_index);
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);

    runtime
        .ext_mut::<CmdlineState>()
        .enter(CmdlinePrompt::Command);
    runtime.ext_mut::<CmdlineState>().insert_char('w');

    // First Tab populates and selects
    let result = CmdlineCompleteNext.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert!(!runtime.ext_mut::<CmdlineState>().completions().is_empty());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_complete_next_empty_input() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);

    runtime
        .ext_mut::<CmdlineState>()
        .enter(CmdlinePrompt::Command);
    // Empty input — should not populate

    let result = CmdlineCompleteNext.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    assert!(runtime.ext_mut::<CmdlineState>().completions().is_empty());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_complete_prev_execute() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);

    runtime
        .ext_mut::<CmdlineState>()
        .enter(CmdlinePrompt::Command);
    runtime
        .ext_mut::<CmdlineState>()
        .set_completions("w".to_string(), vec!["write".to_string(), "wq".to_string()]);

    let result = CmdlineCompletePrev.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    // Should select last item
    assert_eq!(runtime.ext_mut::<CmdlineState>().input(), "wq");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cmdline_complete_no_registry() {
    let ctx = create_test_context(); // no CommandNameIndex
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);

    runtime
        .ext_mut::<CmdlineState>()
        .enter(CmdlinePrompt::Command);
    runtime.ext_mut::<CmdlineState>().insert_char('w');

    let result = CmdlineCompleteNext.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
    // No registry → empty completions
    assert!(runtime.ext_mut::<CmdlineState>().completions().is_empty());
}
