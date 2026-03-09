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
mod tests {
    use super::*;

    // ========================================================================
    // DetachCommand tests
    // ========================================================================

    #[test]
    fn test_detach_command_id() {
        let cmd = DetachCommand;
        assert_eq!(cmd.id().name(), "detach");
        assert_eq!(cmd.id().module().as_str(), "commands");
    }

    #[test]
    fn test_detach_command_names() {
        let cmd = DetachCommand;
        assert_eq!(cmd.names(), &["detach"]);
        assert_eq!(cmd.names().len(), 1);
    }

    #[test]
    fn test_detach_command_description() {
        let cmd = DetachCommand;
        let desc = cmd.description();
        assert!(!desc.is_empty());
        assert!(desc.contains("Detach"));
    }

    #[test]
    fn test_detach_command_complete_returns_empty() {
        let cmd = DetachCommand;
        let completions = cmd.complete("anything");
        assert!(completions.is_empty());
    }

    // ========================================================================
    // ServersCommand tests
    // ========================================================================

    #[test]
    fn test_servers_command_id() {
        let cmd = ServersCommand;
        assert_eq!(cmd.id().name(), "servers");
        assert_eq!(cmd.id().module().as_str(), "commands");
    }

    #[test]
    fn test_servers_command_names() {
        let cmd = ServersCommand;
        assert_eq!(cmd.names(), &["servers"]);
        assert_eq!(cmd.names().len(), 1);
    }

    #[test]
    fn test_servers_command_description() {
        let cmd = ServersCommand;
        let desc = cmd.description();
        assert!(!desc.is_empty());
        assert!(desc.contains("server"));
    }

    #[test]
    fn test_servers_command_complete_returns_empty() {
        let cmd = ServersCommand;
        let completions = cmd.complete("");
        assert!(completions.is_empty());
    }

    // ========================================================================
    // KillServerCommand tests
    // ========================================================================

    #[test]
    fn test_kill_server_command_id() {
        let cmd = KillServerCommand;
        assert_eq!(cmd.id().name(), "kill-server");
        assert_eq!(cmd.id().module().as_str(), "commands");
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
    fn test_kill_server_command_description() {
        let cmd = KillServerCommand;
        let desc = cmd.description();
        assert!(!desc.is_empty());
        assert!(desc.contains("Kill"));
    }

    #[test]
    fn test_kill_server_command_complete_returns_empty() {
        let cmd = KillServerCommand;
        let completions = cmd.complete("anything");
        assert!(completions.is_empty());
    }

    // ========================================================================
    // command_handlers() collection tests
    // ========================================================================

    #[test]
    fn test_command_handlers_collection_count() {
        let cmds = command_handlers();
        assert_eq!(cmds.len(), 3);
    }

    #[test]
    fn test_command_handlers_collection_ids() {
        let cmds = command_handlers();
        let ids: Vec<_> = cmds.iter().map(|c| c.id().name()).collect();
        assert!(ids.contains(&"detach"));
        assert!(ids.contains(&"servers"));
        assert!(ids.contains(&"kill-server"));
    }

    #[test]
    fn test_command_handlers_all_have_names() {
        let cmds = command_handlers();
        for cmd in &cmds {
            assert!(!cmd.names().is_empty(), "command '{}' has no names", cmd.id().name());
        }
    }

    #[test]
    fn test_command_handlers_all_have_descriptions() {
        let cmds = command_handlers();
        for cmd in &cmds {
            assert!(
                !cmd.description().is_empty(),
                "command '{}' has no description",
                cmd.id().name()
            );
        }
    }

    // ========================================================================
    // Trait object safety tests
    // ========================================================================

    #[test]
    fn test_session_commands_as_trait_objects() {
        let cmds: Vec<Box<dyn CommandHandler>> = vec![
            Box::new(DetachCommand),
            Box::new(ServersCommand),
            Box::new(KillServerCommand),
        ];
        assert_eq!(cmds.len(), 3);
        for cmd in &cmds {
            assert!(!cmd.id().name().is_empty());
        }
    }
}
