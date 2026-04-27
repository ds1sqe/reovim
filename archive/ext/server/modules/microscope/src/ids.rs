//! Command and module identifiers for the microscope module.

use reovim_kernel::api::v1::{CommandId, ModuleId};

/// Module identifier for microscope.
pub const MODULE: ModuleId = ModuleId::new("microscope");

/// Open the file picker.
pub const OPEN_FILES: CommandId = CommandId::new(MODULE, "open-files");

/// Open the buffer picker.
pub const OPEN_BUFFERS: CommandId = CommandId::new(MODULE, "open-buffers");

/// Open the grep picker.
pub const OPEN_GREP: CommandId = CommandId::new(MODULE, "open-grep");

/// Open the command picker.
pub const OPEN_COMMANDS: CommandId = CommandId::new(MODULE, "open-commands");

/// Select the currently highlighted item.
pub const SELECT_ITEM: CommandId = CommandId::new(MODULE, "select-item");

/// Close the picker without selecting.
pub const CLOSE: CommandId = CommandId::new(MODULE, "close");

/// Move selection to the next item.
pub const NEXT_ITEM: CommandId = CommandId::new(MODULE, "next-item");

/// Move selection to the previous item.
pub const PREV_ITEM: CommandId = CommandId::new(MODULE, "prev-item");

/// Open the option picker.
pub const OPEN_OPTIONS: CommandId = CommandId::new(MODULE, "open-options");

/// Delete the character before the cursor in the query.
pub const BACKSPACE: CommandId = CommandId::new(MODULE, "backspace");

#[cfg(test)]
#[path = "ids_tests.rs"]
mod tests;
