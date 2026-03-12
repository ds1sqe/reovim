//! Session management commands.
//!
//! Commands for managing client sessions and server connections:
//! - `:detach` - Detach from server (server continues)
//! - `:servers` - List running server instances
//! - `:kill-server` - Kill the current server

use {
    reovim_driver_command::{
        Command, CommandContext, CommandHandler, CommandResult, RuntimeSignal,
    },
    reovim_driver_session::SessionRuntime,
    reovim_kernel::api::v1::{CommandId, ModuleId},
};

const COMMANDS_MODULE: ModuleId = ModuleId::new("commands");

// ============================================================================
// Detach Command
// ============================================================================

/// Detach from the current server session.
///
/// The server continues running after detach, allowing other clients
/// to connect. The TUI client receives a DETACH notification and
/// disconnects gracefully.
///
/// Usage: `:detach`
pub struct DetachCommand;

impl Command for DetachCommand {
    fn id(&self) -> CommandId {
        CommandId::new(COMMANDS_MODULE, "detach")
    }

    fn description(&self) -> &'static str {
        "Detach from the server (server continues running)"
    }

    fn names(&self) -> &[&'static str] {
        &["detach"]
    }
}

impl CommandHandler for DetachCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        runtime.signal(RuntimeSignal::Quit);
        CommandResult::Success
    }
}

// ============================================================================
// Servers Command
// ============================================================================

/// List running server instances.
///
/// Usage: `:servers`
pub struct ServersCommand;

impl Command for ServersCommand {
    fn id(&self) -> CommandId {
        CommandId::new(COMMANDS_MODULE, "servers")
    }

    fn description(&self) -> &'static str {
        "List running server instances"
    }

    fn names(&self) -> &[&'static str] {
        &["servers"]
    }
}

impl CommandHandler for ServersCommand {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        // Note: The actual listing is handled by the runner.
        CommandResult::Success
    }
}

// ============================================================================
// KillServer Command
// ============================================================================

/// Kill the current server.
///
/// Terminates the current server process. All connected clients are
/// disconnected and unsaved changes may be lost.
///
/// Usage: `:kill-server`
pub struct KillServerCommand;

impl Command for KillServerCommand {
    fn id(&self) -> CommandId {
        CommandId::new(COMMANDS_MODULE, "kill-server")
    }

    fn description(&self) -> &'static str {
        "Kill the current server (terminates all sessions)"
    }

    fn names(&self) -> &[&'static str] {
        &["kill-server", "killserver"]
    }
}

impl CommandHandler for KillServerCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        runtime.signal(RuntimeSignal::Quit);
        CommandResult::Success
    }
}

// ============================================================================
// Command Collection
// ============================================================================

/// Get all session management command handlers.
#[must_use]
pub fn command_handlers() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(DetachCommand),
        Box::new(ServersCommand),
        Box::new(KillServerCommand),
    ]
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
