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
//! - **Mechanism (Kernel)**: `CommandHandler` trait, `CommandContext`
//! - **Policy (This Module)**: Which commands exist, what they do
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
mod write;

use reovim_kernel::api::v1::{
    CommandHandler, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
};

pub use {
    quit::QuitCommand,
    write::{WriteCommand, WriteQuitCommand},
};

/// Returns all commands provided by this module.
#[must_use]
pub fn commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(QuitCommand),
        Box::new(WriteCommand),
        Box::new(WriteQuitCommand),
    ]
}

// ============================================================================
// Module trait implementation
// ============================================================================

/// Commands module instance.
pub struct CommandsModule;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commands_list() {
        let cmds = commands();
        assert_eq!(cmds.len(), 3);

        // Check commands are present
        let ids: Vec<_> = cmds.iter().map(|c| c.id()).collect();
        assert!(ids.contains(&"quit"));
        assert!(ids.contains(&"write"));
        assert!(ids.contains(&"write-quit"));
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
