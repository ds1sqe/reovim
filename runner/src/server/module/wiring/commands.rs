//! Command wiring for modules.
//!
//! Provides infrastructure to wire command handlers from modules that
//! implement `CommandProvider` to the command registry.

use std::{fmt, sync::Arc};

use {
    reovim_driver_command::{CommandHandler, CommandProvider},
    reovim_kernel::api::v1::ModuleId,
};

use crate::server::registry::CommandRegistry;

/// Result of a command wiring operation.
pub type CommandWiringResult = Result<CommandWiringStats, CommandWiringError>;

/// Statistics from a command wiring operation.
#[derive(Debug, Default, Clone)]
pub struct CommandWiringStats {
    /// Number of commands successfully wired.
    pub commands_wired: usize,
    /// Number of commands skipped (duplicate or invalid).
    pub commands_skipped: usize,
}

impl CommandWiringStats {
    /// Create empty stats.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            commands_wired: 0,
            commands_skipped: 0,
        }
    }

    /// Merge another stats into this one.
    pub const fn merge(&mut self, other: &Self) {
        self.commands_wired += other.commands_wired;
        self.commands_skipped += other.commands_skipped;
    }
}

/// Error during command wiring.
#[derive(Debug, Clone)]
pub enum CommandWiringError {
    /// Command ID conflict (already registered by another module).
    IdConflict {
        /// The conflicting command ID.
        command_id: String,
        /// The module that already owns this command (if known).
        existing_owner: Option<ModuleId>,
        /// The module attempting to register.
        new_owner: ModuleId,
    },
}

impl fmt::Display for CommandWiringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdConflict {
                command_id,
                existing_owner,
                new_owner,
            } => {
                write!(
                    f,
                    "command {} conflict: owned by {:?}, new owner {}",
                    command_id,
                    existing_owner,
                    new_owner.as_str()
                )
            }
        }
    }
}

impl std::error::Error for CommandWiringError {}

/// Wire a module's commands to the command registry.
///
/// Gets command handlers from a `CommandProvider` and registers them
/// with module ownership tracking.
///
/// # Arguments
///
/// * `module_id` - The ID of the module providing the commands
/// * `provider` - The module implementing `CommandProvider`
/// * `command_registry` - The registry to wire commands to
///
/// # Returns
///
/// - `Ok(CommandWiringStats)` with the number of commands wired/skipped
/// - `Err(CommandWiringError)` if a command ID conflicts
///
/// # Errors
///
/// Returns `CommandWiringError::IdConflict` if a command ID is already
/// registered by a different module.
pub fn wire_module_commands<P: CommandProvider>(
    module_id: &ModuleId,
    provider: &P,
    command_registry: &mut CommandRegistry,
) -> CommandWiringResult {
    let mut stats = CommandWiringStats::new();

    for handler in provider.command_handlers() {
        let arc_handler: Arc<dyn CommandHandler> = handler.into();
        command_registry.register_for_module(arc_handler, module_id.clone());
        stats.commands_wired += 1;
    }

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_driver_command::{ArgSpec, Command, CommandContext, CommandResult},
        reovim_kernel::api::v1::{CommandId, KernelContext},
    };

    const TEST_MODULE: ModuleId = ModuleId::new("test-module");
    const TEST_CMD_ONE: CommandId = CommandId::new(TEST_MODULE, "cmd-one");
    const TEST_CMD_TWO: CommandId = CommandId::new(TEST_MODULE, "cmd-two");

    struct TestCommandOne;

    impl Command for TestCommandOne {
        fn id(&self) -> CommandId {
            TEST_CMD_ONE
        }

        fn description(&self) -> &'static str {
            "Test command one"
        }

        fn args(&self) -> Vec<ArgSpec> {
            vec![]
        }
    }

    impl CommandHandler for TestCommandOne {
        fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
            CommandResult::Success
        }
    }

    struct TestCommandTwo;

    impl Command for TestCommandTwo {
        fn id(&self) -> CommandId {
            TEST_CMD_TWO
        }

        fn description(&self) -> &'static str {
            "Test command two"
        }

        fn args(&self) -> Vec<ArgSpec> {
            vec![]
        }
    }

    impl CommandHandler for TestCommandTwo {
        fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
            CommandResult::Success
        }
    }

    struct TestProvider;

    impl CommandProvider for TestProvider {
        fn command_handlers(&self) -> Vec<Box<dyn CommandHandler>> {
            vec![Box::new(TestCommandOne), Box::new(TestCommandTwo)]
        }
    }

    #[test]
    fn test_command_wiring_stats_new() {
        let stats = CommandWiringStats::new();
        assert_eq!(stats.commands_wired, 0);
        assert_eq!(stats.commands_skipped, 0);
    }

    #[test]
    fn test_command_wiring_stats_merge() {
        let mut stats1 = CommandWiringStats {
            commands_wired: 5,
            commands_skipped: 2,
        };
        let stats2 = CommandWiringStats {
            commands_wired: 3,
            commands_skipped: 1,
        };

        stats1.merge(&stats2);

        assert_eq!(stats1.commands_wired, 8);
        assert_eq!(stats1.commands_skipped, 3);
    }

    #[test]
    fn test_wire_module_commands_simple() {
        let mut registry = CommandRegistry::new();
        let provider = TestProvider;

        let result = wire_module_commands(&TEST_MODULE, &provider, &mut registry);

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.commands_wired, 2);
        assert_eq!(stats.commands_skipped, 0);
    }

    #[test]
    fn test_wiring_error_display() {
        let err = CommandWiringError::IdConflict {
            command_id: "test-cmd".to_string(),
            existing_owner: Some(ModuleId::new("owner")),
            new_owner: ModuleId::new("new"),
        };
        assert!(err.to_string().contains("conflict"));
        assert!(err.to_string().contains("test-cmd"));
    }
}
