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
//! server/lib/drivers/command/  <-- Command framework (this crate)
//!        ^
//!        |  (modules implement commands)
//!        |
//! server/modules/              <-- Policy: actual command implementations
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

    // Integration: verify re-exports from command-types
    #[test]
    fn test_reexported_arg_kind_variants() {
        // Verify all ArgKind variants are accessible from this crate
        let _ = ArgKind::Count;
        let _ = ArgKind::Register;
        let _ = ArgKind::Motion;
        let _ = ArgKind::Range;
        let _ = ArgKind::FilePath;
        let _ = ArgKind::String;
        let _ = ArgKind::Bang;
        let _ = ArgKind::BufferId;
        let _ = ArgKind::Char;
    }

    #[test]
    fn test_reexported_arg_value_variants() {
        // Verify all ArgValue variants are accessible
        let _ = ArgValue::Count(1);
        let _ = ArgValue::Register('a');
        let _ = ArgValue::Motion("w".to_string());
        let _ = ArgValue::Range(1, 5);
        let _ = ArgValue::FilePath("f.txt".to_string());
        let _ = ArgValue::String("s".to_string());
        let _ = ArgValue::Bang(true);
        let _ = ArgValue::BufferId(0);
        let _ = ArgValue::Char('x');
        let _ = ArgValue::Position(0, 0);
    }

    #[test]
    fn test_reexported_command_result_variants() {
        // Verify all CommandResult variants are accessible
        assert!(CommandResult::Success.is_success());
        assert!(CommandResult::Error("e".to_string()).is_error());
        assert!(CommandResult::Quit.is_quit());
        assert!(CommandResult::ForceQuit.is_quit());
        assert!(CommandResult::Detach.is_detach());
    }

    #[test]
    fn test_reexported_motion_type() {
        // Verify MotionType is accessible
        let mt = MotionType::default();
        assert!(mt.is_characterwise());
        assert!(MotionType::Linewise.is_linewise());
    }

    #[test]
    fn test_reexported_command_context() {
        // Verify CommandContext is accessible and functional
        let mut ctx = CommandContext::new();
        ctx.set("count", ArgValue::Count(3));
        assert_eq!(ctx.count(), Some(3));
    }

    #[test]
    fn test_reexported_arg_spec() {
        // Verify ArgSpec is accessible
        let spec = ArgSpec::required("count", ArgKind::Count, "Number");
        assert!(spec.required);
        let spec = ArgSpec::optional("reg", ArgKind::Register, "Register");
        assert!(!spec.required);
    }

    // Integration: verify command default impls
    struct MinimalCommand;

    impl Command for MinimalCommand {
        fn id(&self) -> CommandId {
            CommandId::new(ModuleId::new("test"), "minimal")
        }
        fn description(&self) -> &'static str {
            "Minimal"
        }
    }

    #[test]
    fn test_minimal_command_defaults() {
        let cmd = MinimalCommand;
        assert!(cmd.args().is_empty());
        assert!(cmd.names().is_empty());
    }

    // Integration: CommandHandlerStore with real commands
    #[test]
    fn test_handler_store_integration() {
        let store = CommandHandlerStore::new();
        store.add(Box::new(TestCommand));

        let handlers = store.take_handlers();
        assert_eq!(handlers.len(), 1);
        assert_eq!(handlers[0].id().name(), "test-cmd");
        assert_eq!(handlers[0].names(), &["test", "t"]);
    }

    // Integration: ExCommandHandlerStore
    #[test]
    fn test_ex_handler_store_integration() {
        struct TestExCmd;

        impl ExCommandHandler for TestExCmd {
            fn id(&self) -> &'static str {
                "test-ex"
            }
            fn names(&self) -> &[&'static str] {
                &["texcmd", "tx"]
            }
            fn execute(
                &self,
                _ctx: &mut ExCommandContext<'_>,
                _args: &[&str],
            ) -> Result<(), ExCommandError> {
                Ok(())
            }
        }

        let store = ExCommandHandlerStore::new();
        store.add(Box::new(TestExCmd));

        let handlers = store.take_handlers();
        assert_eq!(handlers.len(), 1);
        assert_eq!(handlers[0].id(), "test-ex");
        assert_eq!(handlers[0].names(), &["texcmd", "tx"]);
    }

    // Integration: ExCommandRegistry with dispatch
    #[test]
    fn test_ex_registry_dispatch_integration() {
        use reovim_kernel::api::v1::KernelContext;

        struct WriteCmd;

        impl ExCommandHandler for WriteCmd {
            fn id(&self) -> &'static str {
                "write"
            }
            fn names(&self) -> &[&'static str] {
                &["w", "write"]
            }
            fn execute(
                &self,
                _ctx: &mut ExCommandContext<'_>,
                _args: &[&str],
            ) -> Result<(), ExCommandError> {
                Ok(())
            }
        }

        let handlers: Vec<std::sync::Arc<dyn ExCommandHandler>> =
            vec![std::sync::Arc::new(WriteCmd)];
        let registry = ExCommandRegistry::from_handlers(handlers);

        let kernel = KernelContext::default();
        let ctx = ExDispatchContext::default();

        assert_eq!(registry.dispatch("w", &kernel, &ctx), ExCommandResult::Success);
        assert_eq!(registry.dispatch("write", &kernel, &ctx), ExCommandResult::Success);
        assert!(matches!(
            registry.dispatch("unknown", &kernel, &ctx),
            ExCommandResult::NotFound(_)
        ));
    }

    // Integration: CommandQueryService + CommandInfo
    #[test]
    fn test_command_info_from_test_command() {
        let cmd = TestCommand;
        let info = CommandInfo::from_command(&cmd);

        assert_eq!(info.id.name(), "test-cmd");
        assert_eq!(info.names, vec!["test", "t"]);
        assert_eq!(info.description, "A test command");
        assert_eq!(info.args.len(), 1);
        assert!(info.has_ex_names());
    }

    #[test]
    fn test_command_info_from_minimal_command() {
        let cmd = MinimalCommand;
        let info = CommandInfo::from_command(&cmd);

        assert_eq!(info.id.name(), "minimal");
        assert!(info.names.is_empty());
        assert_eq!(info.description, "Minimal");
        assert!(info.args.is_empty());
        assert!(!info.has_ex_names());
    }
}
