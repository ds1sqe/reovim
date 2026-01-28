//! Example command implementation.
//!
//! Demonstrates implementing Command and `CommandHandler` traits
//! from the command driver.

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::SessionRuntime,
    reovim_kernel::api::v1::CommandId,
};

use crate::EXAMPLE_MODULE;

// ============================================================================
// HelloCommand
// ============================================================================

/// A simple "hello" command that prints a greeting.
///
/// Demonstrates:
/// - Implementing [`Command`] for metadata
/// - Implementing [`CommandHandler`] for execution
/// - Optional argument support
pub struct HelloCommand;

impl Command for HelloCommand {
    fn id(&self) -> CommandId {
        CommandId::new(EXAMPLE_MODULE, "hello")
    }

    fn description(&self) -> &'static str {
        "Print a greeting message"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional("name", ArgKind::String, "Name to greet")]
    }

    fn names(&self) -> &[&'static str] {
        &["hello", "hi", "greet"]
    }
}

impl CommandHandler for HelloCommand {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let name = args.string("name").unwrap_or("World");
        // In a real implementation, this would update the display
        // For now, we just succeed
        let _ = format!("Hello, {name}!");
        CommandResult::Success
    }
}

// ============================================================================
// QuitCommand
// ============================================================================

/// Quit the editor.
///
/// Supports `!` modifier to force quit without saving.
pub struct QuitCommand;

impl Command for QuitCommand {
    fn id(&self) -> CommandId {
        CommandId::new(EXAMPLE_MODULE, "quit")
    }

    fn description(&self) -> &'static str {
        "Quit the editor"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional("bang", ArgKind::Bang, "Force quit")]
    }

    fn names(&self) -> &[&'static str] {
        &["quit", "q"]
    }
}

impl CommandHandler for QuitCommand {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        if args.has_bang() {
            CommandResult::ForceQuit
        } else {
            CommandResult::Quit
        }
    }
}

// ============================================================================
// EchoCommand
// ============================================================================

/// Echo a message (useful for testing).
pub struct EchoCommand;

impl Command for EchoCommand {
    fn id(&self) -> CommandId {
        CommandId::new(EXAMPLE_MODULE, "echo")
    }

    fn description(&self) -> &'static str {
        "Echo a message"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::required(
            "message",
            ArgKind::String,
            "Message to echo",
        )]
    }

    fn names(&self) -> &[&'static str] {
        &["echo"]
    }
}

impl CommandHandler for EchoCommand {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        args.string("message").map_or_else(
            || CommandResult::error("Message required"),
            |_msg| {
                // In a real implementation, this would show the message
                CommandResult::Success
            },
        )
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hello_command_metadata() {
        let cmd = HelloCommand;
        assert_eq!(cmd.id().name(), "hello");
        assert_eq!(cmd.description(), "Print a greeting message");
        assert_eq!(cmd.args().len(), 1);
        assert_eq!(cmd.names(), &["hello", "hi", "greet"]);
    }

    #[test]
    fn test_quit_command_metadata() {
        let cmd = QuitCommand;
        assert_eq!(cmd.id().name(), "quit");
        assert_eq!(cmd.description(), "Quit the editor");
        assert_eq!(cmd.names(), &["quit", "q"]);
    }

    #[test]
    fn test_echo_command_metadata() {
        let cmd = EchoCommand;
        assert_eq!(cmd.id().name(), "echo");
        assert_eq!(cmd.description(), "Echo a message");
        assert_eq!(cmd.args().len(), 1);
        assert!(cmd.args()[0].required);
    }

    #[test]
    fn test_command_trait_object() {
        // Verify Command can be used as trait object
        let cmd: &dyn Command = &HelloCommand;
        assert_eq!(cmd.id().name(), "hello");
    }

    #[test]
    fn test_command_handler_trait_object() {
        // Verify CommandHandler can be used as trait object
        let cmd: &dyn CommandHandler = &HelloCommand;
        assert_eq!(cmd.id().name(), "hello");
    }

    #[test]
    fn test_all_commands_in_module() {
        // Verify all commands belong to example module
        assert_eq!(HelloCommand.id().module(), &crate::EXAMPLE_MODULE);
        assert_eq!(QuitCommand.id().module(), &crate::EXAMPLE_MODULE);
        assert_eq!(EchoCommand.id().module(), &crate::EXAMPLE_MODULE);
    }
}
