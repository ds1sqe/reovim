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

/// Toggle display of gitignored files.
pub const TOGGLE_GITIGNORED: CommandId = CommandId::new(MODULE, "toggle-gitignored");

/// Mark the selected item for cut (move).
pub const CUT_MARK: CommandId = CommandId::new(MODULE, "cut-mark");

/// Paste (move) the cut-marked item to the cursor directory.
pub const PASTE: CommandId = CommandId::new(MODULE, "paste");

#[cfg(test)]
#[path = "ids_tests.rs"]
mod tests;
