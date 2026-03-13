use super::*;

use {
    reovim_driver_command::Command, reovim_driver_session::api::Selection,
    reovim_kernel::testing::create_test_context,
};

// ========================================================================
// Command metadata tests
// ========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_delete_selection_id() {
    let cmd = DeleteSelection;
    assert_eq!(cmd.id(), ids::DELETE_SELECTION);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_delete_selection_description() {
    let cmd = DeleteSelection;
    assert!(cmd.description().contains("Delete"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_yank_selection_id() {
    let cmd = YankSelection;
    assert_eq!(cmd.id(), ids::YANK_SELECTION);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_yank_selection_description() {
    let cmd = YankSelection;
    assert!(cmd.description().contains("Yank"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_selection_id() {
    let cmd = ChangeSelection;
    assert_eq!(cmd.id(), ids::CHANGE_SELECTION);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_selection_description() {
    let cmd = ChangeSelection;
    assert!(cmd.description().contains("Change"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_indent_selection_id() {
    let cmd = IndentSelection;
    assert_eq!(cmd.id(), ids::INDENT_SELECTION);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_indent_selection_description() {
    let cmd = IndentSelection;
    assert!(cmd.description().contains("Indent"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_dedent_selection_id() {
    let cmd = DedentSelection;
    assert_eq!(cmd.id(), ids::DEDENT_SELECTION);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_dedent_selection_description() {
    let cmd = DedentSelection;
    assert!(cmd.description().contains("Dedent"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_all_operator_commands_debug() {
    assert!(format!("{:?}", DeleteSelection).contains("DeleteSelection"));
    assert!(format!("{:?}", YankSelection).contains("YankSelection"));
    assert!(format!("{:?}", ChangeSelection).contains("ChangeSelection"));
    assert!(format!("{:?}", IndentSelection).contains("IndentSelection"));
    assert!(format!("{:?}", DedentSelection).contains("DedentSelection"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_all_operator_commands_default() {
    let _ = DeleteSelection;
    let _ = YankSelection;
    let _ = ChangeSelection;
    let _ = IndentSelection;
    let _ = DedentSelection;
}

// ========================================================================
// expand_selection_range tests
// ========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_expand_character_mode_passthrough() {
    let sel = Selection::character(Position::new(0, 2), Position::new(0, 8));
    let (start, end, is_linewise) = expand_selection_range(&sel, Some(10), 5);
    assert_eq!(start, Position::new(0, 2));
    assert_eq!(end, Position::new(0, 8));
    assert!(!is_linewise);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_expand_block_mode_passthrough() {
    let sel = Selection::block(Position::new(1, 3), Position::new(3, 7));
    let (start, end, is_linewise) = expand_selection_range(&sel, Some(10), 5);
    assert_eq!(start, Position::new(1, 3));
    assert_eq!(end, Position::new(3, 7));
    assert!(!is_linewise);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_expand_line_mode_expands_to_full_lines() {
    let sel = Selection::line(Position::new(0, 3), Position::new(2, 0));
    let (start, end, is_linewise) = expand_selection_range(&sel, Some(10), 5);
    assert_eq!(start, Position::new(0, 0));
    assert_eq!(end, Position::new(2, 0));
    assert!(is_linewise);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_expand_line_mode_end_past_buffer() {
    let sel = Selection::line(Position::new(0, 0), Position::new(5, 0));
    let (start, end, is_linewise) = expand_selection_range(&sel, Some(8), 3);
    assert_eq!(start, Position::new(0, 0));
    assert_eq!(end, Position::new(4, 8));
    assert!(is_linewise);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_expand_line_mode_end_at_buffer_boundary() {
    let sel = Selection::line(Position::new(0, 0), Position::new(3, 0));
    let (start, end, is_linewise) = expand_selection_range(&sel, Some(5), 3);
    assert_eq!(start, Position::new(0, 0));
    assert_eq!(end, Position::new(2, 5));
    assert!(is_linewise);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_expand_line_mode_within_buffer() {
    let sel = Selection::line(Position::new(1, 0), Position::new(3, 0));
    let (start, end, is_linewise) = expand_selection_range(&sel, Some(10), 10);
    assert_eq!(start, Position::new(1, 0));
    assert_eq!(end, Position::new(3, 0));
    assert!(is_linewise);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_expand_line_mode_none_end_line_len() {
    let sel = Selection::line(Position::new(0, 0), Position::new(5, 0));
    let (start, end, is_linewise) = expand_selection_range(&sel, None, 3);
    assert_eq!(start, Position::new(0, 0));
    // end_line_len is None => 0
    assert_eq!(end, Position::new(4, 0));
    assert!(is_linewise);
}

// ========================================================================
// Execute tests (require SessionRuntime)
// ========================================================================

use {
    reovim_driver_command::CommandHandler,
    reovim_driver_session::{
        ClientId, ExtensionMap, Session, SessionRuntime, WindowLayout, api::RegisterApi,
        testing::StubExecutor,
    },
    reovim_kernel::api::{
        ModeStack,
        v1::{
            Buffer, BufferId, HistoryRing, KernelContext, MarkBank, ModeId, ModuleId, RegisterBank,
        },
    },
};

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
                active_buffer: &mut self.active_buffer,
                terminal_size: &mut self.terminal_size,
            },
            kernel,
            &StubExecutor,
        )
    }
}

// --- DeleteSelection execute ---

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_delete_selection_execute_no_buffer() {
    let ctx = create_test_context();
    let args = CommandContext::new(); // No buffer_id set
    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);

    let result = DeleteSelection.execute(&mut runtime, &args);
    assert!(matches!(result, CommandResult::Error(_)));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_delete_selection_execute_no_selection() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    // No selection - should be no-op
    let result = DeleteSelection.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_delete_selection_execute_character_mode() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let mut state = TestState::with_buffer(Some(buffer_id));
    // Set character selection for "hello" (columns 0-5, exclusive end)
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::character(Position::new(0, 0), Position::new(0, 5)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = DeleteSelection.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Selection should be cleared
    let window = runtime.windows().active().unwrap();
    assert!(window.selection.is_none());

    // Cursor should be at start of deletion
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 0);

    // Register should contain deleted text
    let reg = runtime.get_register(None);
    assert!(reg.is_some());
    let reg = reg.unwrap();
    assert!(reg.is_characterwise());
    assert_eq!(reg.text, "hello");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_delete_selection_execute_clears_selection() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("abcdef");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::character(Position::new(0, 1), Position::new(0, 4)));
    }
    let mut runtime = state.runtime(&ctx);

    DeleteSelection.execute(&mut runtime, &args);

    let window = runtime.windows().active().unwrap();
    assert!(window.selection.is_none());
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 1);
}

// --- YankSelection execute ---

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_yank_selection_execute_no_buffer() {
    let ctx = create_test_context();
    let args = CommandContext::new();
    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);

    let result = YankSelection.execute(&mut runtime, &args);
    assert!(matches!(result, CommandResult::Error(_)));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_yank_selection_execute_no_selection() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    let result = YankSelection.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_yank_selection_execute_character_mode() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::character(Position::new(0, 0), Position::new(0, 5)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = YankSelection.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Selection should be cleared
    let window = runtime.windows().active().unwrap();
    assert!(window.selection.is_none());

    // Register should contain yanked text
    let reg = runtime.get_register(None);
    assert!(reg.is_some());
    let reg = reg.unwrap();
    assert!(reg.is_characterwise());
    assert_eq!(reg.text, "hello");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_yank_selection_execute_does_not_delete_text() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::character(Position::new(0, 0), Position::new(0, 5)));
    }
    let mut runtime = state.runtime(&ctx);

    YankSelection.execute(&mut runtime, &args);

    // Buffer should still contain original text (yank doesn't delete)
    let line = runtime.buffer_line(buffer_id, 0);
    assert_eq!(line, Some("hello world".to_string()));
}

// --- ChangeSelection execute ---

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_selection_execute_no_buffer() {
    let ctx = create_test_context();
    let args = CommandContext::new();
    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);

    let result = ChangeSelection.execute(&mut runtime, &args);
    assert!(matches!(result, CommandResult::Error(_)));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_selection_execute_no_selection() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    let result = ChangeSelection.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_selection_execute_character_mode() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::character(Position::new(0, 0), Position::new(0, 5)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = ChangeSelection.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Selection should be cleared
    let window = runtime.windows().active().unwrap();
    assert!(window.selection.is_none());

    // Cursor at start of deletion
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 0);

    // Register should contain deleted text
    let reg = runtime.get_register(None);
    assert!(reg.is_some());
    let reg = reg.unwrap();
    assert!(reg.is_characterwise());
    assert_eq!(reg.text, "hello");
}

// --- IndentSelection execute ---

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_indent_selection_execute_no_buffer() {
    let ctx = create_test_context();
    let args = CommandContext::new();
    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);

    let result = IndentSelection.execute(&mut runtime, &args);
    assert!(matches!(result, CommandResult::Error(_)));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_indent_selection_execute_no_selection() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    let result = IndentSelection.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_indent_selection_execute_indents_lines() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld\nfoo");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let mut state = TestState::with_buffer(Some(buffer_id));
    // Select lines 0..2 (first two lines)
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::line(Position::new(0, 0), Position::new(2, 0)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = IndentSelection.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Lines 0 and 1 should be indented with 4 spaces
    let line0 = runtime.buffer_line(buffer_id, 0);
    assert_eq!(line0, Some("    hello".to_string()));
    let line1 = runtime.buffer_line(buffer_id, 1);
    assert_eq!(line1, Some("    world".to_string()));
    // Line 2 should be unchanged
    let line2 = runtime.buffer_line(buffer_id, 2);
    assert_eq!(line2, Some("foo".to_string()));

    // Selection should be cleared
    let window = runtime.windows().active().unwrap();
    assert!(window.selection.is_none());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_indent_selection_execute_single_line() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::line(Position::new(1, 0), Position::new(2, 0)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = IndentSelection.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Only line 1 should be indented
    let line0 = runtime.buffer_line(buffer_id, 0);
    assert_eq!(line0, Some("hello".to_string()));
    let line1 = runtime.buffer_line(buffer_id, 1);
    assert_eq!(line1, Some("    world".to_string()));
}

// --- DedentSelection execute ---

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_dedent_selection_execute_no_buffer() {
    let ctx = create_test_context();
    let args = CommandContext::new();
    let mut state = TestState::with_buffer(None);
    let mut runtime = state.runtime(&ctx);

    let result = DedentSelection.execute(&mut runtime, &args);
    assert!(matches!(result, CommandResult::Error(_)));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_dedent_selection_execute_no_selection() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("    hello\n    world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    let result = DedentSelection.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_dedent_selection_execute_removes_spaces() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("    hello\n    world\nfoo");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::line(Position::new(0, 0), Position::new(2, 0)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = DedentSelection.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Lines should be dedented (4 spaces removed)
    let line0 = runtime.buffer_line(buffer_id, 0);
    assert_eq!(line0, Some("hello".to_string()));
    let line1 = runtime.buffer_line(buffer_id, 1);
    assert_eq!(line1, Some("world".to_string()));
    // Line 2 unchanged
    let line2 = runtime.buffer_line(buffer_id, 2);
    assert_eq!(line2, Some("foo".to_string()));

    // Selection should be cleared
    let window = runtime.windows().active().unwrap();
    assert!(window.selection.is_none());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_dedent_selection_execute_removes_tab() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("\thello\n\tworld");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::line(Position::new(0, 0), Position::new(2, 0)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = DedentSelection.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Tabs should be removed
    let line0 = runtime.buffer_line(buffer_id, 0);
    assert_eq!(line0, Some("hello".to_string()));
    let line1 = runtime.buffer_line(buffer_id, 1);
    assert_eq!(line1, Some("world".to_string()));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_dedent_selection_execute_no_leading_whitespace() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::line(Position::new(0, 0), Position::new(2, 0)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = DedentSelection.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Lines with no leading whitespace should remain unchanged
    let line0 = runtime.buffer_line(buffer_id, 0);
    assert_eq!(line0, Some("hello".to_string()));
    let line1 = runtime.buffer_line(buffer_id, 1);
    assert_eq!(line1, Some("world".to_string()));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_dedent_selection_execute_partial_spaces() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("  hello\n      world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::line(Position::new(0, 0), Position::new(2, 0)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = DedentSelection.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // "  hello" has 2 spaces -> removed, "      world" has 6 spaces -> 4 removed
    let line0 = runtime.buffer_line(buffer_id, 0);
    assert_eq!(line0, Some("hello".to_string()));
    let line1 = runtime.buffer_line(buffer_id, 1);
    assert_eq!(line1, Some("  world".to_string()));
}

// ========================================================================
// Additional expand_selection_range edge cases
// ========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_expand_character_mode_single_char() {
    let sel = Selection::character(Position::new(0, 0), Position::new(0, 1));
    let (start, end, is_linewise) = expand_selection_range(&sel, Some(10), 5);
    assert_eq!(start, Position::new(0, 0));
    assert_eq!(end, Position::new(0, 1));
    assert!(!is_linewise);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_expand_line_mode_single_line() {
    let sel = Selection::line(Position::new(2, 0), Position::new(3, 0));
    let (start, end, is_linewise) = expand_selection_range(&sel, Some(10), 5);
    assert_eq!(start, Position::new(2, 0));
    assert_eq!(end, Position::new(3, 0));
    assert!(is_linewise);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_expand_block_mode_single_cell() {
    let sel = Selection::block(Position::new(0, 0), Position::new(0, 1));
    let (start, end, is_linewise) = expand_selection_range(&sel, Some(10), 5);
    assert_eq!(start, Position::new(0, 0));
    assert_eq!(end, Position::new(0, 1));
    assert!(!is_linewise);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_expand_character_mode_multiline() {
    let sel = Selection::character(Position::new(0, 3), Position::new(2, 5));
    let (start, end, is_linewise) = expand_selection_range(&sel, Some(10), 5);
    assert_eq!(start, Position::new(0, 3));
    assert_eq!(end, Position::new(2, 5));
    assert!(!is_linewise);
}

// ========================================================================
// Additional operator execute edge cases
// ========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_delete_selection_middle_of_line() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::character(Position::new(0, 5), Position::new(0, 11)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = DeleteSelection.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let line = runtime.buffer_line(buffer_id, 0);
    assert_eq!(line, Some("hello".to_string()));

    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.column, 5);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_yank_selection_line_mode() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld\nfoo");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::line(Position::new(0, 0), Position::new(2, 0)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = YankSelection.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Buffer should be unchanged
    assert_eq!(runtime.buffer_line(buffer_id, 0), Some("hello".to_string()));
    assert_eq!(runtime.buffer_line(buffer_id, 1), Some("world".to_string()));

    // Register should contain yanked lines
    let reg = runtime.get_register(None);
    assert!(reg.is_some());
    let reg = reg.unwrap();
    assert!(reg.is_linewise());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_selection_enters_insert_mode() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::character(Position::new(0, 0), Position::new(0, 5)));
    }
    let mut runtime = state.runtime(&ctx);

    ChangeSelection.execute(&mut runtime, &args);

    // Should enter insert mode
    assert_eq!(runtime.current_mode().name(), crate::modes::VimMode::INSERT_ID.name());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_indent_selection_already_indented() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("    hello\n    world");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::line(Position::new(0, 0), Position::new(2, 0)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = IndentSelection.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Lines should get additional 4 spaces
    let line0 = runtime.buffer_line(buffer_id, 0);
    assert_eq!(line0, Some("        hello".to_string()));
    let line1 = runtime.buffer_line(buffer_id, 1);
    assert_eq!(line1, Some("        world".to_string()));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_dedent_selection_single_space() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string(" hello");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::line(Position::new(0, 0), Position::new(1, 0)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = DedentSelection.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let line0 = runtime.buffer_line(buffer_id, 0);
    assert_eq!(line0, Some("hello".to_string()));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_delete_selection_clones() {
    let _: DeleteSelection = DeleteSelection;
    let _: YankSelection = YankSelection;
    let _: ChangeSelection = ChangeSelection;
    let _: IndentSelection = IndentSelection;
    let _: DedentSelection = DedentSelection;
}

// ========================================================================
// Coverage: None-path and linewise tests
// ========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_delete_selection_nonexistent_buffer() {
    // Exercise the None path of buffer_text_range (line 107)
    let ctx = create_test_context();
    let fake_id = BufferId::from_raw(9999);

    let mut args = CommandContext::new();
    args.set_buffer_id(fake_id);
    let mut state = TestState::with_buffer(Some(fake_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::character(Position::new(0, 0), Position::new(0, 5)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = DeleteSelection.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_yank_selection_nonexistent_buffer() {
    // Exercise the None path of buffer_text_range (line 169)
    let ctx = create_test_context();
    let fake_id = BufferId::from_raw(9999);

    let mut args = CommandContext::new();
    args.set_buffer_id(fake_id);
    let mut state = TestState::with_buffer(Some(fake_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::character(Position::new(0, 0), Position::new(0, 5)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = YankSelection.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_selection_nonexistent_buffer() {
    // Exercise the None path of buffer_text_range (line 227)
    let ctx = create_test_context();
    let fake_id = BufferId::from_raw(9999);

    let mut args = CommandContext::new();
    args.set_buffer_id(fake_id);
    let mut state = TestState::with_buffer(Some(fake_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::character(Position::new(0, 0), Position::new(0, 5)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = ChangeSelection.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_change_selection_linewise() {
    // Exercise the is_linewise branch in ChangeSelection (line 222)
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld\nfoo");
    let buffer_id = ctx.buffers.register(buffer);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::line(Position::new(0, 0), Position::new(2, 0)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = ChangeSelection.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Register should contain linewise content
    let reg = runtime.get_register(None);
    assert!(reg.is_some());
    let reg = reg.unwrap();
    assert!(reg.is_linewise());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_dedent_selection_nonexistent_buffer() {
    // Exercise the None path of buffer_line (line 348)
    let ctx = create_test_context();
    let fake_id = BufferId::from_raw(9999);

    let mut args = CommandContext::new();
    args.set_buffer_id(fake_id);
    let mut state = TestState::with_buffer(Some(fake_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::line(Position::new(0, 0), Position::new(3, 0)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = DedentSelection.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}
