use {
    super::super::*,
    reovim_driver_command::{ArgKind, ArgValue, Command, CommandContext},
    reovim_driver_session::{
        ClientId, ExtensionMap, Session, SessionRuntime, Window, WindowLayout,
        testing::StubExecutor,
    },
    reovim_kernel::{
        api::v1::{
            Buffer, BufferId, HistoryRing, Jumplist, KernelContext, MarkBank, ModeStack, Position,
            RegisterBank, RegisterContent,
        },
        testing::{create_test_context, test_mode},
    },
};

/// Test state holder for commands (#471).
///
/// Holds per-client state (`mode_stack`, windows, extensions) as **separate fields**
/// to avoid borrow checker conflicts when creating `SessionRuntime::new()`.
///
/// This mirrors the production architecture where `EditingState` is separate from `Session`.
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

#[cfg_attr(coverage_nightly, coverage(off))]
impl TestState {
    fn new() -> Self {
        let home_mode = test_mode();
        Self {
            session: Session::new(ClientId::new(1), home_mode.clone()), // #491
            mode_stack: ModeStack::new(home_mode),
            windows: WindowLayout::empty(),
            extensions: ExtensionMap::new(),
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

    fn with_window(buffer_id: BufferId) -> Self {
        let mut state = Self::new();
        let mut window = Window::new();
        window.buffer_id = Some(buffer_id);
        state.windows.add(window);
        state.active_buffer = Some(buffer_id);
        state
    }

    fn runtime<'a>(
        &'a mut self,
        kernel: &'a KernelContext,
        executor: &'a StubExecutor,
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

/// Create a test runtime for command execution (legacy helper).
///
/// Creates temporary per-client state (`mode_stack`, windows, extensions)
/// for backward compatibility. Use `TestState::runtime()` for tests that
/// need persistent per-client state across multiple runtime operations.
#[cfg_attr(coverage_nightly, coverage(off))]
fn with_test_runtime<F, R>(session: &mut Session, kernel: &KernelContext, f: F) -> R
where
    F: FnOnce(&mut SessionRuntime<'_>) -> R,
{
    let executor = StubExecutor;
    // #491: Use home_mode() since current_mode() was removed from Session
    let mode = session.shared.home_mode().clone();
    let mut mode_stack = ModeStack::new(mode);
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
        session,
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
        kernel,
        &executor,
    );
    f(&mut runtime)
}

// =========================================================================
// Command ID Tests
// =========================================================================

#[test]
fn test_cursor_up_id() {
    let cmd = CursorUp;
    assert_eq!(cmd.id().name(), "cursor-up");
}

#[test]
fn test_cursor_down_id() {
    let cmd = CursorDown;
    assert_eq!(cmd.id().name(), "cursor-down");
}

#[test]
fn test_cursor_left_id() {
    let cmd = CursorLeft;
    assert_eq!(cmd.id().name(), "cursor-left");
}

#[test]
fn test_cursor_right_id() {
    let cmd = CursorRight;
    assert_eq!(cmd.id().name(), "cursor-right");
}

// =========================================================================
// Command Args Tests
// =========================================================================

#[test]
fn test_cursor_commands_have_count_arg() {
    for cmd in cursor_commands() {
        let args = cmd.args();
        assert!(!args.is_empty(), "Command {} should have count arg", cmd.id());
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
    }
}

#[test]
fn test_all_commands_count() {
    let cmds = all_commands();
    // 4 cursor + 2 display + 2 insert-edit + 8 enter-operator
    // + 5 delete + 1 yank + 2 paste + 2 replace (start+exec) + 1 case
    // + 1 repeat + 2 undo + 1 write + 3 mark + 2 jump = 36
    assert_eq!(cmds.len(), 36);
}

#[test]
fn test_cursor_commands_count() {
    let cmds = cursor_commands();
    assert_eq!(cmds.len(), 4);
}

#[test]
fn test_display_line_commands_count() {
    let cmds = display_line_commands();
    assert_eq!(cmds.len(), 2);
}

// =========================================================================
// Cursor Commands Without Buffer ID (Error Handling)
// =========================================================================

#[test]
fn test_cursor_up_no_buffer_id_returns_error() {
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let result = with_test_runtime(&mut session, &kernel, |runtime| {
        let args = CommandContext::new();
        CursorUp.execute(runtime, &args)
    });
    assert!(result.is_error());
}

#[test]
fn test_cursor_down_no_buffer_id_returns_error() {
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let result = with_test_runtime(&mut session, &kernel, |runtime| {
        let args = CommandContext::new();
        CursorDown.execute(runtime, &args)
    });
    assert!(result.is_error());
}

#[test]
fn test_cursor_left_no_buffer_id_returns_error() {
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let result = with_test_runtime(&mut session, &kernel, |runtime| {
        let args = CommandContext::new();
        CursorLeft.execute(runtime, &args)
    });
    assert!(result.is_error());
}

#[test]
fn test_cursor_right_no_buffer_id_returns_error() {
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let result = with_test_runtime(&mut session, &kernel, |runtime| {
        let args = CommandContext::new();
        CursorRight.execute(runtime, &args)
    });
    assert!(result.is_error());
}

// =========================================================================
// Cursor Commands With Invalid Buffer ID (Error Handling)
// =========================================================================

#[test]
fn test_cursor_up_invalid_buffer_gracefully_handles() {
    // Create a valid buffer to set up TestState, but pass an invalid buffer_id
    let kernel = create_test_context();
    let valid_buffer = Buffer::new();
    let valid_buffer_id = kernel.buffers.register(valid_buffer);

    let mut state = TestState::with_window(valid_buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    // Pass an invalid buffer_id (999) - different from the valid one
    // The command gracefully handles this by using 0 as line length
    // (the cursor still exists in the window, just buffer_line_len returns None)
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(999));
    let result = CursorUp.execute(&mut runtime, &args);
    // Command succeeds as a no-op (cursor at line 0, can't move up)
    assert!(result.is_success());
}

// =========================================================================
// Cursor Movement Tests (With Valid Buffer)
// =========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
fn setup_buffer_context() -> (KernelContext, BufferId) {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("line one\nline two\nline three");
    let buffer_id = ctx.buffers.register(buffer);
    (ctx, buffer_id)
}

/// Set up a session with a window displaying the given buffer.
// #491: setup_session_with_window() removed - Session no longer has windows field.
// Use TestState::with_window() instead for tests that need per-client state.

#[test]
fn test_cursor_down_moves_cursor() {
    let (kernel, buffer_id) = setup_buffer_context();
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = CursorDown.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Verify cursor in Window (per-client state, not kernel buffer)
    drop(runtime); // Release borrow
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 1);
    assert_eq!(window.cursor.column, 0);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cursor_up_moves_cursor() {
    let (kernel, buffer_id) = setup_buffer_context();
    let mut state = TestState::with_window(buffer_id);
    // Set initial cursor position in the window
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(2, 0).into();
    }

    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = CursorUp.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Verify cursor in Window (per-client state)
    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 1);
}

#[test]
fn test_cursor_right_moves_cursor() {
    let (kernel, buffer_id) = setup_buffer_context();
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = CursorRight.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Verify cursor in Window (per-client state)
    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 1);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cursor_left_moves_cursor() {
    let (kernel, buffer_id) = setup_buffer_context();
    let mut state = TestState::with_window(buffer_id);
    // Set initial cursor position in the window
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 5).into();
    }

    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = CursorLeft.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Verify cursor in Window (per-client state)
    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.column, 4);
}

