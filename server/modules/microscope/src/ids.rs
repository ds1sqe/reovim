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

/// Delete the character before the cursor in the query.
pub const BACKSPACE: CommandId = CommandId::new(MODULE, "backspace");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_id() {
        assert_eq!(MODULE.as_str(), "microscope");
    }

    #[test]
    fn command_ids_have_correct_module() {
        let commands = [
            OPEN_FILES,
            OPEN_BUFFERS,
            OPEN_GREP,
            OPEN_COMMANDS,
            SELECT_ITEM,
            CLOSE,
            NEXT_ITEM,
            PREV_ITEM,
            BACKSPACE,
        ];
        for cmd in &commands {
            assert_eq!(cmd.module(), &MODULE);
        }
    }

    #[test]
    fn command_ids_are_unique() {
        let names: Vec<&str> = vec![
            OPEN_FILES.name(),
            OPEN_BUFFERS.name(),
            OPEN_GREP.name(),
            OPEN_COMMANDS.name(),
            SELECT_ITEM.name(),
            CLOSE.name(),
            NEXT_ITEM.name(),
            PREV_ITEM.name(),
            BACKSPACE.name(),
        ];
        let mut deduped = names.clone();
        deduped.sort_unstable();
        deduped.dedup();
        assert_eq!(names.len(), deduped.len());
    }
}
