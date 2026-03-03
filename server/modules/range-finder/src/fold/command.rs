//! Fold navigation commands.
//!
//! - `FoldToggleCommand` (`za`) - toggle fold at cursor
//! - `FoldOpenCommand` (`zo`) - open fold at cursor
//! - `FoldCloseCommand` (`zc`) - close fold at cursor
//! - `FoldOpenAllCommand` (`zR`) - open all folds
//! - `FoldCloseAllCommand` (`zM`) - close all folds

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::SessionRuntime,
    reovim_kernel::api::v1::CommandId,
};

use super::ids;

/// Toggle fold at cursor line (`za`).
#[derive(Debug, Clone, Copy, Default)]
pub struct FoldToggleCommand;

impl Command for FoldToggleCommand {
    fn id(&self) -> CommandId {
        ids::FOLD_TOGGLE
    }

    fn description(&self) -> &'static str {
        "Toggle fold at cursor"
    }
}

impl CommandHandler for FoldToggleCommand {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Stub: will get cursor line, find fold, toggle via FoldSessionState
        CommandResult::Success
    }
}

/// Open fold at cursor line (`zo`).
#[derive(Debug, Clone, Copy, Default)]
pub struct FoldOpenCommand;

impl Command for FoldOpenCommand {
    fn id(&self) -> CommandId {
        ids::FOLD_OPEN
    }

    fn description(&self) -> &'static str {
        "Open fold at cursor"
    }
}

impl CommandHandler for FoldOpenCommand {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        CommandResult::Success
    }
}

/// Close fold at cursor line (`zc`).
#[derive(Debug, Clone, Copy, Default)]
pub struct FoldCloseCommand;

impl Command for FoldCloseCommand {
    fn id(&self) -> CommandId {
        ids::FOLD_CLOSE
    }

    fn description(&self) -> &'static str {
        "Close fold at cursor"
    }
}

impl CommandHandler for FoldCloseCommand {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        CommandResult::Success
    }
}

/// Open all folds in buffer (`zR`).
#[derive(Debug, Clone, Copy, Default)]
pub struct FoldOpenAllCommand;

impl Command for FoldOpenAllCommand {
    fn id(&self) -> CommandId {
        ids::FOLD_OPEN_ALL
    }

    fn description(&self) -> &'static str {
        "Open all folds"
    }
}

impl CommandHandler for FoldOpenAllCommand {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        CommandResult::Success
    }
}

/// Close all folds in buffer (`zM`).
#[derive(Debug, Clone, Copy, Default)]
pub struct FoldCloseAllCommand;

impl Command for FoldCloseAllCommand {
    fn id(&self) -> CommandId {
        ids::FOLD_CLOSE_ALL
    }

    fn description(&self) -> &'static str {
        "Close all folds"
    }
}

impl CommandHandler for FoldCloseAllCommand {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        CommandResult::Success
    }
}

/// Return all fold command handlers.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(FoldToggleCommand),
        Box::new(FoldOpenCommand),
        Box::new(FoldCloseCommand),
        Box::new(FoldOpenAllCommand),
        Box::new(FoldCloseAllCommand),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fold_toggle_command_metadata() {
        let cmd = FoldToggleCommand;
        assert_eq!(cmd.id(), ids::FOLD_TOGGLE);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn test_fold_open_command_metadata() {
        let cmd = FoldOpenCommand;
        assert_eq!(cmd.id(), ids::FOLD_OPEN);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn test_fold_close_command_metadata() {
        let cmd = FoldCloseCommand;
        assert_eq!(cmd.id(), ids::FOLD_CLOSE);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn test_fold_open_all_command_metadata() {
        let cmd = FoldOpenAllCommand;
        assert_eq!(cmd.id(), ids::FOLD_OPEN_ALL);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn test_fold_close_all_command_metadata() {
        let cmd = FoldCloseAllCommand;
        assert_eq!(cmd.id(), ids::FOLD_CLOSE_ALL);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn test_all_commands_count() {
        let commands = all_commands();
        assert_eq!(commands.len(), 5);
    }

    #[test]
    fn test_all_commands_unique_ids() {
        let commands = all_commands();
        let ids: Vec<CommandId> = commands.iter().map(|c| c.id()).collect();
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                assert_ne!(ids[i], ids[j]);
            }
        }
    }
}
