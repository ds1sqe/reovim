//! Session management commands.
//!
//! Commands for managing client sessions and server connections:
//! - `:detach` - Detach from server (server continues)
//! - `:servers` - List running server instances
//! - `:kill-server` - Kill the current server
//!
//! These commands are part of issue #350 (Unified Port System).

use crate::types::{CommandError, ExCommandContext, ExCommandHandler};

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

impl ExCommandHandler for DetachCommand {
    fn id(&self) -> &'static str {
        "detach"
    }

    fn names(&self) -> &[&'static str] {
        &["detach"]
    }

    fn execute(&self, _ctx: &mut ExCommandContext<'_>, _args: &[&str]) -> Result<(), CommandError> {
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

impl ExCommandHandler for ServersCommand {
    fn id(&self) -> &'static str {
        "servers"
    }

    fn names(&self) -> &[&'static str] {
        &["servers"]
    }

    fn execute(&self, _ctx: &mut ExCommandContext<'_>, _args: &[&str]) -> Result<(), CommandError> {
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

impl ExCommandHandler for KillServerCommand {
    fn id(&self) -> &'static str {
        "kill-server"
    }

    fn names(&self) -> &[&'static str] {
        &["kill-server", "killserver"]
    }

    fn execute(&self, _ctx: &mut ExCommandContext<'_>, _args: &[&str]) -> Result<(), CommandError> {
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
pub fn commands() -> Vec<Box<dyn ExCommandHandler>> {
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
    use {super::*, reovim_kernel::api::v1::KernelContext};

    // ========================================================================
    // DetachCommand tests
    // ========================================================================

    #[test]
    fn test_detach_command_id() {
        let cmd = DetachCommand;
        assert_eq!(cmd.id(), "detach");
    }

    #[test]
    fn test_detach_command_names() {
        let cmd = DetachCommand;
        assert_eq!(cmd.names(), &["detach"]);
        assert_eq!(cmd.names().len(), 1);
    }

    #[test]
    fn test_detach_command_help() {
        let cmd = DetachCommand;
        let help = cmd.help();
        assert!(!help.is_empty());
        assert!(help.contains("Detach"));
    }

    #[test]
    fn test_detach_command_complete_returns_empty() {
        let cmd = DetachCommand;
        let completions = cmd.complete("anything");
        assert!(completions.is_empty());
    }

    #[test]
    fn test_detach_command_execute() {
        let kernel = KernelContext::default();
        let mut ctx = ExCommandContext::new(&kernel);

        let cmd = DetachCommand;
        let result = cmd.execute(&mut ctx, &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_detach_command_execute_ignores_args() {
        let kernel = KernelContext::default();
        let mut ctx = ExCommandContext::new(&kernel);

        let cmd = DetachCommand;
        let result = cmd.execute(&mut ctx, &["extra", "args"]);
        assert!(result.is_ok());
    }

    // ========================================================================
    // ServersCommand tests
    // ========================================================================

    #[test]
    fn test_servers_command_id() {
        let cmd = ServersCommand;
        assert_eq!(cmd.id(), "servers");
    }

    #[test]
    fn test_servers_command_names() {
        let cmd = ServersCommand;
        assert_eq!(cmd.names(), &["servers"]);
        assert_eq!(cmd.names().len(), 1);
    }

    #[test]
    fn test_servers_command_help() {
        let cmd = ServersCommand;
        let help = cmd.help();
        assert!(!help.is_empty());
        assert!(help.contains("server"));
    }

    #[test]
    fn test_servers_command_complete_returns_empty() {
        let cmd = ServersCommand;
        let completions = cmd.complete("");
        assert!(completions.is_empty());
    }

    #[test]
    fn test_servers_command_execute() {
        let kernel = KernelContext::default();
        let mut ctx = ExCommandContext::new(&kernel);

        let cmd = ServersCommand;
        let result = cmd.execute(&mut ctx, &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_servers_command_execute_ignores_args() {
        let kernel = KernelContext::default();
        let mut ctx = ExCommandContext::new(&kernel);

        let cmd = ServersCommand;
        let result = cmd.execute(&mut ctx, &["arg1"]);
        assert!(result.is_ok());
    }

    // ========================================================================
    // KillServerCommand tests
    // ========================================================================

    #[test]
    fn test_kill_server_command_id() {
        let cmd = KillServerCommand;
        assert_eq!(cmd.id(), "kill-server");
    }

    #[test]
    fn test_kill_server_command_names() {
        let cmd = KillServerCommand;
        let names = cmd.names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"kill-server"));
        assert!(names.contains(&"killserver"));
    }

    #[test]
    fn test_kill_server_command_help() {
        let cmd = KillServerCommand;
        let help = cmd.help();
        assert!(!help.is_empty());
        assert!(help.contains("Kill"));
    }

    #[test]
    fn test_kill_server_command_complete_returns_empty() {
        let cmd = KillServerCommand;
        let completions = cmd.complete("anything");
        assert!(completions.is_empty());
    }

    #[test]
    fn test_kill_server_command_execute() {
        let kernel = KernelContext::default();
        let mut ctx = ExCommandContext::new(&kernel);

        let cmd = KillServerCommand;
        let result = cmd.execute(&mut ctx, &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_kill_server_command_execute_ignores_args() {
        let kernel = KernelContext::default();
        let mut ctx = ExCommandContext::new(&kernel);

        let cmd = KillServerCommand;
        let result = cmd.execute(&mut ctx, &["extra"]);
        assert!(result.is_ok());
    }

    // ========================================================================
    // commands() collection tests
    // ========================================================================

    #[test]
    fn test_commands_collection_count() {
        let cmds = commands();
        assert_eq!(cmds.len(), 3);
    }

    #[test]
    fn test_commands_collection_ids() {
        let cmds = commands();
        let ids: Vec<_> = cmds.iter().map(|c| c.id()).collect();
        assert!(ids.contains(&"detach"));
        assert!(ids.contains(&"servers"));
        assert!(ids.contains(&"kill-server"));
    }

    #[test]
    fn test_commands_collection_all_have_names() {
        let cmds = commands();
        for cmd in &cmds {
            assert!(!cmd.names().is_empty(), "command '{}' has no names", cmd.id());
        }
    }

    #[test]
    fn test_commands_collection_all_have_help() {
        let cmds = commands();
        for cmd in &cmds {
            assert!(!cmd.help().is_empty(), "command '{}' has no help text", cmd.id());
        }
    }

    // ========================================================================
    // Trait object safety tests
    // ========================================================================

    #[test]
    fn test_session_commands_as_trait_objects() {
        let cmds: Vec<Box<dyn ExCommandHandler>> = vec![
            Box::new(DetachCommand),
            Box::new(ServersCommand),
            Box::new(KillServerCommand),
        ];
        assert_eq!(cmds.len(), 3);
        for cmd in &cmds {
            assert!(!cmd.id().is_empty());
        }
    }
}
