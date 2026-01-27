//! Command and mode identifiers for the which-key module.

use reovim_kernel::api::v1::{CommandId, ModuleId};

/// Module identifier for which-key.
pub const MODULE: ModuleId = ModuleId::new("which-key");

/// Command to show the which-key popup immediately.
///
/// This is typically bound to `?` after a prefix key, allowing users
/// to see available bindings without waiting for the timeout.
pub const WHICH_KEY_SHOW: CommandId = CommandId::new(MODULE, "which-key-show");

/// Command to close the which-key popup.
///
/// This is typically bound to `<Escape>` when the popup is visible.
/// It cancels any pending timer and hides the popup.
pub const WHICH_KEY_CLOSE: CommandId = CommandId::new(MODULE, "which-key-close");

/// Command to filter the which-key popup by a typed key.
///
/// When the popup is visible and a key is typed that doesn't match
/// a binding, this command narrows the displayed bindings.
pub const WHICH_KEY_FILTER: CommandId = CommandId::new(MODULE, "which-key-filter");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_id() {
        assert_eq!(MODULE.as_str(), "which-key");
    }

    #[test]
    fn test_command_ids() {
        assert_eq!(WHICH_KEY_SHOW.module(), &MODULE);
        assert_eq!(WHICH_KEY_SHOW.name(), "which-key-show");

        assert_eq!(WHICH_KEY_CLOSE.module(), &MODULE);
        assert_eq!(WHICH_KEY_CLOSE.name(), "which-key-close");

        assert_eq!(WHICH_KEY_FILTER.module(), &MODULE);
        assert_eq!(WHICH_KEY_FILTER.name(), "which-key-filter");
    }
}
