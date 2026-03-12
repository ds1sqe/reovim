#![allow(clippy::significant_drop_tightening, clippy::uninlined_format_args)]
use {
    super::super::*,
    crate::ids,
    reovim_driver_command::{CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::SessionRuntime,
    reovim_kernel::api::v1::Position,
};

use {reovim_driver_command::Command, reovim_kernel::testing::create_test_context};

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_swap_anchor_id() {
    let cmd = SwapAnchor;
    assert_eq!(cmd.id(), ids::VISUAL_SWAP_ANCHOR);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_swap_anchor_description() {
    let cmd = SwapAnchor;
    assert!(cmd.description().contains("Swap"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_toggle_visual_char_id() {
    let cmd = ToggleVisualChar;
    assert_eq!(cmd.id(), ids::TOGGLE_VISUAL_CHAR);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_toggle_visual_char_description() {
    let cmd = ToggleVisualChar;
    assert!(cmd.description().contains("character"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_toggle_visual_line_id() {
    let cmd = ToggleVisualLine;
    assert_eq!(cmd.id(), ids::TOGGLE_VISUAL_LINE);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_toggle_visual_line_description() {
    let cmd = ToggleVisualLine;
    assert!(cmd.description().contains("line"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_toggle_visual_block_id() {
    let cmd = ToggleVisualBlock;
    assert_eq!(cmd.id(), ids::TOGGLE_VISUAL_BLOCK);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_toggle_visual_block_description() {
    let cmd = ToggleVisualBlock;
    assert!(cmd.description().contains("block"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_reselect_last_id() {
    let cmd = ReselectLast;
    assert_eq!(cmd.id(), ids::RESELECT_LAST);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_reselect_last_description() {
    let cmd = ReselectLast;
    assert!(cmd.description().contains("Reselect"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_all_manipulation_commands_debug() {
    assert!(format!("{:?}", SwapAnchor).contains("SwapAnchor"));
    assert!(format!("{:?}", ToggleVisualChar).contains("ToggleVisualChar"));
    assert!(format!("{:?}", ToggleVisualLine).contains("ToggleVisualLine"));
    assert!(format!("{:?}", ToggleVisualBlock).contains("ToggleVisualBlock"));
    assert!(format!("{:?}", ReselectLast).contains("ReselectLast"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_all_manipulation_commands_default() {
    let _ = SwapAnchor;
    let _ = ToggleVisualChar;
    let _ = ToggleVisualLine;
    let _ = ToggleVisualBlock;
    let _ = ReselectLast;
}

// ========================================================================
// Execute tests
// ========================================================================

use {
    reovim_driver_session::{
        ClientId, ExtensionMap, Session, WindowLayout,
        api::{Selection, SelectionMode},
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
    compositor: Option<Box<dyn reovim_driver_display::layout::RootCompositor>>,
    tabs: reovim_driver_session::TabPageSet,
    registers: RegisterBank,
    clipboard_history: HistoryRing,
    local_marks: MarkBank,
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

// --- SwapAnchor execute ---

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_swap_anchor_execute_swaps_start_and_end() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::character(Position::new(0, 2), Position::new(0, 8)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = SwapAnchor.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    let sel = window.selection.as_ref().unwrap();
    assert_eq!(sel.start, Position::new(0, 8));
    assert_eq!(sel.end, Position::new(0, 2));
    assert_eq!(sel.mode, SelectionMode::Character);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_swap_anchor_execute_no_selection() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    // No selection - should be no-op
    let result = SwapAnchor.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    assert!(window.selection.is_none());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_swap_anchor_execute_preserves_mode() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::line(Position::new(0, 0), Position::new(2, 0)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = SwapAnchor.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    let sel = window.selection.as_ref().unwrap();
    assert_eq!(sel.start, Position::new(2, 0));
    assert_eq!(sel.end, Position::new(0, 0));
    assert_eq!(sel.mode, SelectionMode::Line);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_swap_anchor_execute_block_mode() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::block(Position::new(0, 1), Position::new(1, 4)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = SwapAnchor.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    let sel = window.selection.as_ref().unwrap();
    assert_eq!(sel.start, Position::new(1, 4));
    assert_eq!(sel.end, Position::new(0, 1));
    assert_eq!(sel.mode, SelectionMode::Block);
}

// --- ToggleVisualChar execute ---

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_toggle_visual_char_execute_exit_when_already_char() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::character(Position::new(0, 0), Position::new(0, 5)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = ToggleVisualChar.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Selection should be cleared (exit visual mode)
    let window = runtime.windows().active().unwrap();
    assert!(window.selection.is_none());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_toggle_visual_char_execute_switch_from_line() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::line(Position::new(0, 0), Position::new(2, 0)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = ToggleVisualChar.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    // Selection should be changed to character mode
    let window = runtime.windows().active().unwrap();
    let sel = window.selection.as_ref().unwrap();
    assert_eq!(sel.mode, SelectionMode::Character);
    // Positions preserved
    assert_eq!(sel.start, Position::new(0, 0));
    assert_eq!(sel.end, Position::new(2, 0));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_toggle_visual_char_execute_switch_from_block() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::block(Position::new(0, 1), Position::new(1, 3)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = ToggleVisualChar.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    let sel = window.selection.as_ref().unwrap();
    assert_eq!(sel.mode, SelectionMode::Character);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_toggle_visual_char_execute_no_selection() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    let result = ToggleVisualChar.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    assert!(window.selection.is_none());
}

// --- ToggleVisualLine execute ---

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_toggle_visual_line_execute_exit_when_already_line() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::line(Position::new(0, 0), Position::new(2, 0)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = ToggleVisualLine.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    assert!(window.selection.is_none());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_toggle_visual_line_execute_switch_from_char() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::character(Position::new(0, 2), Position::new(0, 8)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = ToggleVisualLine.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    let sel = window.selection.as_ref().unwrap();
    assert_eq!(sel.mode, SelectionMode::Line);
    assert_eq!(sel.start, Position::new(0, 2));
    assert_eq!(sel.end, Position::new(0, 8));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_toggle_visual_line_execute_switch_from_block() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::block(Position::new(0, 1), Position::new(1, 3)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = ToggleVisualLine.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    let sel = window.selection.as_ref().unwrap();
    assert_eq!(sel.mode, SelectionMode::Line);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_toggle_visual_line_execute_no_selection() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    let result = ToggleVisualLine.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    assert!(window.selection.is_none());
}

// --- ToggleVisualBlock execute ---

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_toggle_visual_block_execute_exit_when_already_block() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::block(Position::new(0, 0), Position::new(1, 3)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = ToggleVisualBlock.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    assert!(window.selection.is_none());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_toggle_visual_block_execute_switch_from_char() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::character(Position::new(0, 0), Position::new(0, 5)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = ToggleVisualBlock.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    let sel = window.selection.as_ref().unwrap();
    assert_eq!(sel.mode, SelectionMode::Block);
    assert_eq!(sel.start, Position::new(0, 0));
    assert_eq!(sel.end, Position::new(0, 5));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_toggle_visual_block_execute_switch_from_line() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::line(Position::new(0, 0), Position::new(2, 0)));
    }
    let mut runtime = state.runtime(&ctx);

    let result = ToggleVisualBlock.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    let sel = window.selection.as_ref().unwrap();
    assert_eq!(sel.mode, SelectionMode::Block);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_toggle_visual_block_execute_no_selection() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    let result = ToggleVisualBlock.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);

    let window = runtime.windows().active().unwrap();
    assert!(window.selection.is_none());
}

// --- ReselectLast execute ---

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_reselect_last_execute_returns_success() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    let mut runtime = state.runtime(&ctx);

    let result = ReselectLast.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}

// --- Additional toggle tests ---

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_swap_anchor_double_swap_restores() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::character(Position::new(0, 2), Position::new(0, 8)));
    }
    let mut runtime = state.runtime(&ctx);

    // Swap once
    SwapAnchor.execute(&mut runtime, &args);

    // Swap again - should restore to original
    SwapAnchor.execute(&mut runtime, &args);

    let window = runtime.windows().active().unwrap();
    let sel = window.selection.as_ref().unwrap();
    assert_eq!(sel.start, Position::new(0, 2));
    assert_eq!(sel.end, Position::new(0, 8));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_toggle_visual_char_from_block_preserves_positions() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::block(Position::new(0, 1), Position::new(1, 4)));
    }
    let mut runtime = state.runtime(&ctx);

    ToggleVisualChar.execute(&mut runtime, &args);

    let window = runtime.windows().active().unwrap();
    let sel = window.selection.as_ref().unwrap();
    assert_eq!(sel.mode, SelectionMode::Character);
    assert_eq!(sel.start, Position::new(0, 1));
    assert_eq!(sel.end, Position::new(1, 4));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_toggle_visual_line_from_char_preserves_positions() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::character(Position::new(0, 3), Position::new(1, 2)));
    }
    let mut runtime = state.runtime(&ctx);

    ToggleVisualLine.execute(&mut runtime, &args);

    let window = runtime.windows().active().unwrap();
    let sel = window.selection.as_ref().unwrap();
    assert_eq!(sel.mode, SelectionMode::Line);
    assert_eq!(sel.start, Position::new(0, 3));
    assert_eq!(sel.end, Position::new(1, 2));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_toggle_visual_block_from_line_preserves_positions() {
    let ctx = create_test_context();
    let buffer = Buffer::from_string("hello\nworld");
    let buffer_id = ctx.buffers.register(buffer);

    let args = CommandContext::new();
    let mut state = TestState::with_buffer(Some(buffer_id));
    if let Some(w) = state.windows.active_mut() {
        w.selection = Some(Selection::line(Position::new(0, 0), Position::new(2, 0)));
    }
    let mut runtime = state.runtime(&ctx);

    ToggleVisualBlock.execute(&mut runtime, &args);

    let window = runtime.windows().active().unwrap();
    let sel = window.selection.as_ref().unwrap();
    assert_eq!(sel.mode, SelectionMode::Block);
    assert_eq!(sel.start, Position::new(0, 0));
    assert_eq!(sel.end, Position::new(2, 0));
}
