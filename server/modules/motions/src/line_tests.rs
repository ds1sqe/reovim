use {
    crate::{ids, line::*},
    reovim_driver_command::{
        ArgKind, ArgValue, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{
        ClientId, ExtensionMap, Session, SessionRuntime, Window, WindowLayout,
        api::CommandExecutor, testing::StubExecutor,
    },
    reovim_kernel::{
        api::{
            KernelContext, ModeStack,
            v1::{BufferId, HistoryRing, MarkBank, ModeId, ModuleId, Position, RegisterBank},
        },
        testing::{create_test_context, setup_buffer},
    },
};

// =========================================================================
// Test Infrastructure (#471)
// =========================================================================

/// Explicit test setup for motion commands.
///
/// All state is explicit - no hidden implicit state in helper functions.
/// Per-client state is held as separate fields to avoid borrow conflicts (#471).
struct TestSetup {
    ctx: KernelContext,
    session: Session,
    // Per-client state as separate fields (#471 borrow checker fix)
    mode_stack: ModeStack,
    windows: WindowLayout,
    extensions: ExtensionMap,
    compositor: Option<Box<dyn reovim_driver_layout::RootCompositor>>,
    registers: RegisterBank,
    clipboard_history: HistoryRing,
    local_marks: MarkBank,
    active_buffer: Option<BufferId>,
    terminal_size: (u16, u16),
    buffer_id: BufferId,
}

impl TestSetup {
    /// Create test setup with buffer content.
    fn new(content: &str) -> Self {
        let ctx = create_test_context();
        let buffer_id = setup_buffer(&ctx, content);

        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let session = Session::new(ClientId::new(1), home_mode.clone()); // #491

        // Per-client state as separate fields (#471)
        let mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let extensions = ExtensionMap::new();

        // Create window with buffer
        let mut window = Window::new();
        window.buffer_id = Some(buffer_id);
        windows.add(window);

        Self {
            ctx,
            session,
            mode_stack,
            windows,
            extensions,
            compositor: None,
            registers: RegisterBank::new(),
            clipboard_history: HistoryRing::new(),
            local_marks: MarkBank::new(),
            active_buffer: None,
            terminal_size: (80, 24),
            buffer_id,
        }
    }

    /// Set cursor position explicitly.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn set_cursor(&mut self, pos: Position) {
        if let Some(window) = self.windows.active_mut() {
            window.cursor = pos.into();
        }
    }

    /// Get current cursor position.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn cursor(&self) -> Position {
        self.windows
            .active()
            .map_or_else(|| Position::new(0, 0), |w| Position::new(w.cursor.line, w.cursor.column))
    }

    /// Create command args with `buffer_id` set.
    fn args(&self) -> CommandContext {
        let mut args = CommandContext::new();
        args.set_buffer_id(self.buffer_id);
        args
    }

    /// Run a command and return the result.
    fn run<C: CommandHandler>(&mut self, cmd: &C, args: &CommandContext) -> CommandResult {
        let executor = StubExecutor;
        let mut tabs = reovim_driver_session::TabPageSet::new();
        let mut runtime = SessionRuntime::new(
            &mut self.session,
            reovim_driver_session::ClientContext {
                mode_stack: &mut self.mode_stack,
                windows: &mut self.windows,
                extensions: &mut self.extensions,
                compositor: &mut self.compositor,
                tabs: &mut tabs,
                registers: &mut self.registers,
                clipboard_history: &mut self.clipboard_history,
                local_marks: &mut self.local_marks,
                active_buffer: &mut self.active_buffer,
                terminal_size: &mut self.terminal_size,
            },
            &self.ctx,
            &executor,
        );
        cmd.execute(&mut runtime, args)
    }
}

/// Run command without buffer (for error tests).
/// Creates a session with an empty window to satisfy `windows()` (#471).
fn run_command_no_buffer<C: CommandHandler>(cmd: &C) -> CommandResult {
    let ctx = KernelContext::default();
    let home_mode = ModeId::new(ModuleId::new("test"), "normal");
    let mut session = Session::new(ClientId::new(1), home_mode.clone()); // #491

    // Per-client state as separate fields (#471)
    let mut mode_stack = ModeStack::new(home_mode);
    let mut windows = WindowLayout::empty();
    let mut extensions = ExtensionMap::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    // Add an empty window so windows() doesn't panic
    windows.add(Window::new());

    let executor = StubExecutor;
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
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
        &ctx,
        &executor,
    );
    cmd.execute(&mut runtime, &CommandContext::new())
}

