//! Quit command.

use {
    reovim_driver_command::{
        Command, CommandContext, CommandHandler, CommandResult, RuntimeSignal,
    },
    reovim_driver_text_session::{BufferApi, SessionRuntime},
    reovim_kernel::api::v1::{CommandId, ModuleId},
};

const COMMANDS_MODULE: ModuleId = ModuleId::new("commands");

/// Quit command - exit the editor.
///
/// Behavior:
/// - `:q` - Quit if no unsaved changes
/// - `:q!` - Force quit, discard unsaved changes
#[derive(Debug, Clone, Copy)]
pub struct QuitCommand;

impl Command for QuitCommand {
    fn id(&self) -> CommandId {
        CommandId::new(COMMANDS_MODULE, "quit")
    }

    fn description(&self) -> &'static str {
        "Quit the editor. Use :q! to force quit with unsaved changes."
    }

    fn names(&self) -> &[&'static str] {
        &["q", "quit"]
    }
}

impl CommandHandler for QuitCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, ctx: &CommandContext) -> CommandResult {
        // Check for unsaved changes (unless bang is set)
        if !ctx.has_bang()
            && let Some(buffer_id) = runtime.active_buffer()
            && runtime
                .with_buffer_read(buffer_id, reovim_kernel::api::BufferMeta::is_modified)
                .unwrap_or(false)
        {
            return CommandResult::Error(
                "No write since last change (add ! to override)".to_string(),
            );
        }

        runtime.signal(RuntimeSignal::Quit);
        CommandResult::Success
    }
}

#[cfg(test)]
#[path = "quit_tests.rs"]
mod tests;
