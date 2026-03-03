//! Command and module identifiers for the completion module.

use reovim_kernel::api::v1::{CommandId, ModuleId};

/// Module identifier for completion.
pub const MODULE: ModuleId = ModuleId::new("completion");

/// Editor module ID (matches `reovim_module_editor::MODULE`).
const EDITOR: ModuleId = ModuleId::new("editor");

/// Trigger completion popup (same ID as `editor::COMPLETION_TRIGGER`).
pub const TRIGGER: CommandId = CommandId::new(EDITOR, "completion-trigger");

/// Next completion item (same ID as `editor::COMPLETION_NEXT`).
pub const NEXT: CommandId = CommandId::new(EDITOR, "completion-next");

/// Previous completion item (same ID as `editor::COMPLETION_PREV`).
pub const PREV: CommandId = CommandId::new(EDITOR, "completion-prev");

/// Confirm selected completion item.
pub const CONFIRM: CommandId = CommandId::new(MODULE, "confirm");

/// Dismiss completion popup.
pub const DISMISS: CommandId = CommandId::new(MODULE, "dismiss");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_id() {
        assert_eq!(MODULE.as_str(), "completion");
    }

    #[test]
    fn editor_command_ids() {
        // TRIGGER/NEXT/PREV re-use editor module IDs for keybinding compatibility.
        assert_eq!(TRIGGER.module(), &EDITOR);
        assert_eq!(NEXT.module(), &EDITOR);
        assert_eq!(PREV.module(), &EDITOR);
    }

    #[test]
    fn completion_command_ids() {
        assert_eq!(CONFIRM.module(), &MODULE);
        assert_eq!(DISMISS.module(), &MODULE);
    }

    #[test]
    fn command_ids_are_unique() {
        let names: Vec<&str> = vec![
            TRIGGER.name(),
            NEXT.name(),
            PREV.name(),
            CONFIRM.name(),
            DISMISS.name(),
        ];
        let mut deduped = names.clone();
        deduped.sort_unstable();
        deduped.dedup();
        assert_eq!(names.len(), deduped.len());
    }
}