// =========================================================================
// Command ID Tests
// =========================================================================

#[test]
fn test_line_start_id() {
    let cmd = LineStart;
    assert_eq!(cmd.id().module(), &ids::MODULE);
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
    let mut setup = TestSetup::new("  hello world");
    setup.set_cursor(Position::new(0, 5)); // Middle of line

    let args = setup.args();
    let result = setup.run(&LineStart, &args);

    assert!(result.is_success());
    assert_eq!(setup.cursor().column, 0);
}

#[test]
fn test_line_start_on_empty_line() {
    let mut setup = TestSetup::new("hello\n\nworld");
    setup.set_cursor(Position::new(1, 0)); // On empty line

    let args = setup.args();
    let result = setup.run(&LineStart, &args);

    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 1);
    assert_eq!(setup.cursor().column, 0);
}

// =========================================================================
// Line End ($) Tests
// =========================================================================

#[test]
fn test_line_end_basic() {
    let mut setup = TestSetup::new("hello world");
    setup.set_cursor(Position::new(0, 0));

    let args = setup.args();
    let result = setup.run(&LineEnd, &args);

    assert!(result.is_success());
    assert_eq!(setup.cursor().column, 10); // Last char 'd' at index 10
}

#[test]
fn test_line_end_on_empty_line() {
    let mut setup = TestSetup::new("hello\n\nworld");
    setup.set_cursor(Position::new(1, 0)); // On empty line

    let args = setup.args();
    let result = setup.run(&LineEnd, &args);

    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 1);
    // Empty line - $ should stay at 0 or go to 0
}

// =========================================================================
// First Non-Blank (^) Tests
// =========================================================================

#[test]
fn test_first_non_blank_basic() {
    let mut setup = TestSetup::new("  hello world");
    setup.set_cursor(Position::new(0, 10)); // At end of line

    let args = setup.args();
    let result = setup.run(&FirstNonBlank, &args);

    assert!(result.is_success());
    assert_eq!(setup.cursor().column, 2); // 'h' is at column 2
}

#[test]
fn test_first_non_blank_no_leading_whitespace() {
    let mut setup = TestSetup::new("hello world");
    setup.set_cursor(Position::new(0, 5)); // In middle

    let args = setup.args();
    let result = setup.run(&FirstNonBlank, &args);

    assert!(result.is_success());
    assert_eq!(setup.cursor().column, 0); // 'h' is at column 0
}

// =========================================================================
// Document Start (gg) Tests
// =========================================================================

#[test]
fn test_document_start_basic() {
    let mut setup = TestSetup::new("line one\nline two\nline three");
    setup.set_cursor(Position::new(2, 3)); // On last line

    let args = setup.args();
    let result = setup.run(&DocumentStart, &args);

    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 0);
    assert_eq!(setup.cursor().column, 0);
}

#[test]
fn test_document_start_with_count() {
    let mut setup = TestSetup::new("line one\nline two\nline three");
    setup.set_cursor(Position::new(0, 0));

    let mut args = setup.args();
    args.set("count", ArgValue::Count(2)); // Go to line 2 (0-indexed: line 1)

    let result = setup.run(&DocumentStart, &args);

    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 1); // Line 2 in vim is line 1 in 0-indexed
}

// =========================================================================
// Document End (G) Tests
// =========================================================================

#[test]
fn test_document_end_basic() {
    let mut setup = TestSetup::new("line one\nline two\nline three");
    setup.set_cursor(Position::new(0, 0));

    let args = setup.args();
    let result = setup.run(&DocumentEnd, &args);

    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 2); // Last line
}

