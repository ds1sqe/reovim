//! Session management commands.
//!
//! Commands for managing client sessions and server connections:
//! - `:detach` - Detach from server (server continues)
//! - `:servers` - List running server instances
//! - `:kill-server` - Kill the current server
//!
//! These commands are part of issue #350 (Unified Port System).

use crate::types::{CommandContext, CommandError, CommandHandler};

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

impl CommandHandler for DetachCommand {
    fn id(&self) -> &'static str {
        "detach"
    }

    fn names(&self) -> &[&'static str] {
        &["detach"]
    }

    fn execute(&self, _ctx: &mut CommandContext<'_>, _args: &[&str]) -> Result<(), CommandError> {
        // Note: The actual detach is handled by the runner.
        // This command signals the intent to detach.
        // The runner checks for this command and sends DETACH notification.
        Ok(())
    }

    fn help(&self) -> &'static str {
        "Detach from the server (server continues running)"
    }
}

// ============================================================================
// Servers Command
// ============================================================================

/// List running server instances.
///
/// Queries the instance registry and displays all registered servers
/// with their names, transport addresses, and PIDs.
///
/// Usage: `:servers`
pub struct ServersCommand;

impl CommandHandler for ServersCommand {
    fn id(&self) -> &'static str {
        "servers"
    }

    fn names(&self) -> &[&'static str] {
        &["servers"]
    }

    fn execute(&self, _ctx: &mut CommandContext<'_>, _args: &[&str]) -> Result<(), CommandError> {
        // Note: The actual listing is handled by the runner.
        // This command signals the intent to list servers.
        // The runner queries the instance registry and displays results.
        Ok(())
    }

    fn help(&self) -> &'static str {
        "List running server instances"
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

impl CommandHandler for KillServerCommand {
    fn id(&self) -> &'static str {
        "kill-server"
    }

    fn names(&self) -> &[&'static str] {
        &["kill-server", "killserver"]
    }

    fn execute(&self, _ctx: &mut CommandContext<'_>, _args: &[&str]) -> Result<(), CommandError> {
        // Note: The actual termination is handled by the runner.
        // This command signals the intent to kill the server.
        // The runner initiates a forced quit.
        Ok(())
    }

    fn help(&self) -> &'static str {
        "Kill the current server (terminates all sessions)"
    }
}

// ============================================================================
// Command Collection
// ============================================================================

/// Get all session management commands.
#[must_use]
pub fn commands() -> Vec<Box<dyn CommandHandler>> {
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
mod tests {
    use super::*;

    #[test]
    fn test_detach_command_metadata() {
        let cmd = DetachCommand;
        assert_eq!(cmd.id(), "detach");
        assert_eq!(cmd.names(), &["detach"]);
        assert!(!cmd.help().is_empty());
    }

    #[test]
    fn test_servers_command_metadata() {
        let cmd = ServersCommand;
        assert_eq!(cmd.id(), "servers");
        assert_eq!(cmd.names(), &["servers"]);
        assert!(!cmd.help().is_empty());
    }

    #[test]
    fn test_kill_server_command_metadata() {
        let cmd = KillServerCommand;
        assert_eq!(cmd.id(), "kill-server");
        assert!(cmd.names().contains(&"kill-server"));
        assert!(cmd.names().contains(&"killserver"));
        assert!(!cmd.help().is_empty());
    }

    #[test]
    fn test_commands_collection() {
        let cmds = commands();
        assert_eq!(cmds.len(), 3);
    }
}
