//! Undotree commands for visualization and navigation.
//!
//! These commands implement the `:undotree` ex command and navigation
//! commands for the undotree panel.
//!
//! # Architecture (Epic #385)
//!
//! These commands are stubs that will be implemented when `SessionRuntime`
//! provides direct access to undotree state. See #394 for API gap tracking.

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::SessionRuntime,
    reovim_kernel::api::v1::CommandId,
};

use crate::ids;

/// The `:undotree` command - toggles the undotree visualization panel.
pub struct UndotreeCommand;

impl Command for UndotreeCommand {
    fn id(&self) -> CommandId {
        ids::UNDOTREE
    }

    fn description(&self) -> &'static str {
        "Toggle undotree visualization panel"
    }

    fn names(&self) -> &[&'static str] {
        &["undotree", "ut"]
    }
}

impl CommandHandler for UndotreeCommand {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        CommandResult::Success
    }
}

/// Move selection down in undotree (toward children).
pub struct UndotreeDownCommand;

impl Command for UndotreeDownCommand {
    fn id(&self) -> CommandId {
        ids::UNDOTREE_DOWN
    }

    fn description(&self) -> &'static str {
        "Move selection down in undotree"
    }
}

impl CommandHandler for UndotreeDownCommand {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        CommandResult::Success
    }
}

/// Move selection up in undotree (toward parent).
pub struct UndotreeUpCommand;

impl Command for UndotreeUpCommand {
    fn id(&self) -> CommandId {
        ids::UNDOTREE_UP
    }

    fn description(&self) -> &'static str {
        "Move selection up in undotree"
    }
}

impl CommandHandler for UndotreeUpCommand {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        CommandResult::Success
    }
}

/// Navigate to the selected node in the undotree.
pub struct UndotreeGotoCommand;

impl Command for UndotreeGotoCommand {
    fn id(&self) -> CommandId {
        ids::UNDOTREE_GOTO
    }

    fn description(&self) -> &'static str {
        "Go to selected node in undotree"
    }
}

impl CommandHandler for UndotreeGotoCommand {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        CommandResult::Success
    }
}

/// Close the undotree panel.
pub struct UndotreeCloseCommand;

impl Command for UndotreeCloseCommand {
    fn id(&self) -> CommandId {
        ids::UNDOTREE_CLOSE
    }

    fn description(&self) -> &'static str {
        "Close undotree panel"
    }
}

impl CommandHandler for UndotreeCloseCommand {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        CommandResult::Success
    }
}

/// Preview the diff of the selected undotree node.
pub struct UndotreePreviewCommand;

impl Command for UndotreePreviewCommand {
    fn id(&self) -> CommandId {
        ids::UNDOTREE_PREVIEW
    }

    fn description(&self) -> &'static str {
        "Preview diff of selected undotree node"
    }
}

impl CommandHandler for UndotreePreviewCommand {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        CommandResult::Success
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::UNDOTREE_MODULE};

    #[test]
    fn test_undotree_command_id() {
        let cmd = UndotreeCommand;
        assert_eq!(cmd.id().name(), "undotree");
    }

    #[test]
    fn test_undotree_command_names() {
        let cmd = UndotreeCommand;
        assert_eq!(cmd.names(), &["undotree", "ut"]);
    }

    #[test]
    fn test_undotree_command_description() {
        let cmd = UndotreeCommand;
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn test_undotree_down_command_id() {
        let cmd = UndotreeDownCommand;
        assert_eq!(cmd.id().name(), "undotree-down");
    }

    #[test]
    fn test_undotree_up_command_id() {
        let cmd = UndotreeUpCommand;
        assert_eq!(cmd.id().name(), "undotree-up");
    }

    #[test]
    fn test_undotree_goto_command_id() {
        let cmd = UndotreeGotoCommand;
        assert_eq!(cmd.id().name(), "undotree-goto");
    }

    #[test]
    fn test_undotree_close_command_id() {
        let cmd = UndotreeCloseCommand;
        assert_eq!(cmd.id().name(), "undotree-close");
    }

    #[test]
    fn test_undotree_preview_command_id() {
        let cmd = UndotreePreviewCommand;
        assert_eq!(cmd.id().name(), "undotree-preview");
    }

    #[test]
    fn test_all_commands_have_description() {
        assert!(!UndotreeCommand.description().is_empty());
        assert!(!UndotreeDownCommand.description().is_empty());
        assert!(!UndotreeUpCommand.description().is_empty());
        assert!(!UndotreeGotoCommand.description().is_empty());
        assert!(!UndotreeCloseCommand.description().is_empty());
        assert!(!UndotreePreviewCommand.description().is_empty());
    }

    #[test]
    fn test_command_module_id() {
        // All commands should belong to undotree module
        assert_eq!(UndotreeCommand.id().module(), &UNDOTREE_MODULE);
        assert_eq!(UndotreeDownCommand.id().module(), &UNDOTREE_MODULE);
        assert_eq!(UndotreeUpCommand.id().module(), &UNDOTREE_MODULE);
        assert_eq!(UndotreeGotoCommand.id().module(), &UNDOTREE_MODULE);
        assert_eq!(UndotreeCloseCommand.id().module(), &UNDOTREE_MODULE);
        assert_eq!(UndotreePreviewCommand.id().module(), &UNDOTREE_MODULE);
    }
}