#[test]
fn test_document_end_with_count() {
    let mut setup = TestSetup::new("line one\nline two\nline three");
    setup.set_cursor(Position::new(0, 0)); // At start

    let mut args = setup.args();
    args.set("count", ArgValue::Count(2)); // Go to line 2 (0-indexed: line 1)

    let result = setup.run(&DocumentEnd, &args);

    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 1); // Line 2 in vim is line 1 in 0-indexed
}

#[test]
fn test_document_end_single_line_buffer() {
    let mut setup = TestSetup::new("only line");
    setup.set_cursor(Position::new(0, 0));

    let args = setup.args();
    let result = setup.run(&DocumentEnd, &args);

    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 0); // Only line
}

// =========================================================================
// Error Handling Tests
// =========================================================================

#[test]
fn test_line_motion_no_buffer_returns_error() {
    let result = run_command_no_buffer(&LineStart);
    assert!(result.is_error());
}

// =========================================================================
// Command Count Tests
// =========================================================================

#[test]
fn test_all_commands_count() {
    let cmds = all_commands();
    assert_eq!(cmds.len(), 6); // 0, $, ^, gg, G, whole-line
}

// =========================================================================
// Description Tests
// =========================================================================

#[test]
fn test_line_command_descriptions() {
    assert_eq!(LineStart.description(), "Move to start of line");
    assert_eq!(LineEnd.description(), "Move to end of line");
    assert_eq!(FirstNonBlank.description(), "Move to first non-blank character");
    assert_eq!(DocumentStart.description(), "Move to start of document");
    assert_eq!(DocumentEnd.description(), "Move to end of document or line N");
    assert_eq!(WholeLine.description(), "Whole line motion (for operator doubling)");
}

// =========================================================================
// Args Tests
// =========================================================================

#[test]
fn test_line_start_has_no_args() {
    assert!(LineStart.args().is_empty());
}

#[test]
fn test_line_end_has_no_args() {
    assert!(LineEnd.args().is_empty());
}

#[test]
fn test_first_non_blank_has_no_args() {
    assert!(FirstNonBlank.args().is_empty());
}

#[test]
fn test_document_start_has_count_arg() {
    let args = DocumentStart.args();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].name, "count");
    assert_eq!(args[0].kind, ArgKind::Count);
}

#[test]
fn test_document_end_has_count_arg() {
    let args = DocumentEnd.args();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].name, "count");
    assert_eq!(args[0].kind, ArgKind::Count);
}

#[test]
fn test_whole_line_has_count_arg() {
    let args = WholeLine.args();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].name, "count");
    assert_eq!(args[0].kind, ArgKind::Count);
}

// =========================================================================
// Additional Line Start Tests
// =========================================================================

#[test]
fn test_line_start_already_at_start_is_noop() {
    let mut setup = TestSetup::new("hello world");
    setup.set_cursor(Position::new(0, 0));

    let args = setup.args();
    let result = setup.run(&LineStart, &args);
    assert!(result.is_success());
    assert_eq!(setup.cursor().column, 0);
}

#[test]
fn test_line_start_on_second_line() {
    let mut setup = TestSetup::new("first line\n  second line");
    setup.set_cursor(Position::new(1, 8));

    let args = setup.args();
    let result = setup.run(&LineStart, &args);
    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 1);
    assert_eq!(setup.cursor().column, 0);
}

// =========================================================================
// Additional Line End Tests
// =========================================================================

#[test]
fn test_line_end_multiline() {
    let mut setup = TestSetup::new("hello\nworld");
    setup.set_cursor(Position::new(0, 0));

    let args = setup.args();
    let result = setup.run(&LineEnd, &args);
    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 0);
    assert_eq!(setup.cursor().column, 4); // last char 'o' at col 4
}

#[test]
fn test_line_end_already_at_end_is_noop() {
    let mut setup = TestSetup::new("hello");
    setup.set_cursor(Position::new(0, 4));

    let args = setup.args();
    let result = setup.run(&LineEnd, &args);
    assert!(result.is_success());
    assert_eq!(setup.cursor().column, 4);
}

// =========================================================================
// Additional First Non-Blank Tests
// =========================================================================

