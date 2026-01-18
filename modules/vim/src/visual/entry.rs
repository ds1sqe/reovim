//! Visual mode entry commands.
//!
//! Provides commands to enter the various visual selection modes:
//! - `v` - Character-wise selection
//! - `V` - Line-wise selection
//! - `Ctrl-V` - Block (rectangular) selection

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_kernel::api::v1::{CommandId, KernelContext, SelectionMode, events::ModeChanged},
};

use crate::modes::{VIM_MODULE, VimMode};

/// Enter visual mode (character-wise selection).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterVisualMode;

impl Command for EnterVisualMode {
    fn id(&self) -> CommandId {
        CommandId::new(VIM_MODULE, "enter-visual")
    }

    fn description(&self) -> &'static str {
        "Enter visual mode (character-wise)"
    }
}

impl CommandHandler for EnterVisualMode {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        // Start character-wise selection at current cursor position
        {
            let mut buffer = buffer_arc.write();
            let pos = buffer.position();
            buffer.selection_mut().start(pos, SelectionMode::Character);
        }

        // Emit mode change event with target ModeId
        ctx.event_bus
            .emit(ModeChanged::with_mode_id("normal", VimMode::VISUAL_ID));

        CommandResult::Success
    }
}

/// Enter visual line mode (line-wise selection).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterVisualLineMode;

impl Command for EnterVisualLineMode {
    fn id(&self) -> CommandId {
        CommandId::new(VIM_MODULE, "enter-visual-line")
    }

    fn description(&self) -> &'static str {
        "Enter visual line mode"
    }
}

impl CommandHandler for EnterVisualLineMode {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        // Start line-wise selection at current cursor position
        {
            let mut buffer = buffer_arc.write();
            let pos = buffer.position();
            buffer.selection_mut().start(pos, SelectionMode::Line);
        }

        // Emit mode change event with target ModeId
        ctx.event_bus
            .emit(ModeChanged::with_mode_id("normal", VimMode::VISUAL_LINE_ID));

        CommandResult::Success
    }
}

/// Enter visual block mode (rectangular selection).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterVisualBlockMode;

impl Command for EnterVisualBlockMode {
    fn id(&self) -> CommandId {
        CommandId::new(VIM_MODULE, "enter-visual-block")
    }

    fn description(&self) -> &'static str {
        "Enter visual block mode"
    }
}

impl CommandHandler for EnterVisualBlockMode {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        // Start block selection at current cursor position
        {
            let mut buffer = buffer_arc.write();
            let pos = buffer.position();
            buffer.selection_mut().start(pos, SelectionMode::Block);
        }

        // Emit mode change event with target ModeId
        ctx.event_bus
            .emit(ModeChanged::with_mode_id("normal", VimMode::VISUAL_BLOCK_ID));

        CommandResult::Success
    }
}
