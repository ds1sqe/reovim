//! Visual mode entry commands.
//!
//! Provides commands to enter the various visual selection modes:
//! - `v` - Character-wise selection
//! - `V` - Line-wise selection
//! - `Ctrl-V` - Block (rectangular) selection

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{
        BufferApi, SessionRuntime, TransitionContext,
        api::{ModeApi, Selection},
    },
    reovim_kernel::api::v1::CommandId,
};

use crate::{ids, modes::VimMode};

/// Enter visual mode (character-wise selection).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterVisualMode;

impl Command for EnterVisualMode {
    fn id(&self) -> CommandId {
        ids::ENTER_VISUAL
    }

    fn description(&self) -> &'static str {
        "Enter visual mode (character-wise)"
    }
}

impl CommandHandler for EnterVisualMode {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get current cursor position
        let Some(pos) = runtime.buffer_position(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        // Start character-wise selection at current cursor position
        let selection = Selection::character(pos, pos);
        runtime.set_selection(buffer_id, Some(selection));

        // Change to visual mode
        runtime.set_mode(VimMode::VISUAL_ID, TransitionContext::new());

        CommandResult::Success
    }
}

/// Enter visual line mode (line-wise selection).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterVisualLineMode;

impl Command for EnterVisualLineMode {
    fn id(&self) -> CommandId {
        ids::ENTER_VISUAL_LINE
    }

    fn description(&self) -> &'static str {
        "Enter visual line mode"
    }
}

impl CommandHandler for EnterVisualLineMode {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get current cursor position
        let Some(pos) = runtime.buffer_position(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        // Start line-wise selection at current cursor position
        let selection = Selection::line(pos, pos);
        runtime.set_selection(buffer_id, Some(selection));

        // Change to visual line mode
        runtime.set_mode(VimMode::VISUAL_LINE_ID, TransitionContext::new());

        CommandResult::Success
    }
}

/// Enter visual block mode (rectangular selection).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterVisualBlockMode;

impl Command for EnterVisualBlockMode {
    fn id(&self) -> CommandId {
        ids::ENTER_VISUAL_BLOCK
    }

    fn description(&self) -> &'static str {
        "Enter visual block mode"
    }
}

impl CommandHandler for EnterVisualBlockMode {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get current cursor position
        let Some(pos) = runtime.buffer_position(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        // Start block selection at current cursor position
        let selection = Selection::block(pos, pos);
        runtime.set_selection(buffer_id, Some(selection));

        // Change to visual block mode
        runtime.set_mode(VimMode::VISUAL_BLOCK_ID, TransitionContext::new());

        CommandResult::Success
    }
}