#[test]
fn test_first_non_blank_tab_indentation() {
    let mut setup = TestSetup::new("\t\thello");
    setup.set_cursor(Position::new(0, 5));

    let args = setup.args();
    let result = setup.run(&FirstNonBlank, &args);
    assert!(result.is_success());
    assert_eq!(setup.cursor().column, 2); // 'h' is at column 2
}

#[test]
fn test_first_non_blank_already_there_is_noop() {
    let mut setup = TestSetup::new("  hello");
    setup.set_cursor(Position::new(0, 2));

    let args = setup.args();
    let result = setup.run(&FirstNonBlank, &args);
    assert!(result.is_success());
    assert_eq!(setup.cursor().column, 2);
}

// =========================================================================
// Additional Document Start (gg) Tests
// =========================================================================

#[test]
fn test_document_start_already_at_start_is_noop() {
    let mut setup = TestSetup::new("line one\nline two");
    setup.set_cursor(Position::new(0, 0));

    let args = setup.args();
    let result = setup.run(&DocumentStart, &args);
    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 0);
}

#[test]
fn test_document_start_with_count_past_end() {
    let mut setup = TestSetup::new("line one\nline two");
    setup.set_cursor(Position::new(0, 0));

    let mut args = setup.args();
    args.set("count", ArgValue::Count(100)); // way past end

    let result = setup.run(&DocumentStart, &args);
    // Should succeed, but clamp to last line
    assert!(result.is_success());
}

// =========================================================================
// Additional Document End (G) Tests
// =========================================================================

#[test]
fn test_document_end_already_at_end_is_noop() {
    let mut setup = TestSetup::new("line one\nline two");
    setup.set_cursor(Position::new(1, 0)); // already at last line

    let args = setup.args();
    let result = setup.run(&DocumentEnd, &args);
    assert!(result.is_success());
}

#[test]
fn test_document_end_many_lines() {
    let mut setup = TestSetup::new("line 1\nline 2\nline 3\nline 4\nline 5");
    setup.set_cursor(Position::new(0, 0));

    let args = setup.args();
    let result = setup.run(&DocumentEnd, &args);
    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 4);
}

// =========================================================================
// WholeLine Tests
// =========================================================================

#[test]
fn test_whole_line_execute_returns_success() {
    let mut setup = TestSetup::new("hello world");
    let args = setup.args();
    let result = setup.run(&WholeLine, &args);
    assert!(result.is_success());
}

#[test]
fn test_whole_line_id() {
    let cmd = WholeLine;
    assert_eq!(cmd.id().module(), &ids::MODULE);
    assert_eq!(cmd.id().name(), "whole-line");
}

// =========================================================================
// Error Tests (all line commands)
// =========================================================================

#[test]
fn test_line_end_no_buffer_returns_error() {
    let result = run_command_no_buffer(&LineEnd);
    assert!(result.is_error());
}

#[test]
fn test_first_non_blank_no_buffer_returns_error() {
    let result = run_command_no_buffer(&FirstNonBlank);
    assert!(result.is_error());
}

#[test]
fn test_document_start_no_buffer_returns_error() {
    let result = run_command_no_buffer(&DocumentStart);
    assert!(result.is_error());
}

#[test]
fn test_document_end_no_buffer_returns_error() {
    let result = run_command_no_buffer(&DocumentEnd);
    assert!(result.is_error());
}

// =========================================================================
// Empty Buffer Tests
// =========================================================================

#[test]
fn test_line_start_empty_buffer() {
    let mut setup = TestSetup::new("");
    let args = setup.args();
    let result = setup.run(&LineStart, &args);
    assert!(result.is_success());
}

#[test]
fn test_line_end_empty_buffer() {
    let mut setup = TestSetup::new("");
    let args = setup.args();
    let result = setup.run(&LineEnd, &args);
    assert!(result.is_success());
}

#[test]
fn test_first_non_blank_empty_buffer() {
    let mut setup = TestSetup::new("");
    let args = setup.args();
    let result = setup.run(&FirstNonBlank, &args);
    assert!(result.is_success());
}

#[test]
fn test_document_start_empty_buffer() {
    let mut setup = TestSetup::new("");
    let args = setup.args();
    let result = setup.run(&DocumentStart, &args);
    assert!(result.is_success());
}

