//! Command driver for reovim - command execution framework.
//!
//! Linux equivalent: `drivers/block/` (block command interface)
//!
//! # Architecture
//!
//! This crate defines the command execution framework for reovim.
//! Commands implement [`Command`] for metadata and [`CommandHandler`] for execution.
//!
//! ```text
//! lib/drivers/command/      <-- Command framework (this crate)
//!        ^
//!        |  (modules implement commands)
//!        |
//! modules/                  <-- Policy: actual command implementations
//! ```
//!
//! # Components
//!
//! - [`Command`] - Self-describing command metadata
//! - [`CommandHandler`] - Command execution trait
//! - [`ArgSpec`] - Argument specification
//! - [`ArgKind`], [`ArgValue`] - Argument types
//! - [`CommandContext`] - Context carrying all command inputs
//! - [`CommandResult`] - Command execution result
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_command::{Command, CommandHandler, CommandContext, CommandResult, ArgSpec, ArgKind};
//! use reovim_driver_session::SessionRuntime;
//! use reovim_kernel::api::v1::{CommandId, ModuleId};
//!
//! const MY_MODULE: ModuleId = ModuleId::new_const("my-module");
//!
//! pub struct CursorDown;
//!
//! impl Command for CursorDown {
//!     fn id(&self) -> CommandId {
//!         CommandId::new(MY_MODULE, "cursor-down")
//!     }
//!
//!     fn description(&self) -> &'static str {
//!         "Move cursor down"
//!     }
//!
//!     fn args(&self) -> Vec<ArgSpec> {
//!         vec![ArgSpec::optional("count", ArgKind::Count, "Number of lines")]
//!     }
//! }
//!
//! impl CommandHandler for CursorDown {
//!     fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
//!         let count = args.count().unwrap_or(1);
//!         // Move cursor down by count lines using runtime.kernel() escape hatch
//!         // or BufferApi methods
//!         CommandResult::Success
//!     }
//! }
//! ```

// Internal modules
mod ex_dispatch;
mod ex_handler;
mod ex_registry;
mod provider;
mod query;
mod registry;
mod traits;

// Re-export from command-types for backwards compatibility
pub use reovim_driver_command_types::{
    ArgKind, ArgSpec, ArgValue, CommandContext, CommandResult, MotionType,
};

// Re-export provider trait
pub use provider::CommandProvider;

// Re-export registry for ServiceRegistry (Epic #417 Part 3)
pub use registry::CommandHandlerStore;

// Re-export query service (#453)
pub use query::{CommandInfo, CommandQueryService};

// Re-export ex-command dispatcher and registry (#465)
pub use ex_dispatch::{ExCommandDispatcher, ExCommandRegistry, ExCommandResult, ExDispatchContext};

// Re-export ex-command handler (#465)
pub use ex_handler::{ExCommandContext, ExCommandError, ExCommandHandler, ExCommandRange};

// Re-export ex-command handler store (#465)
pub use ex_registry::ExCommandHandlerStore;

// Re-export traits
pub use traits::{Command, CommandHandler};

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_driver_session::SessionRuntime,
        reovim_kernel::api::v1::{CommandId, ModuleId},
    };

    // Integration test: verify trait object safety
    #[test]
    fn test_command_trait_object_safety() {
        fn _accepts_ref(_: &dyn Command) {}
        fn _accepts_box(_: Box<dyn Command>) {}
    }

    #[test]
    fn test_command_handler_trait_object_safety() {
        fn _accepts_ref(_: &dyn CommandHandler) {}
        fn _accepts_box(_: Box<dyn CommandHandler>) {}
    }

    // Integration test: verify complete command implementation
    struct TestCommand;

    impl Command for TestCommand {
        fn id(&self) -> CommandId {
            CommandId::new(ModuleId::new("test"), "test-cmd")
        }

        fn description(&self) -> &'static str {
            "A test command"
        }

        fn args(&self) -> Vec<ArgSpec> {
            vec![ArgSpec::optional("count", ArgKind::Count, "Test count")]
        }

        fn names(&self) -> &[&'static str] {
            &["test", "t"]
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

    #[test]
    fn test_command_implementation() {
        let cmd = TestCommand;
        assert_eq!(cmd.id().name(), "test-cmd");
        assert_eq!(cmd.description(), "A test command");
        assert_eq!(cmd.args().len(), 1);
        assert_eq!(cmd.names(), &["test", "t"]);
    }

    #[test]
    fn test_command_handler_as_trait_object() {
        let cmd: &dyn CommandHandler = &TestCommand;
        assert_eq!(cmd.description(), "A test command");
    }
}
