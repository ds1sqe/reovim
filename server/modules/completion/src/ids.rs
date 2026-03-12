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
#[path = "ids_tests.rs"]
mod tests;
