//! Fold navigation commands.
//!
//! - `FoldToggleCommand` (`za`) - toggle fold at cursor
//! - `FoldOpenCommand` (`zo`) - open fold at cursor
//! - `FoldCloseCommand` (`zc`) - close fold at cursor
//! - `FoldOpenAllCommand` (`zR`) - open all folds
//! - `FoldCloseAllCommand` (`zM`) - close all folds

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{ExtensionApi, SessionRuntime},
    reovim_driver_syntax::SyntaxSessionState,
    reovim_kernel::api::v1::CommandId,
};

use super::state::FoldSessionState;

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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::Success;
        };
        let Some(window) = runtime.windows().active() else {
            return CommandResult::Success;
        };
        #[allow(clippy::cast_possible_truncation)]
        let cursor_line = window.cursor.line as u32;

        // Read fold ranges from syntax driver (immutable borrow).
        let folds = runtime
            .ext::<SyntaxSessionState>()
            .and_then(|s| s.get(buffer_id))
            .map(reovim_driver_syntax::SyntaxDriver::folds)
            .unwrap_or_default();

        // Mutate fold state.
        let fold = runtime
            .ext_mut::<FoldSessionState>()
            .get_or_insert(buffer_id);
        fold.set_ranges(folds);
        if let Some(idx) = fold.fold_at_line(cursor_line) {
            fold.toggle(idx);
        }
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::Success;
        };
        let Some(window) = runtime.windows().active() else {
            return CommandResult::Success;
        };
        #[allow(clippy::cast_possible_truncation)]
        let cursor_line = window.cursor.line as u32;

        let folds = runtime
            .ext::<SyntaxSessionState>()
            .and_then(|s| s.get(buffer_id))
            .map(reovim_driver_syntax::SyntaxDriver::folds)
            .unwrap_or_default();

        let fold = runtime
            .ext_mut::<FoldSessionState>()
            .get_or_insert(buffer_id);
        fold.set_ranges(folds);
        if let Some(idx) = fold.fold_at_line(cursor_line) {
            fold.open(idx);
        }
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::Success;
        };
        let Some(window) = runtime.windows().active() else {
            return CommandResult::Success;
        };
        #[allow(clippy::cast_possible_truncation)]
        let cursor_line = window.cursor.line as u32;

        let folds = runtime
            .ext::<SyntaxSessionState>()
            .and_then(|s| s.get(buffer_id))
            .map(reovim_driver_syntax::SyntaxDriver::folds)
            .unwrap_or_default();

        let fold = runtime
            .ext_mut::<FoldSessionState>()
            .get_or_insert(buffer_id);
        fold.set_ranges(folds);
        if let Some(idx) = fold.fold_at_line(cursor_line) {
            fold.close(idx);
        }
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::Success;
        };

        let folds = runtime
            .ext::<SyntaxSessionState>()
            .and_then(|s| s.get(buffer_id))
            .map(reovim_driver_syntax::SyntaxDriver::folds)
            .unwrap_or_default();

        let fold = runtime
            .ext_mut::<FoldSessionState>()
            .get_or_insert(buffer_id);
        fold.set_ranges(folds);
        fold.open_all();
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::Success;
        };

        let folds = runtime
            .ext::<SyntaxSessionState>()
            .and_then(|s| s.get(buffer_id))
            .map(reovim_driver_syntax::SyntaxDriver::folds)
            .unwrap_or_default();

        let fold = runtime
            .ext_mut::<FoldSessionState>()
            .get_or_insert(buffer_id);
        fold.set_ranges(folds);
        fold.close_all();
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
