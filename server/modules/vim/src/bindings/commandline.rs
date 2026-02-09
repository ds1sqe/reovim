//! Command-line mode keybindings.
//!
//! Command-line mode is entered by pressing `:` in normal mode.
//! This mode allows entering Ex commands like `:w`, `:q`, `:set`, etc.

use reovim_kernel::api::v1::KeybindingRegistration;

use crate::ids as vim;

/// Command-line mode keybindings.
#[must_use]
pub fn bindings() -> Vec<KeybindingRegistration> {
    vec![
        // ====================================================================
        // Cancel command-line mode (Escape)
        // ====================================================================
        KeybindingRegistration::new("<Esc>", vim::CANCEL_COMMANDLINE)
            .with_modes(&["vim:command"])
            .with_category("mode")
            .with_description("Cancel and exit command-line mode"),
        KeybindingRegistration::new("<C-c>", vim::CANCEL_COMMANDLINE)
            .with_modes(&["vim:command"])
            .with_category("mode")
            .with_description("Cancel and exit command-line mode"),
        // ====================================================================
        // Execute command-line (Enter)
        // ====================================================================
        KeybindingRegistration::new("<CR>", vim::EXIT_COMMANDLINE)
            .with_modes(&["vim:command"])
            .with_category("mode")
            .with_description("Execute command and exit command-line mode"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bindings_not_empty() {
        let b = bindings();
        assert!(!b.is_empty());
    }

    #[test]
    fn test_bindings_count() {
        let b = bindings();
        assert_eq!(b.len(), 3);
    }

    #[test]
    fn test_all_bindings_in_command_mode() {
        for binding in bindings() {
            assert!(
                binding.modes.contains(&"vim:command"),
                "Binding '{}' should be in vim:command mode",
                binding.keys
            );
        }
    }

    #[test]
    fn test_escape_binding_exists() {
        let b = bindings();
        assert!(b.iter().any(|kb| kb.keys == "<Esc>"), "Missing '<Esc>' binding");
    }

    #[test]
    fn test_ctrl_c_binding_exists() {
        let b = bindings();
        assert!(b.iter().any(|kb| kb.keys == "<C-c>"), "Missing '<C-c>' binding");
    }

    #[test]
    fn test_enter_binding_exists() {
        let b = bindings();
        assert!(b.iter().any(|kb| kb.keys == "<CR>"), "Missing '<CR>' binding");
    }

    #[test]
    fn test_all_bindings_have_description() {
        for binding in bindings() {
            assert!(
                !binding.description.is_empty(),
                "Binding '{}' should have a description",
                binding.keys
            );
        }
    }

    #[test]
    fn test_all_bindings_have_category() {
        for binding in bindings() {
            assert!(
                binding.category.is_some(),
                "Binding '{}' should have a category",
                binding.keys
            );
        }
    }
}