// =========================================================================
// Count Argument Tests
// =========================================================================

#[test]
fn test_cursor_down_count_respected() {
    let (kernel, buffer_id) = setup_buffer_context();
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));

    let result = CursorDown.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Verify cursor in Window
    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 2); // Moved down 2 lines
}

#[test]
fn test_cursor_right_count_respected() {
    let (kernel, buffer_id) = setup_buffer_context();
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(3));

    let result = CursorRight.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Verify cursor in Window
    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.column, 3);
}

// =========================================================================
// Boundary Tests
// =========================================================================

#[test]
fn test_cursor_up_at_line_zero_is_noop() {
    let (kernel, buffer_id) = setup_buffer_context();
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    // Cursor starts at (0, 0), should stay there
    let result = CursorUp.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Verify cursor in Window
    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 0);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cursor_down_at_eof_is_noop() {
    let (kernel, buffer_id) = setup_buffer_context();
    let mut state = TestState::with_window(buffer_id);
    // Set initial cursor position to last line
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(2, 0).into();
    }

    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = CursorDown.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Verify cursor in Window - should stay at last line
    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.line, 2);
}

#[test]
fn test_cursor_left_at_col_zero_is_noop() {
    let (kernel, buffer_id) = setup_buffer_context();
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    // Cursor starts at column 0
    let result = CursorLeft.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Verify cursor in Window
    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.column, 0);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_cursor_right_at_eol_is_noop() {
    let (kernel, buffer_id) = setup_buffer_context();
    let mut state = TestState::with_window(buffer_id);
    // Move to last character of line ("line one" is 8 chars, last char at index 7)
    // In normal mode, cursor can't go past the last character
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(0, 7).into(); // 'e' in "line one"
    }

    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = CursorRight.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Verify cursor in Window - should stay at last character
    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.column, 7);
}

