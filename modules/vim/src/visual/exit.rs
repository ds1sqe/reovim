//! Visual mode exit commands.
//!
//! Provides commands to exit visual mode and return to normal mode.

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_kernel::api::v1::{CommandId, KernelContext, events::ModeChanged},
};

use reovim_module_editor::{EDITOR_MODULE, EditorMode};

/// Exit visual mode and return to normal mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExitVisualMode;

impl Command for ExitVisualMode {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "exit-visual")
    }

    fn description(&self) -> &'static str {
        "Exit visual mode and return to normal mode"
    }
}

impl CommandHandler for ExitVisualMode {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        // Clear selection if we have a buffer
        if let Some(buffer_id) = args.buffer_id()
            && let Some(buffer_arc) = ctx.buffers.get(buffer_id)
        {
            let mut buffer = buffer_arc.write();
            buffer.selection_mut().clear();
        }

        // Emit mode change event with target ModeId
        ctx.event_bus
            .emit(ModeChanged::with_mode_id("visual", EditorMode::NORMAL_ID));

        CommandResult::Success
    }
}
