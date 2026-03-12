//! `CheckHealth` command handler.

use {
    crate::diagnostics,
    reovim_driver_command::{Command, CommandHandler},
    reovim_driver_command_types::{CommandContext, CommandResult},
    reovim_driver_session::{BufferApi, SessionRuntime},
    reovim_kernel::api::v1::{CommandId, ModuleId},
};

const HEALTH_CHECK_MODULE: ModuleId = ModuleId::new("health-check");

/// Run health diagnostics and display results in a scratch buffer.
pub struct CheckHealthCommand;

impl CheckHealthCommand {
    /// Create a new checkhealth command.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for CheckHealthCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for CheckHealthCommand {
    fn id(&self) -> CommandId {
        CommandId::new(HEALTH_CHECK_MODULE, "checkhealth")
    }

    fn description(&self) -> &'static str {
        "Run health diagnostics and display results"
    }

    fn names(&self) -> &[&'static str] {
        &["checkhealth"]
    }
}

impl CommandHandler for CheckHealthCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let sections = diagnostics::collect_all(runtime.kernel());
        let report = diagnostics::format_report(&sections);
        let buf_id = runtime.create_buffer(Some("[checkhealth]"), &report);
        runtime.set_buffer_modified(buf_id, false);
        runtime.set_active_buffer(Some(buf_id));
        CommandResult::Success
    }
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
