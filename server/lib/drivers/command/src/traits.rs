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
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_command_trait_object_safety() {
        // Verify Command trait is object-safe (inner fns never called)
        fn _accepts_ref(_: &dyn Command) {}
        fn _accepts_box(_: Box<dyn Command>) {}
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_command_handler_trait_object_safety() {
        // Verify CommandHandler trait is object-safe (inner fns never called)
        fn _accepts_ref(_: &dyn CommandHandler) {}
        fn _accepts_box(_: Box<dyn CommandHandler>) {}
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
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

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_command_default_args_is_empty() {
        use reovim_kernel::api::v1::ModuleId;

        struct MinimalCommand;

        impl Command for MinimalCommand {
            fn id(&self) -> CommandId {
                CommandId::new(ModuleId::new("test"), "minimal")
            }
            fn description(&self) -> &'static str {
                "A minimal command"
            }
        }

        let cmd = MinimalCommand;
        assert!(cmd.args().is_empty());
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_command_default_names_is_empty() {
        use reovim_kernel::api::v1::ModuleId;

        struct MinimalCommand;

        impl Command for MinimalCommand {
            fn id(&self) -> CommandId {
                CommandId::new(ModuleId::new("test"), "minimal")
            }
            fn description(&self) -> &'static str {
                "A minimal command"
            }
        }

        let cmd = MinimalCommand;
        assert!(cmd.names().is_empty());
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_command_with_custom_args() {
        use {crate::ArgKind, reovim_kernel::api::v1::ModuleId};

        struct ArgsCommand;

        impl Command for ArgsCommand {
            fn id(&self) -> CommandId {
                CommandId::new(ModuleId::new("test"), "args-cmd")
            }
            fn description(&self) -> &'static str {
                "Command with args"
            }
            fn args(&self) -> Vec<ArgSpec> {
                vec![
                    ArgSpec::required("count", ArgKind::Count, "Number of times"),
                    ArgSpec::optional("register", ArgKind::Register, "Target register"),
                ]
            }
        }

        let cmd = ArgsCommand;
        let args = cmd.args();
        assert_eq!(args.len(), 2);
        assert!(args[0].required);
        assert!(!args[1].required);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_command_with_custom_names() {
        use reovim_kernel::api::v1::ModuleId;

        struct NamesCommand;

        impl Command for NamesCommand {
            fn id(&self) -> CommandId {
                CommandId::new(ModuleId::new("test"), "names-cmd")
            }
            fn description(&self) -> &'static str {
                "Command with names"
            }
            fn names(&self) -> &[&'static str] {
                &["w", "write", "wr"]
            }
        }

        let cmd = NamesCommand;
        assert_eq!(cmd.names(), &["w", "write", "wr"]);
        assert_eq!(cmd.names().len(), 3);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_command_handler_upcasts_to_command() {
        use reovim_kernel::api::v1::ModuleId;

        struct TestCmd;

        impl Command for TestCmd {
            fn id(&self) -> CommandId {
                CommandId::new(ModuleId::new("test"), "test-upcast")
            }
            fn description(&self) -> &'static str {
                "Upcast test"
            }
        }

        impl CommandHandler for TestCmd {
            fn execute(
                &self,
                _runtime: &mut SessionRuntime<'_>,
                _args: &CommandContext,
            ) -> CommandResult {
                CommandResult::Quit
            }
        }

        // CommandHandler implies Command, so we can call Command methods
        let handler: &dyn CommandHandler = &TestCmd;
        assert_eq!(handler.id().name(), "test-upcast");
        assert_eq!(handler.description(), "Upcast test");
        assert!(handler.args().is_empty());
        assert!(handler.names().is_empty());
    }

    // ========================================================================
    // Command trait default method coverage
    // ========================================================================

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_command_default_args_returns_empty_vec() {
        use reovim_kernel::api::v1::ModuleId;

        struct NoArgsCommand;
        impl Command for NoArgsCommand {
            fn id(&self) -> CommandId {
                CommandId::new(ModuleId::new("test"), "no-args")
            }
            fn description(&self) -> &'static str {
                "No args"
            }
        }

        let cmd = NoArgsCommand;
        let args = cmd.args();
        assert!(args.is_empty());
        assert_eq!(args.len(), 0);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_command_default_names_returns_empty_slice() {
        use reovim_kernel::api::v1::ModuleId;

        struct NoNamesCommand;
        impl Command for NoNamesCommand {
            fn id(&self) -> CommandId {
                CommandId::new(ModuleId::new("test"), "no-names")
            }
            fn description(&self) -> &'static str {
                "No names"
            }
        }

        let cmd = NoNamesCommand;
        let names = cmd.names();
        assert!(names.is_empty());
        assert_eq!(names.len(), 0);
    }

    // ========================================================================
    // Command with custom args and names
    // ========================================================================

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_command_with_multiple_args() {
        use {crate::ArgKind, reovim_kernel::api::v1::ModuleId};

        struct MultiArgCommand;
        impl Command for MultiArgCommand {
            fn id(&self) -> CommandId {
                CommandId::new(ModuleId::new("test"), "multi-arg")
            }
            fn description(&self) -> &'static str {
                "Multi arg command"
            }
            fn args(&self) -> Vec<ArgSpec> {
                vec![
                    ArgSpec::required("count", ArgKind::Count, "Number of times"),
                    ArgSpec::optional("register", ArgKind::Register, "Target register"),
                    ArgSpec::optional("file", ArgKind::FilePath, "File path"),
                ]
            }
        }

        let cmd = MultiArgCommand;
        let args = cmd.args();
        assert_eq!(args.len(), 3);
        assert!(args[0].required);
        assert!(!args[1].required);
        assert!(!args[2].required);
        assert_eq!(args[0].name, "count");
        assert_eq!(args[1].name, "register");
        assert_eq!(args[2].name, "file");
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_command_with_single_name() {
        use reovim_kernel::api::v1::ModuleId;

        struct SingleNameCommand;
        impl Command for SingleNameCommand {
            fn id(&self) -> CommandId {
                CommandId::new(ModuleId::new("test"), "single-name")
            }
            fn description(&self) -> &'static str {
                "Single name"
            }
            fn names(&self) -> &[&'static str] {
                &["q"]
            }
        }

        let cmd = SingleNameCommand;
        assert_eq!(cmd.names().len(), 1);
        assert_eq!(cmd.names()[0], "q");
    }

    // ========================================================================
    // CommandHandler with different result types
    // ========================================================================

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_command_handler_returns_error() {
        use reovim_kernel::api::v1::ModuleId;

        struct ErrorCommand;
        impl Command for ErrorCommand {
            fn id(&self) -> CommandId {
                CommandId::new(ModuleId::new("test"), "error-cmd")
            }
            fn description(&self) -> &'static str {
                "Error command"
            }
        }
        impl CommandHandler for ErrorCommand {
            fn execute(
                &self,
                _runtime: &mut SessionRuntime<'_>,
                _args: &CommandContext,
            ) -> CommandResult {
                CommandResult::Error("something went wrong".to_string())
            }
        }

        let cmd: &dyn CommandHandler = &ErrorCommand;
        assert_eq!(cmd.id().name(), "error-cmd");
        assert_eq!(cmd.description(), "Error command");
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_command_handler_returns_force_quit() {
        use reovim_kernel::api::v1::ModuleId;

        struct ForceQuitCommand;
        impl Command for ForceQuitCommand {
            fn id(&self) -> CommandId {
                CommandId::new(ModuleId::new("test"), "force-quit")
            }
            fn description(&self) -> &'static str {
                "Force quit"
            }
        }
        impl CommandHandler for ForceQuitCommand {
            fn execute(
                &self,
                _runtime: &mut SessionRuntime<'_>,
                _args: &CommandContext,
            ) -> CommandResult {
                CommandResult::ForceQuit
            }
        }

        let cmd: &dyn CommandHandler = &ForceQuitCommand;
        assert_eq!(cmd.id().name(), "force-quit");
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_command_handler_returns_detach() {
        use reovim_kernel::api::v1::ModuleId;

        struct DetachCommand;
        impl Command for DetachCommand {
            fn id(&self) -> CommandId {
                CommandId::new(ModuleId::new("test"), "detach")
            }
            fn description(&self) -> &'static str {
                "Detach"
            }
        }
        impl CommandHandler for DetachCommand {
            fn execute(
                &self,
                _runtime: &mut SessionRuntime<'_>,
                _args: &CommandContext,
            ) -> CommandResult {
                CommandResult::Detach
            }
        }

        let cmd: &dyn CommandHandler = &DetachCommand;
        assert_eq!(cmd.id().name(), "detach");
    }

    // ========================================================================
    // Command trait object with id() comparison
    // ========================================================================

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_command_id_module_and_name() {
        use reovim_kernel::api::v1::ModuleId;

        struct DetailedCommand;
        impl Command for DetailedCommand {
            fn id(&self) -> CommandId {
                CommandId::new(ModuleId::new("my-module"), "my-command")
            }
            fn description(&self) -> &'static str {
                "Detailed"
            }
        }

        let cmd = DetailedCommand;
        assert_eq!(cmd.id().name(), "my-command");
        assert_eq!(cmd.id().module().as_str(), "my-module");
    }

    // ========================================================================
    // Command box vs ref trait object
    // ========================================================================

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_command_as_boxed_trait_object() {
        use reovim_kernel::api::v1::ModuleId;

        struct BoxableCommand;
        impl Command for BoxableCommand {
            fn id(&self) -> CommandId {
                CommandId::new(ModuleId::new("test"), "boxable")
            }
            fn description(&self) -> &'static str {
                "Boxable"
            }
            fn args(&self) -> Vec<ArgSpec> {
                vec![]
            }
            fn names(&self) -> &[&'static str] {
                &["boxable"]
            }
        }

        let cmd: Box<dyn Command> = Box::new(BoxableCommand);
        assert_eq!(cmd.id().name(), "boxable");
        assert_eq!(cmd.description(), "Boxable");
        assert!(cmd.args().is_empty());
        assert_eq!(cmd.names(), &["boxable"]);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_command_handler_as_boxed_trait_object() {
        use reovim_kernel::api::v1::ModuleId;

        struct BoxableHandler;
        impl Command for BoxableHandler {
            fn id(&self) -> CommandId {
                CommandId::new(ModuleId::new("test"), "boxable-handler")
            }
            fn description(&self) -> &'static str {
                "Boxable handler"
            }
        }
        impl CommandHandler for BoxableHandler {
            fn execute(
                &self,
                _runtime: &mut SessionRuntime<'_>,
                _args: &CommandContext,
            ) -> CommandResult {
                CommandResult::Success
            }
        }

        let cmd: Box<dyn CommandHandler> = Box::new(BoxableHandler);
        assert_eq!(cmd.id().name(), "boxable-handler");
        assert_eq!(cmd.description(), "Boxable handler");
    }
}
