use super::*;

use reovim_kernel::testing::create_test_context;

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_get_line_indent_spaces() {
    assert_eq!(get_line_indent("    hello"), "    ");
    assert_eq!(get_line_indent("  world"), "  ");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_get_line_indent_tabs() {
    assert_eq!(get_line_indent("\t\thello"), "\t\t");
    assert_eq!(get_line_indent("\tworld"), "\t");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_get_line_indent_mixed() {
    assert_eq!(get_line_indent("\t  hello"), "\t  ");
    assert_eq!(get_line_indent("  \thello"), "  \t");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_get_line_indent_no_indent() {
    assert_eq!(get_line_indent("hello"), "");
    assert_eq!(get_line_indent("world"), "");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_get_line_indent_empty() {
    assert_eq!(get_line_indent(""), "");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_get_line_indent_whitespace_only() {
    assert_eq!(get_line_indent("    "), "    ");
    assert_eq!(get_line_indent("\t\t"), "\t\t");
}

// ========================================================================
// Command metadata tests
// ========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_first_non_blank_id() {
    use reovim_driver_command::Command;
    let cmd = EnterInsertFirstNonBlank;
    assert_eq!(cmd.id(), ids::ENTER_INSERT_BOL);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_first_non_blank_description() {
    use reovim_driver_command::Command;
    let cmd = EnterInsertFirstNonBlank;
    assert!(cmd.description().contains("first non-blank"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_end_of_line_id() {
    use reovim_driver_command::Command;
    let cmd = EnterInsertEndOfLine;
    assert_eq!(cmd.id(), ids::ENTER_INSERT_EOL);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_end_of_line_description() {
    use reovim_driver_command::Command;
    let cmd = EnterInsertEndOfLine;
    assert!(cmd.description().contains("end of line"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_open_line_below_id() {
    use reovim_driver_command::Command;
    let cmd = OpenLineBelow;
    assert_eq!(cmd.id(), ids::OPEN_LINE_BELOW);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_open_line_below_description() {
    use reovim_driver_command::Command;
    let cmd = OpenLineBelow;
    assert!(cmd.description().contains("below"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_open_line_above_id() {
    use reovim_driver_command::Command;
    let cmd = OpenLineAbove;
    assert_eq!(cmd.id(), ids::OPEN_LINE_ABOVE);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_open_line_above_description() {
    use reovim_driver_command::Command;
    let cmd = OpenLineAbove;
    assert!(cmd.description().contains("above"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_all_mode_entry_commands_debug() {
    let debug = format!("{:?}", EnterInsertFirstNonBlank);
    assert!(debug.contains("EnterInsertFirstNonBlank"));

    let debug = format!("{:?}", EnterInsertEndOfLine);
    assert!(debug.contains("EnterInsertEndOfLine"));

    let debug = format!("{:?}", OpenLineBelow);
    assert!(debug.contains("OpenLineBelow"));

    let debug = format!("{:?}", OpenLineAbove);
    assert!(debug.contains("OpenLineAbove"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_all_mode_entry_commands_default() {
    let _ = EnterInsertFirstNonBlank;
    let _ = EnterInsertEndOfLine;
    let _ = OpenLineBelow;
    let _ = OpenLineAbove;
}

// ========================================================================
// Additional get_line_indent tests
// ========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_get_line_indent_unicode() {
    // Unicode content but ascii indent
    assert_eq!(get_line_indent("  \u{1f600}hello"), "  ");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_get_line_indent_single_space() {
    assert_eq!(get_line_indent(" x"), " ");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_get_line_indent_single_tab() {
    assert_eq!(get_line_indent("\tx"), "\t");
}

// ========================================================================
// Execute tests
// ========================================================================

use {
    reovim_domain_text::{HistoryRing, RegisterBank},
    reovim_driver_command::CommandHandler,
    reovim_driver_session::{
        ClientId, ExtensionMap, Jumplist, MarkBank, Session, SessionRuntime, WindowLayout,
        testing::StubExecutor,
    },
    reovim_kernel::api::{
        ModeStack,
        v1::{BufferId, KernelContext, RwLock},
    },
    reovim_provider_text::{Buffer, TextBufferRegistry},
    std::sync::Arc,
};

fn make_test_ctx() -> KernelContext {
    let ctx = create_test_context();
    ctx.services.register(Arc::new(TextBufferRegistry::new()));
    ctx
}

fn register_buf(ctx: &KernelContext, buffer: Buffer) -> BufferId {
    let arc = Arc::new(RwLock::new(buffer));
    if let Some(reg) = ctx.services.get::<TextBufferRegistry>() {
        reg.register(arc.clone());
    }
    ctx.buffers.register(arc)
}

use reovim_driver_session::api::ModeApi;

struct TestState {
    session: Session,
    mode_stack: ModeStack,
    windows: WindowLayout,
    extensions: ExtensionMap,
    compositor: Option<Box<dyn reovim_subsys_layout::RootCompositor>>,
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

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_first_non_blank_execute() {
    let ctx = make_test_ctx();
    let buffer = Buffer::from_string("    hello");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = EnterInsertFirstNonBlank.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);
    assert_eq!(runtime.current_mode(), &VimMode::INSERT_ID);

    // Cursor should be at column 4 (first non-blank)
    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.column, 4);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_first_non_blank_no_indent() {
    let ctx = make_test_ctx();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = EnterInsertFirstNonBlank.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);

    // Cursor should be at column 0 (no indent)
    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.column, 0);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_first_non_blank_without_buffer() {
    let ctx = make_test_ctx();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);
    let result = EnterInsertFirstNonBlank.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);
    assert_eq!(runtime.current_mode(), &VimMode::INSERT_ID);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_end_of_line_execute() {
    let ctx = make_test_ctx();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = EnterInsertEndOfLine.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);
    assert_eq!(runtime.current_mode(), &VimMode::INSERT_ID);

    // Cursor should be at column 5 (end of "hello")
    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.column, 5);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_end_of_line_empty_line() {
    let ctx = make_test_ctx();
    let buffer = Buffer::from_string("");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = EnterInsertEndOfLine.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.column, 0);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_open_line_below_execute() {
    let ctx = make_test_ctx();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = OpenLineBelow.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);
    assert_eq!(runtime.current_mode(), &VimMode::INSERT_ID);

    // Cursor should be on line 1 (new line below)
    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.line, 1);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_open_line_below_without_buffer() {
    let ctx = make_test_ctx();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);
    let result = OpenLineBelow.execute(&mut runtime, &args);
    assert!(matches!(result, reovim_driver_command::CommandResult::Error(_)));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_open_line_below_with_indent() {
    let ctx = make_test_ctx();
    let buffer = Buffer::from_string("    hello");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = OpenLineBelow.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);

    // Cursor should be at end of indent on the new line
    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.line, 1);
    assert_eq!(window.cursor.column, 4); // 4 spaces indent
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_open_line_above_execute() {
    let ctx = make_test_ctx();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = OpenLineAbove.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);
    assert_eq!(runtime.current_mode(), &VimMode::INSERT_ID);

    // Cursor should be on line 0 (new line above)
    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.line, 0);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_open_line_above_without_buffer() {
    let ctx = make_test_ctx();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);
    let result = OpenLineAbove.execute(&mut runtime, &args);
    assert!(matches!(result, reovim_driver_command::CommandResult::Error(_)));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_open_line_above_with_indent() {
    let ctx = make_test_ctx();
    let buffer = Buffer::from_string("    hello");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = OpenLineAbove.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);

    // Cursor should be at end of indent on the new line (line 0)
    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 4); // 4 spaces indent
}

// ========================================================================
// Additional get_line_indent edge cases
// ========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_get_line_indent_many_spaces() {
    assert_eq!(get_line_indent("        deep"), "        ");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_get_line_indent_only_newline_chars() {
    // \n is not considered whitespace by char::is_whitespace in this context
    // since we parse line by line (no newlines in a line)
    assert_eq!(get_line_indent("text"), "");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_get_line_indent_form_feed() {
    // \x0c is a whitespace character
    assert_eq!(get_line_indent("\x0chello"), "\x0c");
}

// ========================================================================
// Additional execute tests
// ========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_first_non_blank_all_whitespace() {
    let ctx = make_test_ctx();
    let buffer = Buffer::from_string("    ");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = EnterInsertFirstNonBlank.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);

    // When all whitespace, cursor goes to column 0
    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.column, 0);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_first_non_blank_tab_indent() {
    let ctx = make_test_ctx();
    let buffer = Buffer::from_string("\t\thello");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = EnterInsertFirstNonBlank.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.column, 2); // 2 tab chars
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_end_of_line_multiline() {
    let ctx = make_test_ctx();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    // Cursor on second line
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(1, 0).into();
    }
    let mut runtime = state.runtime(&ctx);
    let result = EnterInsertEndOfLine.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.line, 1);
    assert_eq!(window.cursor.column, 5); // end of "world"
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_end_of_line_without_buffer() {
    let ctx = make_test_ctx();
    let args = CommandContext::new();

    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);
    let result = EnterInsertEndOfLine.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);
    assert_eq!(runtime.current_mode(), &VimMode::INSERT_ID);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_open_line_below_empty_buffer() {
    let ctx = make_test_ctx();
    let buffer = Buffer::from_string("");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = OpenLineBelow.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.line, 1);
    assert_eq!(window.cursor.column, 0);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_open_line_below_tab_indent() {
    let ctx = make_test_ctx();
    let buffer = Buffer::from_string("\thello");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = OpenLineBelow.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.line, 1);
    assert_eq!(window.cursor.column, 1); // 1 tab char
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_open_line_above_empty_buffer() {
    let ctx = make_test_ctx();
    let buffer = Buffer::from_string("");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = OpenLineAbove.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 0);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_open_line_above_tab_indent() {
    let ctx = make_test_ctx();
    let buffer = Buffer::from_string("\thello");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = OpenLineAbove.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 1); // 1 tab char
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_open_line_below_multiline_at_middle() {
    let ctx = make_test_ctx();
    let buffer = Buffer::from_string("aaa\n  bbb\nccc");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    // Cursor on second line (which has indent)
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(1, 0).into();
    }
    let mut runtime = state.runtime(&ctx);
    let result = OpenLineBelow.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.line, 2);
    assert_eq!(window.cursor.column, 2); // 2 spaces from "  bbb"
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_open_line_above_multiline_at_middle() {
    let ctx = make_test_ctx();
    let buffer = Buffer::from_string("aaa\n  bbb\nccc");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    // Cursor on second line (which has indent)
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(1, 0).into();
    }
    let mut runtime = state.runtime(&ctx);
    let result = OpenLineAbove.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.line, 1);
    assert_eq!(window.cursor.column, 2); // 2 spaces from "  bbb"
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_first_non_blank_on_second_line() {
    let ctx = make_test_ctx();
    let buffer = Buffer::from_string("hello\n    world");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    // Cursor on second line
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(1, 0).into();
    }
    let mut runtime = state.runtime(&ctx);
    let result = EnterInsertFirstNonBlank.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.line, 1);
    assert_eq!(window.cursor.column, 4); // first non-blank on second line
}

// ========================================================================
// Undo batching tests (exercises begin_insert_batch path)
// ========================================================================

use {
    reovim_domain_text::{Edit, UndoResult, UndoTree},
    reovim_driver_undo::{UndoKey, UndoPersistError, UndoProvider, UndoProviderRegistry},
    reovim_kernel::api::v1::{OptionSpec, OptionValue},
    reovim_subsys_vfs::VfsDriver,
};

struct MockUndoProvider {
    batch_begins: RwLock<Vec<(BufferId, Position)>>,
}

impl MockUndoProvider {
    fn new() -> Self {
        Self {
            batch_begins: RwLock::new(Vec::new()),
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
    fn record(&self, _: BufferId, _: Vec<Edit>, _: Position, _: Position) {}
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
    let ctx = make_test_ctx();
    let mock_undo = Arc::new(MockUndoProvider::new());
    let undo_registry = Arc::new(UndoProviderRegistry::new());
    undo_registry.register(UndoKey::Buffer, mock_undo.clone() as Arc<dyn UndoProvider>);
    ctx.services.register(undo_registry);
    (ctx, mock_undo)
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_first_non_blank_calls_begin_batch() {
    let (ctx, mock_undo) = create_test_context_with_undo();
    let buffer = Buffer::from_string("  hello");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = EnterInsertFirstNonBlank.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);

    let begins = mock_undo.batch_begins.read();
    assert_eq!(begins.len(), 1);
    assert_eq!(begins[0].0, buffer_id);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_end_of_line_calls_begin_batch() {
    let (ctx, mock_undo) = create_test_context_with_undo();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = EnterInsertEndOfLine.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);

    let begins = mock_undo.batch_begins.read();
    assert_eq!(begins.len(), 1);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_open_line_below_calls_begin_batch() {
    let (ctx, mock_undo) = create_test_context_with_undo();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = OpenLineBelow.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);

    let begins = mock_undo.batch_begins.read();
    assert_eq!(begins.len(), 1);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_open_line_above_calls_begin_batch() {
    let (ctx, mock_undo) = create_test_context_with_undo();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = OpenLineAbove.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);

    let begins = mock_undo.batch_begins.read();
    assert_eq!(begins.len(), 1);
}

// ========================================================================
// Autoindent disabled tests
// ========================================================================

fn create_test_context_autoindent_disabled() -> KernelContext {
    let ctx = make_test_ctx();
    // Register the autoindent option as disabled
    let _ = ctx.options.register(OptionSpec::new(
        "autoindent",
        "Enable auto-indentation",
        OptionValue::Bool(false),
    ));
    ctx
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_open_line_below_autoindent_disabled() {
    let ctx = create_test_context_autoindent_disabled();
    let buffer = Buffer::from_string("    hello");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = OpenLineBelow.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);

    // With autoindent disabled, new line should have NO indent
    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.line, 1);
    assert_eq!(window.cursor.column, 0); // No indent
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_open_line_above_autoindent_disabled() {
    let ctx = create_test_context_autoindent_disabled();
    let buffer = Buffer::from_string("    hello");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);
    let result = OpenLineAbove.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);

    // With autoindent disabled, new line should have NO indent
    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 0); // No indent
}

// ========================================================================
// No-window edge cases (exercises get_cursor_position returning None)
// ========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_open_line_below_no_active_window() {
    let ctx = make_test_ctx();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    // Create state with no active window
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

    let result = OpenLineBelow.execute(&mut runtime, &args);
    // Should fail because no cursor position available
    assert!(matches!(result, reovim_driver_command::CommandResult::Error(_)));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_open_line_above_no_active_window() {
    let ctx = make_test_ctx();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    // Create state with no active window
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

    let result = OpenLineAbove.execute(&mut runtime, &args);
    // Should fail because no cursor position available
    assert!(matches!(result, reovim_driver_command::CommandResult::Error(_)));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_first_non_blank_no_active_window() {
    let ctx = make_test_ctx();
    let buffer = Buffer::from_string("  hello");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    // Create state with no active window
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

    // Should still succeed (enters insert mode) but skips cursor movement
    let result = EnterInsertFirstNonBlank.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);
    assert_eq!(runtime.current_mode(), &VimMode::INSERT_ID);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_insert_end_of_line_no_active_window() {
    let ctx = make_test_ctx();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    // Create state with no active window
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

    // Should still succeed but skip cursor movement
    let result = EnterInsertEndOfLine.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);
    assert_eq!(runtime.current_mode(), &VimMode::INSERT_ID);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_open_line_below_multiline_last_line() {
    let ctx = make_test_ctx();
    let buffer = Buffer::from_string("hello\n  world");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(1, 0).into();
    }
    let mut runtime = state.runtime(&ctx);
    let result = OpenLineBelow.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.line, 2);
    assert_eq!(window.cursor.column, 2); // indent from "  world"
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_open_line_above_multiline_first_line_with_indent() {
    let ctx = make_test_ctx();
    let buffer = Buffer::from_string("  hello\nworld");
    let buffer_id = register_buf(&ctx, buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let mut state = TestState::with_buffer(Some(buffer_id));
    // Cursor on first line
    let mut runtime = state.runtime(&ctx);
    let result = OpenLineAbove.execute(&mut runtime, &args);
    assert_eq!(result, reovim_driver_command::CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 2); // indent from "  hello"
}
