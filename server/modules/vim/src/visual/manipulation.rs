//! Selection manipulation commands.
//!
//! Provides commands for manipulating the visual selection:
//! - `o` - Swap cursor and anchor positions
//! - `v` (in visual mode) - Toggle to character-wise mode
//! - `V` (in visual mode) - Toggle to line-wise mode
//! - `Ctrl-V` (in visual mode) - Toggle to block mode
//! - `gv` - Reselect last visual selection

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{
            SessionRuntime, TransitionContext,
        api::{ModeApi, SelectionMode},
    },
    reovim_kernel::api::v1::CommandId,
};

use crate::{ids, modes::VimMode};

/// Swap cursor and anchor positions (o in visual mode).
///
/// In Vim, pressing 'o' in visual mode swaps the cursor position with the
/// anchor position, allowing you to adjust the other end of the selection.
#[derive(Debug, Clone, Copy, Default)]
pub struct SwapAnchor;

impl Command for SwapAnchor {
    fn id(&self) -> CommandId {
        ids::VISUAL_SWAP_ANCHOR
    }

    fn description(&self) -> &'static str {
        "Swap cursor and anchor in visual mode"
    }
}

impl CommandHandler for SwapAnchor {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Only operate if selection is active
        let Some(selection) = runtime.windows().active().and_then(|w| w.selection.clone()) else {
            return CommandResult::Success; // No selection - no-op
        };

        // Swap anchor and cursor - swap start and end
        let swapped = reovim_driver_session::api::Selection {
            start: selection.end,
            end: selection.start,
            mode: selection.mode,
        };

        if let Some(window) = runtime.windows_mut().active_mut() {
            window.selection = Some(swapped);
        }

        CommandResult::Success
    }
}

/// Toggle to character-wise visual mode (v in visual mode).
///
/// If already in character-wise mode, exit to normal mode.
/// Otherwise, switch to character-wise selection.
#[derive(Debug, Clone, Copy, Default)]
pub struct ToggleVisualChar;

impl Command for ToggleVisualChar {
    fn id(&self) -> CommandId {
        ids::TOGGLE_VISUAL_CHAR
    }

    fn description(&self) -> &'static str {
        "Toggle to character-wise visual mode"
    }
}

impl CommandHandler for ToggleVisualChar {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let selection = runtime.windows().active().and_then(|w| w.selection.clone());

        let target_mode = match selection {
            Some(sel) if sel.mode == SelectionMode::Character => {
                // Already in character mode - exit to normal
                if let Some(window) = runtime.windows_mut().active_mut() {
                    window.selection = None;
                }
                VimMode::NORMAL_ID
            }
            Some(mut sel) => {
                // Switch to character mode
                sel.mode = SelectionMode::Character;
                if let Some(window) = runtime.windows_mut().active_mut() {
                    window.selection = Some(sel);
                }
                VimMode::VISUAL_ID
            }
            None => {
                // No selection - nothing to do
                return CommandResult::Success;
            }
        };

        runtime.set_mode(target_mode, TransitionContext::new());

        CommandResult::Success
    }
}

/// Toggle to line-wise visual mode (V in visual mode).
///
/// If already in line-wise mode, exit to normal mode.
/// Otherwise, switch to line-wise selection.
#[derive(Debug, Clone, Copy, Default)]
pub struct ToggleVisualLine;

impl Command for ToggleVisualLine {
    fn id(&self) -> CommandId {
        ids::TOGGLE_VISUAL_LINE
    }

    fn description(&self) -> &'static str {
        "Toggle to line-wise visual mode"
    }
}

impl CommandHandler for ToggleVisualLine {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let selection = runtime.windows().active().and_then(|w| w.selection.clone());

        let target_mode = match selection {
            Some(sel) if sel.mode == SelectionMode::Line => {
                // Already in line mode - exit to normal
                if let Some(window) = runtime.windows_mut().active_mut() {
                    window.selection = None;
                }
                VimMode::NORMAL_ID
            }
            Some(mut sel) => {
                // Switch to line mode
                sel.mode = SelectionMode::Line;
                if let Some(window) = runtime.windows_mut().active_mut() {
                    window.selection = Some(sel);
                }
                VimMode::VISUAL_LINE_ID
            }
            None => {
                // No selection - nothing to do
                return CommandResult::Success;
            }
        };

        runtime.set_mode(target_mode, TransitionContext::new());

        CommandResult::Success
    }
}

/// Toggle to block-wise visual mode (Ctrl-V in visual mode).
///
/// If already in block mode, exit to normal mode.
/// Otherwise, switch to block selection.
#[derive(Debug, Clone, Copy, Default)]
pub struct ToggleVisualBlock;

impl Command for ToggleVisualBlock {
    fn id(&self) -> CommandId {
        ids::TOGGLE_VISUAL_BLOCK
    }

    fn description(&self) -> &'static str {
        "Toggle to block-wise visual mode"
    }
}

impl CommandHandler for ToggleVisualBlock {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let selection = runtime.windows().active().and_then(|w| w.selection.clone());

        let target_mode = match selection {
            Some(sel) if sel.mode == SelectionMode::Block => {
                // Already in block mode - exit to normal
                if let Some(window) = runtime.windows_mut().active_mut() {
                    window.selection = None;
                }
                VimMode::NORMAL_ID
            }
            Some(mut sel) => {
                // Switch to block mode
                sel.mode = SelectionMode::Block;
                if let Some(window) = runtime.windows_mut().active_mut() {
                    window.selection = Some(sel);
                }
                VimMode::VISUAL_BLOCK_ID
            }
            None => {
                // No selection - nothing to do
                return CommandResult::Success;
            }
        };

        runtime.set_mode(target_mode, TransitionContext::new());

        CommandResult::Success
    }
}

/// Reselect the last visual selection (gv in normal mode).
///
/// Restores the previous visual selection bounds and enters the appropriate
/// visual mode. If no previous selection exists, this is a no-op.
///
/// Note: The actual restoration logic is handled by the event loop since it
/// requires access to `AppState.last_visual_selection`. This command just signals
/// the intent to reselect.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReselectLast;

impl Command for ReselectLast {
    fn id(&self) -> CommandId {
        ids::RESELECT_LAST
    }

    fn description(&self) -> &'static str {
        "Reselect the last visual selection (gv)"
    }
}

impl CommandHandler for ReselectLast {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        // Will signal intent to restore the last visual selection
        CommandResult::Success
    }
}

#[cfg(test)]
#[allow(clippy::significant_drop_tightening, clippy::uninlined_format_args)]
mod tests {
    use reovim_kernel::testing::create_test_context;
    use {super::*, reovim_driver_command::Command};

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
        reovim_driver_command::CommandHandler,
        reovim_driver_session::{
            testing::StubExecutor,
            ClientId, ExtensionMap, Session, SessionRuntime, WindowLayout,
            api::{Selection, SelectionMode},
        },
        reovim_kernel::api::{
            ModeStack,
            v1::{
                Buffer, BufferId, HistoryRing, KernelContext,
                MarkBank, ModeId, ModuleId, Position, RegisterBank,
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
}
