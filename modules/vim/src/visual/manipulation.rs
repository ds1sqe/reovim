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
    reovim_kernel::api::v1::{CommandId, KernelContext, SelectionMode, events::ModeChanged},
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
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let mut buffer = buffer_arc.write();

        // Only operate if selection is active
        if !buffer.selection().is_active() {
            return CommandResult::Success;
        }

        // Swap anchor and cursor
        let old_anchor = buffer.selection().anchor;
        let cursor = buffer.position();

        buffer.selection_mut().anchor = cursor;
        buffer.set_position(old_anchor);

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
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let mut buffer = buffer_arc.write();
        let (from_mode, target_mode) =
            if buffer.selection().is_active() && buffer.selection().mode().is_character() {
                // Already in character mode - exit to normal
                buffer.selection_mut().clear();
                ("visual", VimMode::NORMAL_ID)
            } else {
                // Switch to character mode
                buffer.selection_mut().set_mode(SelectionMode::Character);
                ("visual-line", VimMode::VISUAL_ID)
            };
        drop(buffer);

        ctx.event_bus
            .emit(ModeChanged::with_mode_id(from_mode, target_mode));

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
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let mut buffer = buffer_arc.write();
        let (from_mode, target_mode) =
            if buffer.selection().is_active() && buffer.selection().mode().is_line() {
                // Already in line mode - exit to normal
                buffer.selection_mut().clear();
                ("visual-line", VimMode::NORMAL_ID)
            } else {
                // Switch to line mode
                buffer.selection_mut().set_mode(SelectionMode::Line);
                ("visual", VimMode::VISUAL_LINE_ID)
            };
        drop(buffer);

        ctx.event_bus
            .emit(ModeChanged::with_mode_id(from_mode, target_mode));

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
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let mut buffer = buffer_arc.write();
        let (from_mode, target_mode) =
            if buffer.selection().is_active() && buffer.selection().mode().is_block() {
                // Already in block mode - exit to normal
                buffer.selection_mut().clear();
                ("visual-block", VimMode::NORMAL_ID)
            } else {
                // Switch to block mode
                buffer.selection_mut().set_mode(SelectionMode::Block);
                ("visual", VimMode::VISUAL_BLOCK_ID)
            };
        drop(buffer);

        ctx.event_bus
            .emit(ModeChanged::with_mode_id(from_mode, target_mode));

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
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        // Signal intent to restore the last visual selection.
        // The runner handles the actual restoration since it requires AppState.
        CommandResult::ReselectVisual
    }
}