// =========================================================================
// Edge Case Tests
// =========================================================================

#[test]
fn test_cursor_movement_empty_buffer() {
    let kernel = create_test_context();
    let buffer = Buffer::new(); // Empty buffer
    let buffer_id = kernel.buffers.register(buffer);

    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    // All movements should succeed (no-op on empty buffer)
    assert!(CursorDown.execute(&mut runtime, &args).is_success());
    assert!(CursorUp.execute(&mut runtime, &args).is_success());
    assert!(CursorLeft.execute(&mut runtime, &args).is_success());
    assert!(CursorRight.execute(&mut runtime, &args).is_success());
}

#[test]
fn test_cursor_movement_single_line() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("single line");
    let buffer_id = kernel.buffers.register(buffer);

    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    // Up/down should be no-op
    assert!(CursorDown.execute(&mut runtime, &args).is_success());
    assert!(CursorUp.execute(&mut runtime, &args).is_success());

    // Left/right should work
    assert!(CursorRight.execute(&mut runtime, &args).is_success());
    // Verify cursor in Window (per-client state)
    drop(runtime);
    let window = state.windows.active().unwrap();
    assert_eq!(window.cursor.column, 1);
}

// =========================================================================
// Undo/Redo Command Tests
// =========================================================================

#[test]
fn test_undo_command_names() {
    let cmd = UndoCommand;
    let names = cmd.names();
    assert!(names.contains(&"u"));
    assert!(names.contains(&"undo"));
}

#[test]
fn test_redo_command_names() {
    let cmd = RedoCommand;
    let names = cmd.names();
    assert!(names.contains(&"redo"));
}

#[test]
fn test_undo_command_returns_error_without_buffer() {
    // Undo requires an active buffer - returns error when none is set
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let result = with_test_runtime(&mut session, &kernel, |runtime| {
        let args = CommandContext::new();
        UndoCommand.execute(runtime, &args)
    });
    assert!(result.is_error());
}

#[test]
fn test_redo_command_returns_error_without_buffer() {
    // Redo requires an active buffer - returns error when none is set
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let result = with_test_runtime(&mut session, &kernel, |runtime| {
        let args = CommandContext::new();
        RedoCommand.execute(runtime, &args)
    });
    assert!(result.is_error());
}

#[test]
fn test_undo_command_with_count() {
    // Undo with count still requires an active buffer
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let result = with_test_runtime(&mut session, &kernel, |runtime| {
        let mut args = CommandContext::new();
        args.set("count", ArgValue::Count(5));
        UndoCommand.execute(runtime, &args)
    });
    assert!(result.is_error()); // No buffer = error
}

#[test]
fn test_redo_command_with_count() {
    // Redo with count still requires an active buffer
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let result = with_test_runtime(&mut session, &kernel, |runtime| {
        let mut args = CommandContext::new();
        args.set("count", ArgValue::Count(3));
        RedoCommand.execute(runtime, &args)
    });
    assert!(result.is_error()); // No buffer = error
}

#[test]
fn test_undo_commands_helper_returns_two_commands() {
    let cmds = undo_commands();
    assert_eq!(cmds.len(), 2);
}

#[test]
fn test_undo_command_has_count_arg() {
    let cmd = UndoCommand;
    let args = cmd.args();
    assert!(!args.is_empty());
    assert_eq!(args[0].name, "count");
    assert_eq!(args[0].kind, ArgKind::Count);
}

#[test]
fn test_redo_command_has_count_arg() {
    let cmd = RedoCommand;
    let args = cmd.args();
    assert!(!args.is_empty());
    assert_eq!(args[0].name, "count");
    assert_eq!(args[0].kind, ArgKind::Count);
}

// =========================================================================
// Yank Command Tests
// =========================================================================

#[test]
fn test_yank_line_has_count_arg() {
    let cmd = YankLine;
    let args = cmd.args();
    assert!(!args.is_empty());
    assert_eq!(args[0].name, "count");
    assert_eq!(args[0].kind, ArgKind::Count);
}

