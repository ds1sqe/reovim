//! Ex-commands module - POLICY.
//!
//! Reference: `lib/core/src/command_line/ex_command.rs` (concept-extraction, not migration)
//!
//! This module implements the standard ex-commands (colon commands):
//! - `:q` / `:quit` - Quit the editor
//! - `:w` / `:write` - Write the buffer to disk
//! - `:wq` - Write and quit
//!
//! # Mechanism vs Policy
//!
//! - **Mechanism (Kernel)**: Buffer management, position types
//! - **Policy (This Module)**: `CommandHandler` trait, what commands do
//!
//! # Example
//!
//! ```ignore
//! use reovim_module_commands::commands;
//!
//! // Get all commands registered by this module
//! let cmds = commands();
//! for cmd in &cmds {
//!     println!("{}: {:?}", cmd.id(), cmd.names());
//! }
//! ```

mod quit;
mod session;
mod types;
mod write;

use reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version};

// Re-export command types
pub use types::{CommandContext, CommandError, CommandHandler, Range};

pub use {
    quit::QuitCommand,
    session::{DetachCommand, KillServerCommand, ServersCommand},
    write::{WriteCommand, WriteQuitCommand},
};

/// Returns all commands provided by this module.
#[must_use]
pub fn commands() -> Vec<Box<dyn CommandHandler>> {
    let mut cmds: Vec<Box<dyn CommandHandler>> = vec![
        Box::new(QuitCommand),
        Box::new(WriteCommand),
        Box::new(WriteQuitCommand),
    ];
    // Add session management commands from #350
    cmds.extend(session::commands());
    cmds
}

// ============================================================================
// Module trait implementation
// ============================================================================

/// Commands module instance.
pub struct CommandsModule;

impl CommandsModule {
    /// Create a new commands module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for CommandsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for CommandsModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("commands")
    }

    fn name(&self) -> &'static str {
        "Ex-Commands"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(CommandsModule);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commands_list() {
        let cmds = commands();
        assert_eq!(cmds.len(), 6); // 3 base + 3 session commands

        // Check commands are present
        let ids: Vec<_> = cmds.iter().map(|c| c.id()).collect();
        assert!(ids.contains(&"quit"));
        assert!(ids.contains(&"write"));
        assert!(ids.contains(&"write-quit"));
        // Session commands from #350
        assert!(ids.contains(&"detach"));
        assert!(ids.contains(&"servers"));
        assert!(ids.contains(&"kill-server"));
    }

    #[test]
    fn test_quit_command_names() {
        let cmd = QuitCommand;
        let names = cmd.names();
        assert!(names.contains(&"q"));
        assert!(names.contains(&"quit"));
    }

    #[test]
    fn test_write_command_names() {
        let cmd = WriteCommand;
        let names = cmd.names();
        assert!(names.contains(&"w"));
        assert!(names.contains(&"write"));
    }

    #[test]
    fn test_module_trait() {
        let module = CommandsModule;
        assert_eq!(module.id().as_str(), "commands");
        assert_eq!(module.name(), "Ex-Commands");
    }
}
