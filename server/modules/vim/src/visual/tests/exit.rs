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
fn test_exit_visual_mode_id() {
    let cmd = ExitVisualMode;
    assert_eq!(cmd.id(), ids::EXIT_VISUAL);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_visual_mode_description() {
    let cmd = ExitVisualMode;
    assert!(cmd.description().contains("visual"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_visual_mode_debug() {
    let debug = format!("{:?}", ExitVisualMode);
    assert!(debug.contains("ExitVisualMode"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_visual_mode_default() {
    let _ = ExitVisualMode;
}

// ========================================================================
// Execute tests
// ========================================================================

use {
    reovim_driver_session::{
        ClientId, ExtensionMap, Session, WindowLayout, api::Selection, testing::StubExecutor,
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

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_visual_mode_execute_clears_selection() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));

    // Set up a selection first
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::character(Position::new(0, 0), Position::new(0, 5)));
    }

    let mut runtime = state.runtime(&ctx);
    let result = ExitVisualMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Selection should be cleared
    let window = runtime.windows().active().unwrap();
    assert!(window.selection.is_none());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_visual_mode_execute_no_selection() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    // Even without selection, exit should succeed
    let result = ExitVisualMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    assert!(window.selection.is_none());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_visual_mode_execute_clears_line_selection() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::line(Position::new(0, 0), Position::new(2, 0)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = ExitVisualMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    assert!(window.selection.is_none());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_visual_mode_execute_clears_block_selection() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::block(Position::new(0, 1), Position::new(1, 3)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = ExitVisualMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    assert!(window.selection.is_none());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_visual_mode_sets_normal_mode() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::character(Position::new(0, 0), Position::new(0, 3)));
    }
    let mut runtime = state.runtime(&ctx);

    ExitVisualMode.execute(&mut runtime, &args);

    assert_eq!(runtime.current_mode().name(), crate::modes::VimMode::NORMAL_ID.name());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_visual_mode_clone_copy() {
    let cmd = ExitVisualMode;
    let _: ExitVisualMode = cmd;
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_visual_mode_description_contains_normal() {
    let cmd = ExitVisualMode;
    assert!(cmd.description().contains("normal"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_visual_mode_preserves_cursor() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.cursor = Position::new(0, 5).into();
        w.selection = Some(Selection::character(Position::new(0, 2), Position::new(0, 8)));
    }
    let mut runtime = state.runtime(&ctx);

    ExitVisualMode.execute(&mut runtime, &args);

    // Cursor should be preserved at its position
    let window = runtime.windows().active().unwrap();
    assert_eq!(window.cursor.line, 0);
    assert_eq!(window.cursor.column, 5);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_exit_visual_mode_no_window() {
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

    // Even without a window, exit should succeed
    let result = ExitVisualMode.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}
