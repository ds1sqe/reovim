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
        BufferApi, SessionRuntime, TransitionContext,
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Only operate if selection is active
        if runtime.selection(buffer_id).is_none() {
            return CommandResult::Success;
        }

        // Swap anchor and cursor
        runtime.swap_selection_ends(buffer_id);

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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let target_mode = match runtime.selection(buffer_id) {
            Some(sel) if sel.mode == SelectionMode::Character => {
                // Already in character mode - exit to normal
                runtime.set_selection(buffer_id, None);
                VimMode::NORMAL_ID
            }
            Some(_) => {
                // Switch to character mode
                runtime.set_selection_mode(buffer_id, SelectionMode::Character);
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let target_mode = match runtime.selection(buffer_id) {
            Some(sel) if sel.mode == SelectionMode::Line => {
                // Already in line mode - exit to normal
                runtime.set_selection(buffer_id, None);
                VimMode::NORMAL_ID
            }
            Some(_) => {
                // Switch to line mode
                runtime.set_selection_mode(buffer_id, SelectionMode::Line);
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let target_mode = match runtime.selection(buffer_id) {
            Some(sel) if sel.mode == SelectionMode::Block => {
                // Already in block mode - exit to normal
                runtime.set_selection(buffer_id, None);
                VimMode::NORMAL_ID
            }
            Some(_) => {
                // Switch to block mode
                runtime.set_selection_mode(buffer_id, SelectionMode::Block);
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
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        // Will signal intent to restore the last visual selection
        CommandResult::Success
    }
}
