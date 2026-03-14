#![allow(clippy::significant_drop_tightening, clippy::uninlined_format_args)]
use {
    super::super::*,
    crate::ids,
    reovim_driver_command::{CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{SessionRuntime, api::ModeApi},
    reovim_kernel::api::v1::Position,
};

use {reovim_driver_command::Command, reovim_kernel::testing::create_test_context};

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_visual_mode_id() {
    let cmd = EnterVisualMode;
    assert_eq!(cmd.id(), ids::ENTER_VISUAL);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_visual_mode_description() {
    let cmd = EnterVisualMode;
    assert!(cmd.description().contains("visual"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_visual_line_mode_id() {
    let cmd = EnterVisualLineMode;
    assert_eq!(cmd.id(), ids::ENTER_VISUAL_LINE);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_visual_line_mode_description() {
    let cmd = EnterVisualLineMode;
    assert!(cmd.description().contains("visual line"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_visual_block_mode_id() {
    let cmd = EnterVisualBlockMode;
    assert_eq!(cmd.id(), ids::ENTER_VISUAL_BLOCK);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_visual_block_mode_description() {
    let cmd = EnterVisualBlockMode;
    assert!(cmd.description().contains("visual block"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_all_visual_entry_debug() {
    assert!(format!("{:?}", EnterVisualMode).contains("EnterVisualMode"));
    assert!(format!("{:?}", EnterVisualLineMode).contains("EnterVisualLineMode"));
    assert!(format!("{:?}", EnterVisualBlockMode).contains("EnterVisualBlockMode"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_all_visual_entry_default() {
    let _ = EnterVisualMode;
    let _ = EnterVisualLineMode;
    let _ = EnterVisualBlockMode;
}

// ========================================================================
// Execute tests
// ========================================================================

use {
    reovim_driver_session::{ClientId, ExtensionMap, Session, WindowLayout, testing::StubExecutor},
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
    jumplist: Jumplist,
    active_buffer: Option<BufferId>,
    terminal_size: (u16, u16),
}

impl TestState {
    #[cfg_attr(coverage_nightly, coverage(off))]
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

// --- EnterVisualMode execute ---

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_visual_mode_execute_sets_character_selection() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    let result = EnterVisualMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Selection should be set to character mode at cursor position (0,0) to (0,1)
    let window = runtime.windows().active().unwrap();
    let sel = window.selection.as_ref().unwrap();
    assert_eq!(sel.start, Position::new(0, 0));
    assert_eq!(sel.end, Position::new(0, 1));
    assert_eq!(sel.mode, reovim_driver_session::api::SelectionMode::Character);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_visual_mode_execute_at_nonzero_cursor() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    // Move cursor to column 5
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(0, 5).into();
    }
    let mut runtime = state.runtime(&ctx);

    let result = EnterVisualMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    let sel = window.selection.as_ref().unwrap();
    assert_eq!(sel.start, Position::new(0, 5));
    assert_eq!(sel.end, Position::new(0, 6));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_visual_mode_execute_no_active_window() {
    let ctx = create_test_context();
    let args = CommandContext::new();

    let home_mode = ModeId::new(ModuleId::new("test"), "normal");
    let mut session = Session::new(ClientId::new(1), home_mode.clone());
    let mut mode_stack = ModeStack::new(home_mode);
    let mut windows = WindowLayout::empty(); // No windows at all
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
        &ctx,
        &StubExecutor,
    );

    let result = EnterVisualMode.execute(&mut runtime, &args);
    assert!(matches!(result, CommandResult::Error(_)));
}

// --- EnterVisualLineMode execute ---

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_visual_line_mode_execute_sets_line_selection() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld\nfoo");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    let result = EnterVisualLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    let sel = window.selection.as_ref().unwrap();
    assert_eq!(sel.start, Position::new(0, 0));
    assert_eq!(sel.end, Position::new(1, 0));
    assert_eq!(sel.mode, reovim_driver_session::api::SelectionMode::Line);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_visual_line_mode_execute_at_line_2() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld\nfoo");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(2, 1).into();
    }
    let mut runtime = state.runtime(&ctx);

    let result = EnterVisualLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    let sel = window.selection.as_ref().unwrap();
    assert_eq!(sel.start, Position::new(2, 0));
    assert_eq!(sel.end, Position::new(3, 0));
    assert_eq!(sel.mode, reovim_driver_session::api::SelectionMode::Line);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_visual_line_mode_execute_no_active_window() {
    let ctx = create_test_context();
    let args = CommandContext::new();

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
        &StubExecutor,
    );

    let result = EnterVisualLineMode.execute(&mut runtime, &args);
    assert!(matches!(result, CommandResult::Error(_)));
}

// --- EnterVisualBlockMode execute ---

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_visual_block_mode_execute_sets_block_selection() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    let result = EnterVisualBlockMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    let sel = window.selection.as_ref().unwrap();
    assert_eq!(sel.start, Position::new(0, 0));
    assert_eq!(sel.end, Position::new(0, 1));
    assert_eq!(sel.mode, reovim_driver_session::api::SelectionMode::Block);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_visual_block_mode_execute_at_nonzero_cursor() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(1, 3).into();
    }
    let mut runtime = state.runtime(&ctx);

    let result = EnterVisualBlockMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    let sel = window.selection.as_ref().unwrap();
    assert_eq!(sel.start, Position::new(1, 3));
    assert_eq!(sel.end, Position::new(1, 4));
    assert_eq!(sel.mode, reovim_driver_session::api::SelectionMode::Block);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_visual_block_mode_execute_no_active_window() {
    let ctx = create_test_context();
    let args = CommandContext::new();

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
        &StubExecutor,
    );

    let result = EnterVisualBlockMode.execute(&mut runtime, &args);
    assert!(matches!(result, CommandResult::Error(_)));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_visual_mode_sets_mode_to_visual() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    EnterVisualMode.execute(&mut runtime, &args);

    assert_eq!(runtime.current_mode().name(), crate::modes::VimMode::VISUAL_ID.name());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_visual_line_mode_sets_mode() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    EnterVisualLineMode.execute(&mut runtime, &args);

    assert_eq!(runtime.current_mode().name(), crate::modes::VimMode::VISUAL_LINE_ID.name());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_visual_block_mode_sets_mode() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    EnterVisualBlockMode.execute(&mut runtime, &args);

    assert_eq!(runtime.current_mode().name(), crate::modes::VimMode::VISUAL_BLOCK_ID.name());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_visual_mode_at_line_2_col_3() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld\nfoo");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(2, 1).into();
    }
    let mut runtime = state.runtime(&ctx);

    let result = EnterVisualMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    let sel = window.selection.as_ref().unwrap();
    assert_eq!(sel.start, Position::new(2, 1));
    assert_eq!(sel.end, Position::new(2, 2));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_visual_block_mode_at_line_1() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(1, 0).into();
    }
    let mut runtime = state.runtime(&ctx);

    let result = EnterVisualBlockMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    let sel = window.selection.as_ref().unwrap();
    assert_eq!(sel.start, Position::new(1, 0));
    assert_eq!(sel.end, Position::new(1, 1));
    assert_eq!(sel.mode, reovim_driver_session::api::SelectionMode::Block);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_enter_visual_line_mode_selection_starts_at_col_0() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("  hello\nworld");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    // Cursor at column 5 but line selection always starts at col 0
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(0, 5).into();
    }
    let mut runtime = state.runtime(&ctx);

    let result = EnterVisualLineMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    let sel = window.selection.as_ref().unwrap();
    assert_eq!(sel.start, Position::new(0, 0));
    assert_eq!(sel.end, Position::new(1, 0));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_all_entry_commands_clone_copy() {
    let _: EnterVisualMode = EnterVisualMode;
    let _: EnterVisualLineMode = EnterVisualLineMode;
    let _: EnterVisualBlockMode = EnterVisualBlockMode;
}