#[test]
fn test_document_end_empty_buffer() {
    let mut setup = TestSetup::new("");
    let args = setup.args();
    let result = setup.run(&DocumentEnd, &args);
    assert!(result.is_success());
}

// =========================================================================
// Default Trait Tests
// =========================================================================

#[test]
fn test_line_commands_default_trait() {
    let _: LineStart = LineStart;
    let _: LineEnd = LineEnd;
    let _: FirstNonBlank = FirstNonBlank;
    let _: DocumentStart = DocumentStart;
    let _: DocumentEnd = DocumentEnd;
    let _: WholeLine = WholeLine;
}

// =========================================================================
// Invalid Buffer Tests
// =========================================================================

#[test]
fn test_line_start_invalid_buffer() {
    let ctx = KernelContext::default();
    let home_mode = ModeId::new(ModuleId::new("test"), "normal");
    let mut session = Session::new(ClientId::new(1), home_mode.clone());
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
    windows.add(Window::new());

    let executor = StubExecutor;
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
        &ctx,
        &executor,
    );
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(999));
    let result = LineStart.execute(&mut runtime, &args);
    assert!(result.is_error());
}

// =========================================================================
// Test infrastructure coverage helpers
// =========================================================================

#[test]
fn test_buffer_manager_list_and_count() {
    let setup = TestSetup::new("hello world");
    assert_eq!(setup.ctx.buffers.count(), 1);
    let list = setup.ctx.buffers.list();
    assert!(list.contains(&setup.buffer_id));
}

#[test]
fn test_buffer_manager_unregister() {
    let setup = TestSetup::new("hello");
    let result = setup.ctx.buffers.unregister(setup.buffer_id);
    assert!(result.is_ok());
    assert_eq!(setup.ctx.buffers.count(), 0);
}

#[test]
fn test_buffer_manager_unregister_nonexistent() {
    let setup = TestSetup::new("hello");
    let bid = BufferId::from_raw(999);
    let result = setup.ctx.buffers.unregister(bid);
    assert!(result.is_err());
}

#[test]
fn test_buffer_manager_create() {
    let setup = TestSetup::new("hello");
    let bid = setup.ctx.buffers.create();
    assert!(setup.ctx.buffers.get(bid).is_some());
}

#[test]
fn test_stub_executor_returns_none() {
    let executor = StubExecutor;
    let cmd_id = ids::LINE_START;
    let result = executor.get_handle(&cmd_id);
    assert!(result.is_none());
}

// =========================================================================
// No window error tests
// =========================================================================

#[test]
fn test_line_start_no_window() {
    let ctx = KernelContext::default();
    let home_mode = ModeId::new(ModuleId::new("test"), "normal");
    let mut session = Session::new(ClientId::new(1), home_mode.clone());
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

    let executor = StubExecutor;
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
        &ctx,
        &executor,
    );
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(1));
    let result = LineStart.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_line_end_no_window() {
    let ctx = KernelContext::default();
    let home_mode = ModeId::new(ModuleId::new("test"), "normal");
    let mut session = Session::new(ClientId::new(1), home_mode.clone());
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

    let executor = StubExecutor;
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
        &ctx,
        &executor,
    );
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(1));
    let result = LineEnd.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_document_start_no_window() {
    let ctx = KernelContext::default();
    let home_mode = ModeId::new(ModuleId::new("test"), "normal");
    let mut session = Session::new(ClientId::new(1), home_mode.clone());
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

    let executor = StubExecutor;
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
        &ctx,
        &executor,
    );
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(1));
    let result = DocumentStart.execute(&mut runtime, &args);
    assert!(result.is_error());
}

// =========================================================================
// Invalid buffer tests for more commands
// =========================================================================

