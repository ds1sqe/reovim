//! Command and module identifiers for the explorer module.

use reovim_kernel::api::v1::{CommandId, ModuleId};

/// Module identifier for explorer.
pub const MODULE: ModuleId = ModuleId::new("explorer");

/// Toggle the explorer sidebar visibility.
pub const TOGGLE: CommandId = CommandId::new(MODULE, "toggle");

/// Close the explorer sidebar.
pub const CLOSE: CommandId = CommandId::new(MODULE, "close");

/// Move cursor up in the tree.
pub const CURSOR_UP: CommandId = CommandId::new(MODULE, "cursor-up");

/// Move cursor down in the tree.
pub const CURSOR_DOWN: CommandId = CommandId::new(MODULE, "cursor-down");

/// Open the selected file or toggle directory.
pub const OPEN: CommandId = CommandId::new(MODULE, "open");

/// Expand a directory node.
pub const EXPAND: CommandId = CommandId::new(MODULE, "expand");

/// Collapse a directory node.
pub const COLLAPSE: CommandId = CommandId::new(MODULE, "collapse");

/// Navigate to the parent directory.
pub const GOTO_PARENT: CommandId = CommandId::new(MODULE, "goto-parent");

/// Go to the first item in the tree.
pub const GOTO_FIRST: CommandId = CommandId::new(MODULE, "goto-first");

/// Go to the last item in the tree.
pub const GOTO_LAST: CommandId = CommandId::new(MODULE, "goto-last");

/// Toggle display of hidden files.
pub const TOGGLE_HIDDEN: CommandId = CommandId::new(MODULE, "toggle-hidden");

/// Refresh the tree from filesystem.
pub const REFRESH: CommandId = CommandId::new(MODULE, "refresh");

/// Begin creating a new file (enter input mode).
pub const CREATE_FILE: CommandId = CommandId::new(MODULE, "create-file");

/// Begin creating a new directory (enter input mode).
pub const CREATE_DIR: CommandId = CommandId::new(MODULE, "create-dir");

/// Begin renaming the selected item (enter input mode).
pub const RENAME: CommandId = CommandId::new(MODULE, "rename");

/// Begin deleting the selected item (enter confirm mode).
pub const DELETE: CommandId = CommandId::new(MODULE, "delete");

/// Confirm the current input operation.
pub const CONFIRM_INPUT: CommandId = CommandId::new(MODULE, "confirm-input");

/// Cancel the current input operation.
pub const CANCEL_INPUT: CommandId = CommandId::new(MODULE, "cancel-input");

/// Delete the last character from input buffer.
pub const INPUT_BACKSPACE: CommandId = CommandId::new(MODULE, "input-backspace");

/// Copy the selected item's path to clipboard.
pub const YANK_PATH: CommandId = CommandId::new(MODULE, "yank-path");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_id() {
        assert_eq!(MODULE.as_str(), "explorer");
    }

    #[test]
    fn command_ids_have_correct_module() {
        let commands = [
            TOGGLE,
            CLOSE,
            CURSOR_UP,
            CURSOR_DOWN,
            OPEN,
            EXPAND,
            COLLAPSE,
            GOTO_PARENT,
            GOTO_FIRST,
            GOTO_LAST,
            TOGGLE_HIDDEN,
            REFRESH,
            CREATE_FILE,
            CREATE_DIR,
            RENAME,
            DELETE,
            CONFIRM_INPUT,
            CANCEL_INPUT,
            INPUT_BACKSPACE,
            YANK_PATH,
        ];
        for cmd in &commands {
            assert_eq!(cmd.module(), &MODULE);
        }
    }

    #[test]
    fn command_ids_are_unique() {
        let names: Vec<&str> = vec![
            TOGGLE.name(),
            CLOSE.name(),
            CURSOR_UP.name(),
            CURSOR_DOWN.name(),
            OPEN.name(),
            EXPAND.name(),
            COLLAPSE.name(),
            GOTO_PARENT.name(),
            GOTO_FIRST.name(),
            GOTO_LAST.name(),
            TOGGLE_HIDDEN.name(),
            REFRESH.name(),
            CREATE_FILE.name(),
            CREATE_DIR.name(),
            RENAME.name(),
            DELETE.name(),
            CONFIRM_INPUT.name(),
            CANCEL_INPUT.name(),
            INPUT_BACKSPACE.name(),
            YANK_PATH.name(),
        ];
        let mut deduped = names.clone();
        deduped.sort_unstable();
        deduped.dedup();
        assert_eq!(names.len(), deduped.len());
    }
}