#[test]
fn test_yank_line_no_buffer_returns_error() {
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let result = with_test_runtime(&mut session, &kernel, |runtime| {
        let args = CommandContext::new();
        YankLine.execute(runtime, &args)
    });
    assert!(result.is_error());
}

#[test]
fn test_yank_line_single_line() {
    let (kernel, buffer_id) = setup_buffer_context();
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = YankLine.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Check register content (per-client registers, #515)
    drop(runtime);
    let content = state.registers.get().clone();
    assert!(content.is_linewise());
    assert_eq!(content.text, "line one\n");
}

#[test]
fn test_yank_line_count() {
    let (kernel, buffer_id) = setup_buffer_context();
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));

    let result = YankLine.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Check register content (per-client registers, #515)
    drop(runtime);
    let content = state.registers.get().clone();
    assert!(content.is_linewise());
    assert_eq!(content.text, "line one\nline two\n");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_yank_line_at_eof() {
    let (kernel, buffer_id) = setup_buffer_context();
    let mut state = TestState::with_window(buffer_id);
    // Set cursor to last line
    if let Some(window) = state.windows.active_mut() {
        window.cursor = Position::new(2, 0).into();
    }

    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(5)); // More than remaining lines

    let result = YankLine.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Should only yank the last line (per-client registers, #515)
    drop(runtime);
    let content = state.registers.get().clone();
    assert!(content.is_linewise());
    assert_eq!(content.text, "line three\n");
}

#[test]
fn test_yank_commands_count() {
    let cmds = yank_commands();
    assert_eq!(cmds.len(), 1);
}

// =========================================================================
// Paste Command Tests
// =========================================================================

#[test]
fn test_paste_after_has_count_arg() {
    let cmd = PasteAfter;
    let args = cmd.args();
    assert!(!args.is_empty());
    assert_eq!(args[0].name, "count");
    assert_eq!(args[0].kind, ArgKind::Count);
}

#[test]
fn test_paste_before_has_count_arg() {
    let cmd = PasteBefore;
    let args = cmd.args();
    assert!(!args.is_empty());
    assert_eq!(args[0].name, "count");
    assert_eq!(args[0].kind, ArgKind::Count);
}

#[test]
fn test_paste_after_no_buffer_returns_error() {
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let result = with_test_runtime(&mut session, &kernel, |runtime| {
        let args = CommandContext::new();
        PasteAfter.execute(runtime, &args)
    });
    assert!(result.is_error());
}

#[test]
fn test_paste_before_no_buffer_returns_error() {
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let result = with_test_runtime(&mut session, &kernel, |runtime| {
        let args = CommandContext::new();
        PasteBefore.execute(runtime, &args)
    });
    assert!(result.is_error());
}

#[test]
fn test_paste_after_empty_register() {
    let (kernel, buffer_id) = setup_buffer_context();
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    // Register is empty by default
    let result = PasteAfter.execute(&mut runtime, &args);
    assert!(result.is_success()); // No-op, not error
}

#[test]
fn test_paste_after_linewise() {
    let (kernel, buffer_id) = setup_buffer_context();

    // Set linewise register content directly (per-client registers, #515)
    let mut state = TestState::with_window(buffer_id);
    state.registers.set(RegisterContent::linewise("line one\n"));
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = PasteAfter.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Check buffer content
    let buffer = kernel.buffers.get(buffer_id).unwrap();
    let buffer_read = buffer.read();
    let line_count = buffer_read.line_count();
    let line1 = buffer_read.line(1).map(str::to_owned);
    drop(buffer_read);
    assert_eq!(line_count, 4); // Original 3 + 1 pasted
    assert_eq!(line1.as_deref(), Some("line one")); // Pasted line
}

#[test]
fn test_paste_before_linewise() {
    let (kernel, buffer_id) = setup_buffer_context();

    // Set linewise register content directly (per-client registers, #515)
    let mut state = TestState::with_window(buffer_id);
    state.registers.set(RegisterContent::linewise("line one\n"));
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = PasteBefore.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Check buffer content
    let buffer = kernel.buffers.get(buffer_id).unwrap();
    let buffer_read = buffer.read();
    let line_count = buffer_read.line_count();
    let line0 = buffer_read.line(0).map(str::to_owned);
    drop(buffer_read);
    assert_eq!(line_count, 4); // Original 3 + 1 pasted
    assert_eq!(line0.as_deref(), Some("line one")); // Pasted line at top
}

#[test]
fn test_paste_after_characterwise() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = kernel.buffers.register(buffer);

    // Set characterwise content in per-client register (#515)
    let mut state = TestState::with_window(buffer_id);
    state.registers.set(RegisterContent::characterwise("XYZ"));
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = PasteAfter.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Check buffer content - "XYZ" pasted after cursor (position 0)
    let buffer = kernel.buffers.get(buffer_id).unwrap();
    let buffer_read = buffer.read();
    let content = buffer_read.line(0).map(str::to_owned);
    drop(buffer_read);
    assert_eq!(content.as_deref(), Some("hXYZello world"));
}

