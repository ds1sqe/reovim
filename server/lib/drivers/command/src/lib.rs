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
mod name_index;
mod parse;
mod provider;
mod query;
mod registry;
mod traits;

// Re-export from command-types for backwards compatibility
pub use reovim_driver_command_types::{
    ArgKind, ArgSpec, ArgValue, CommandContext, CommandResult, MotionType, RuntimeSignal,
};

// Re-export provider trait
pub use provider::CommandProvider;

// Re-export registry for ServiceRegistry (Epic #417 Part 3)
pub use registry::CommandHandlerStore;

// Re-export query service (#453, #522)
pub use query::{CommandInfo, CommandQueryProvider, CommandQueryService};

// Re-export name index and cmdline parser (#547, #559, #561)
pub use {
    name_index::{AmbiguousPrefix, CommandNameIndex},
    parse::{ArgError, ParsedCmdline, bind_args, parse_cmdline, tokenize_args},
};

// Re-export traits
pub use traits::{Command, CommandHandler, CommandPriority};

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
        #[cfg_attr(coverage_nightly, coverage(off))]
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
        let _ = ArgKind::Rest;
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
        let _ = ArgValue::WindowId(0);
    }

    #[test]
    fn test_reexported_command_result_variants() {
        // Verify all CommandResult variants are accessible
        assert!(CommandResult::Success.is_success());
        assert!(CommandResult::Error("e".to_string()).is_error());
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

    // Integration: ParsedCmdline + parse_cmdline re-exports (#547)
    #[test]
    fn test_reexported_parse_cmdline() {
        let parsed = parse_cmdline("w! file.txt").unwrap();
        assert_eq!(parsed.name, "w");
        assert!(parsed.bang);
        assert_eq!(parsed.args, vec!["file.txt"]);
        assert!(parse_cmdline("").is_none());
    }

    // Integration: CommandNameIndex re-export (#547)
    #[test]
    fn test_reexported_command_name_index() {
        let mut idx = CommandNameIndex::new();
        let cmd: std::sync::Arc<dyn Command> = std::sync::Arc::new(TestCommand);
        let id = cmd.id();
        for &name in cmd.names() {
            idx.insert(name.to_string(), id.clone(), std::sync::Arc::clone(&cmd));
        }
        assert_eq!(idx.count(), 1);
        assert!(idx.resolve("test").is_some());
        assert!(idx.resolve("t").is_some());
    }

    // Integration: resolve_prefix + AmbiguousPrefix re-exports (#561)
    #[test]
    fn test_reexported_resolve_prefix() {
        let mut idx = CommandNameIndex::new();
        let cmd: std::sync::Arc<dyn Command> = std::sync::Arc::new(TestCommand);
        let id = cmd.id();
        for &name in cmd.names() {
            idx.insert(name.to_string(), id.clone(), std::sync::Arc::clone(&cmd));
        }
        // "tes" prefix resolves to TestCommand
        let result = idx.resolve_prefix("tes").unwrap().unwrap();
        assert_eq!(result.0.name(), "test-cmd");
    }

    #[test]
    fn test_reexported_ambiguous_prefix() {
        let _ = AmbiguousPrefix {
            prefix: "s".to_string(),
            candidates: vec!["set".to_string()],
        };
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
        assert!(info.has_user_names());
    }

    #[test]
    fn test_command_info_from_minimal_command() {
        let cmd = MinimalCommand;
        let info = CommandInfo::from_command(&cmd);

        assert_eq!(info.id.name(), "minimal");
        assert!(info.names.is_empty());
        assert_eq!(info.description, "Minimal");
        assert!(info.args.is_empty());
        assert!(!info.has_user_names());
    }
}