#[test]
fn test_line_end_invalid_buffer() {
    let ctx = KernelContext::default();
    let home_mode = ModeId::new(ModuleId::new("test"), "normal");
    let mut session = Session::new(ClientId::new(1), home_mode.clone());
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
    windows.add(Window::new());

    let executor = StubExecutor;
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
        &ctx,
        &executor,
    );
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(999));
    let result = LineEnd.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_first_non_blank_invalid_buffer() {
    let ctx = KernelContext::default();
    let home_mode = ModeId::new(ModuleId::new("test"), "normal");
    let mut session = Session::new(ClientId::new(1), home_mode.clone());
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
    windows.add(Window::new());

    let executor = StubExecutor;
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
        &ctx,
        &executor,
    );
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(999));
    let result = FirstNonBlank.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_document_start_invalid_buffer() {
    let ctx = KernelContext::default();
    let home_mode = ModeId::new(ModuleId::new("test"), "normal");
    let mut session = Session::new(ClientId::new(1), home_mode.clone());
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
    windows.add(Window::new());

    let executor = StubExecutor;
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
        &ctx,
        &executor,
    );
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(999));
    let result = DocumentStart.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_document_end_invalid_buffer() {
    let ctx = KernelContext::default();
    let home_mode = ModeId::new(ModuleId::new("test"), "normal");
    let mut session = Session::new(ClientId::new(1), home_mode.clone());
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
    windows.add(Window::new());

    let executor = StubExecutor;
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
        &ctx,
        &executor,
    );
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(999));
    let result = DocumentEnd.execute(&mut runtime, &args);
    assert!(result.is_error());
}

// =========================================================================
// Additional motion tests
// =========================================================================

#[test]
fn test_line_end_on_second_line() {
    let mut setup = TestSetup::new("hello\nworld");
    setup.set_cursor(Position::new(1, 0));

    let args = setup.args();
    let result = setup.run(&LineEnd, &args);
    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 1);
    assert_eq!(setup.cursor().column, 4);
}

#[test]
fn test_first_non_blank_on_second_line() {
    let mut setup = TestSetup::new("hello\n    world");
    setup.set_cursor(Position::new(1, 8));

    let args = setup.args();
    let result = setup.run(&FirstNonBlank, &args);
    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 1);
    assert_eq!(setup.cursor().column, 4);
}

#[test]
fn test_document_start_with_count_one() {
    let mut setup = TestSetup::new("line one\nline two\nline three");
    setup.set_cursor(Position::new(2, 3));

    let mut args = setup.args();
    args.set("count", ArgValue::Count(1));

    let result = setup.run(&DocumentStart, &args);
    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 0);
}

#[test]
fn test_document_end_with_count_past_end() {
    let mut setup = TestSetup::new("line one\nline two");
    setup.set_cursor(Position::new(0, 0));

    let mut args = setup.args();
    args.set("count", ArgValue::Count(100));

    let result = setup.run(&DocumentEnd, &args);
    assert!(result.is_success());
}

#[test]
fn test_whole_line_multiline() {
    let mut setup = TestSetup::new("line one\nline two\nline three");
    setup.set_cursor(Position::new(1, 3));

    let args = setup.args();
    let result = setup.run(&WholeLine, &args);
    assert!(result.is_success());
}

#[test]
fn test_line_commands_debug() {
    assert!(!format!("{LineStart:?}").is_empty());
    assert!(!format!("{LineEnd:?}").is_empty());
    assert!(!format!("{FirstNonBlank:?}").is_empty());
    assert!(!format!("{DocumentStart:?}").is_empty());
    assert!(!format!("{DocumentEnd:?}").is_empty());
    assert!(!format!("{WholeLine:?}").is_empty());
}

#[test]
fn test_first_non_blank_whitespace_only_line() {
    let mut setup = TestSetup::new("hello\n    \nworld");
    setup.set_cursor(Position::new(1, 2));

    let args = setup.args();
    let result = setup.run(&FirstNonBlank, &args);
    assert!(result.is_success());
}

#[test]
fn test_document_end_with_count_one() {
    let mut setup = TestSetup::new("line one\nline two\nline three");
    setup.set_cursor(Position::new(2, 0));

    let mut args = setup.args();
    args.set("count", ArgValue::Count(1));

    let result = setup.run(&DocumentEnd, &args);
    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 0);
}

#[test]
fn test_whole_line_empty_buffer() {
    let mut setup = TestSetup::new("");
    let args = setup.args();
    let result = setup.run(&WholeLine, &args);
    assert!(result.is_success());
}
