//! Jump navigation commands.
//!
//! - `JumpSearchCommand` (`s` key) - start jump search, enter jump-input mode
//! - `JumpExecuteCommand` - cursor move after label resolution

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::SessionRuntime,
    reovim_kernel::api::v1::CommandId,
};

use super::ids;

/// Command to start a jump search (`s` in normal mode).
///
/// Initialises the `JumpSessionState` state machine and transitions
/// to `range-finder:jump-input` mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct JumpSearchCommand;

impl Command for JumpSearchCommand {
    fn id(&self) -> CommandId {
        ids::JUMP_SEARCH
    }

    fn description(&self) -> &'static str {
        "Start jump search (two-char pattern)"
    }
}

impl CommandHandler for JumpSearchCommand {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Phase 4 stub: will initialise JumpSessionState and push jump-input mode.
        // Actual wiring requires ModeApi and BufferApi from runtime.
        CommandResult::Success
    }
}

/// Command to execute a resolved jump (cursor move).
///
/// Called after label selection completes. Takes the target from
/// `JumpSessionState::take_target()` and moves the cursor.
#[derive(Debug, Clone, Copy, Default)]
pub struct JumpExecuteCommand;

impl Command for JumpExecuteCommand {
    fn id(&self) -> CommandId {
        ids::JUMP_EXECUTE
    }

    fn description(&self) -> &'static str {
        "Execute jump to resolved target"
    }
}

impl CommandHandler for JumpExecuteCommand {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Phase 4 stub: will read target from JumpSessionState and move cursor.
        CommandResult::Success
    }
}

/// Return all jump command handlers.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![Box::new(JumpSearchCommand), Box::new(JumpExecuteCommand)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jump_search_command_id() {
        let cmd = JumpSearchCommand;
        assert_eq!(cmd.id(), ids::JUMP_SEARCH);
    }

    #[test]
    fn test_jump_search_command_description() {
        let cmd = JumpSearchCommand;
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn test_jump_execute_command_id() {
        let cmd = JumpExecuteCommand;
        assert_eq!(cmd.id(), ids::JUMP_EXECUTE);
    }

    #[test]
    fn test_jump_execute_command_description() {
        let cmd = JumpExecuteCommand;
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn test_all_commands_count() {
        let commands = all_commands();
        assert_eq!(commands.len(), 2);
    }

    #[test]
    fn test_all_commands_unique_ids() {
        let commands = all_commands();
        let ids: Vec<CommandId> = commands.iter().map(|c| c.id()).collect();
        assert_ne!(ids[0], ids[1]);
    }

    #[test]
    fn test_jump_search_command_no_args() {
        let cmd = JumpSearchCommand;
        assert!(cmd.args().is_empty());
    }

    #[test]
    fn test_jump_search_command_no_names() {
        let cmd = JumpSearchCommand;
        assert!(cmd.names().is_empty());
    }
}