#[test]
fn test_paste_before_characterwise() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = kernel.buffers.register(buffer);

    // Set characterwise content in per-client register (#515)
    let mut state = TestState::with_window(buffer_id);
    state.registers.set(RegisterContent::characterwise("XYZ"));
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = PasteBefore.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Check buffer content - "XYZ" pasted at cursor (position 0)
    let buffer = kernel.buffers.get(buffer_id).unwrap();
    let buffer_read = buffer.read();
    let content = buffer_read.line(0).map(str::to_owned);
    drop(buffer_read);
    assert_eq!(content.as_deref(), Some("XYZhello world"));
}

#[test]
fn test_paste_after_count() {
    let kernel = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = kernel.buffers.register(buffer);

    // Set characterwise content in per-client register (#515)
    let mut state = TestState::with_window(buffer_id);
    state.registers.set(RegisterContent::characterwise("X"));
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(3));
    let result = PasteAfter.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Check buffer content - "XXX" pasted after cursor
    let buffer = kernel.buffers.get(buffer_id).unwrap();
    let buffer_read = buffer.read();
    let content = buffer_read.line(0).map(str::to_owned);
    drop(buffer_read);
    assert_eq!(content.as_deref(), Some("hXXXello"));
}

#[test]
fn test_paste_commands_count() {
    let cmds = paste_commands();
    assert_eq!(cmds.len(), 2);
}

// =========================================================================
// Replace Char Tests
// =========================================================================

#[test]
fn test_replace_char_start_has_count_arg() {
    let cmd = ReplaceCharStart;
    let args = cmd.args();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].name, "count");
}

#[test]
fn test_replace_char_start_returns_success() {
    // Commands now return Success - actual replace-char logic
    // will be handled by SessionRuntime (see #394)/vim resolver
    let (kernel, _buffer_id) = setup_buffer_context();
    let mut session = Session::new(ClientId::new(1), test_mode());
    let result = with_test_runtime(&mut session, &kernel, |runtime| {
        let args = CommandContext::new();
        ReplaceCharStart.execute(runtime, &args)
    });
    assert!(result.is_success());
}

#[test]
fn test_replace_char_start_with_count_returns_success() {
    let (kernel, _buffer_id) = setup_buffer_context();
    let mut session = Session::new(ClientId::new(1), test_mode());
    let result = with_test_runtime(&mut session, &kernel, |runtime| {
        let mut args = CommandContext::new();
        args.set("count", ArgValue::Count(3));
        ReplaceCharStart.execute(runtime, &args)
    });
    assert!(result.is_success());
}

// =========================================================================
// Repeat Dot Tests
// =========================================================================

#[test]
fn test_repeat_dot_has_count_arg() {
    let cmd = RepeatDot;
    let args = cmd.args();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].name, "count");
}

#[test]
fn test_repeat_dot_returns_success() {
    // Commands now return Success - actual repeat logic
    // will be handled by SessionRuntime (see #394)
    let (kernel, _buffer_id) = setup_buffer_context();
    let mut session = Session::new(ClientId::new(1), test_mode());
    let result = with_test_runtime(&mut session, &kernel, |runtime| {
        let args = CommandContext::new();
        RepeatDot.execute(runtime, &args)
    });
    assert!(result.is_success());
}

// =========================================================================
// Helper Function Tests
// =========================================================================

#[test]
fn test_insert_edit_commands_count() {
    let cmds = insert_edit_commands();
    assert_eq!(cmds.len(), 2);
}

#[test]
fn test_delete_commands_count() {
    let cmds = delete_commands();
    assert_eq!(cmds.len(), 5);
}

#[test]
fn test_mark_commands_count() {
    let cmds = mark_commands();
    assert_eq!(cmds.len(), 3);
}

#[test]
fn test_jump_commands_count() {
    let cmds = jump_commands();
    assert_eq!(cmds.len(), 2);
}
