//! Command traits for the command execution framework.
//!
//! This module defines the core traits for the command system:
//! - [`Command`] - Self-describing command metadata
//! - [`CommandHandler`] - Command execution trait

use {
    crate::{ArgSpec, CommandContext, CommandResult},
    reovim_driver_session::SessionRuntime,
    reovim_kernel::api::v1::CommandId,
};

/// Self-describing command metadata.
///
/// Commands implement this trait to provide metadata about themselves:
/// - Unique identifier ([`CommandId`])
/// - Human-readable description
/// - Argument specifications
/// - Command aliases (for ex commands like `:w`, `:write`)
///
/// # Design Philosophy
///
/// This trait separates command metadata from execution. The [`CommandHandler`]
/// trait handles actual execution. This allows querying command information
/// without executing.
///
/// # Example
///
/// ```ignore
/// use reovim_driver_command::{Command, ArgSpec, ArgKind};
/// use reovim_kernel::api::v1::{CommandId, ModuleId};
///
/// struct DeleteLine;
///
/// impl Command for DeleteLine {
///     fn id(&self) -> CommandId {
///         CommandId::new(ModuleId::new("editor"), "delete-line")
///     }
///
///     fn description(&self) -> &'static str {
///         "Delete the current line"
///     }
///
///     fn args(&self) -> Vec<ArgSpec> {
///         vec![ArgSpec::optional("count", ArgKind::Count, "Number of lines")]
///     }
/// }
/// ```
pub trait Command: Send + Sync + 'static {
    /// Get the unique identifier for this command.
    fn id(&self) -> CommandId;

    /// Get a human-readable description of what this command does.
    fn description(&self) -> &'static str;

    /// Get the argument specifications for this command.
    ///
    /// Returns an empty vector if the command takes no arguments.
    fn args(&self) -> Vec<ArgSpec> {
        vec![]
    }

    /// Get command name aliases.
    ///
    /// These are used for ex commands (command-line mode). For example,
    /// `:w` and `:write` are aliases for the same command.
    fn names(&self) -> &[&'static str] {
        &[]
    }
}

/// Command execution trait.
///
/// Commands that can be executed implement this trait in addition to [`Command`].
///
/// # Design Philosophy
///
/// Separating execution from metadata allows:
/// - Querying command info without execution capability
/// - Different execution strategies (sync, async, background)
/// - Testing command metadata independently
///
/// # Session Runtime
///
/// Commands receive a [`SessionRuntime`] which provides access to:
/// - **`ModeApi`** - Mode stack operations (push, pop, set)
/// - **`BufferApi`** - Buffer content and cursor operations
/// - **`WindowApi`** - Window management and focus
/// - **`ExtensionApi`** - Per-session module state
/// - **`ChangeTracker`** - State change accumulation
///
/// For operations not yet covered by these APIs, use the escape hatch:
/// ```ignore
/// let kernel = runtime.kernel();
/// let buffer = kernel.buffers.get(buffer_id)?;
/// ```
///
/// # Example
///
/// ```ignore
/// use reovim_driver_command::{Command, CommandHandler, CommandContext, CommandResult};
/// use reovim_driver_session::{SessionRuntime, ModeApi, TransitionContext};
/// use reovim_kernel::api::v1::{CommandId, ModuleId};
///
/// struct EnterInsertMode;
///
/// impl Command for EnterInsertMode {
///     fn id(&self) -> CommandId {
///         CommandId::new(ModuleId::new("vim"), "enter-insert-mode")
///     }
///     fn description(&self) -> &'static str { "Enter insert mode" }
/// }
///
/// impl CommandHandler for EnterInsertMode {
///     fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
///         let insert_mode = ModeId::new(ModuleId::new("vim"), "insert");
///         runtime.set_mode(insert_mode, TransitionContext::new());
///         CommandResult::Success
///     }
/// }
/// ```
pub trait CommandHandler: Command {
    /// Execute the command.
    ///
    /// # Arguments
    ///
    /// * `runtime` - Session runtime providing API trait access and kernel escape hatch
    /// * `args` - The command arguments parsed from user input
    ///
    /// # Returns
    ///
    /// A [`CommandResult`] indicating success, error, or special results like quit.
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_trait_object_safety() {
        // Verify Command trait is object-safe
        fn _accepts_ref(_: &dyn Command) {}
        fn _accepts_box(_: Box<dyn Command>) {}
    }

    #[test]
    fn test_command_handler_trait_object_safety() {
        // Verify CommandHandler trait is object-safe
        fn _accepts_ref(_: &dyn CommandHandler) {}
        fn _accepts_box(_: Box<dyn CommandHandler>) {}
    }

    #[test]
    fn test_command_handler_signature() {
        // Verify the new signature compiles with SessionRuntime
        use reovim_kernel::api::v1::ModuleId;

        struct TestCommand;

        impl Command for TestCommand {
            fn id(&self) -> CommandId {
                CommandId::new(ModuleId::new("test"), "test-cmd")
            }
            fn description(&self) -> &'static str {
                "Test command"
            }
        }

        impl CommandHandler for TestCommand {
            fn execute(
                &self,
                _runtime: &mut SessionRuntime<'_>,
                _args: &CommandContext,
            ) -> CommandResult {
                CommandResult::Success
            }
        }

        // Verify it can be used as trait object
        let cmd: &dyn CommandHandler = &TestCommand;
        assert_eq!(cmd.description(), "Test command");
    }
}
