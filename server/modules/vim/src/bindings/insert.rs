//! Insert mode keybindings.
//!
//! Reference: lib/core/src/bind/mod.rs (concept-extraction, not migration)

use {reovim_kernel::api::v1::KeybindingRegistration, reovim_module_editor::ids as editor};

use crate::ids as vim;

/// Insert mode keybindings.
#[must_use]
pub fn bindings() -> Vec<KeybindingRegistration> {
    vec![
        // ====================================================================
        // Exit insert mode
        // ====================================================================
        KeybindingRegistration::new("<Esc>", vim::EXIT_INSERT)
            .with_modes(&["vim:insert"])
            .with_category("mode")
            .with_description("Exit insert mode"),
        KeybindingRegistration::new("<C-c>", vim::EXIT_INSERT)
            .with_modes(&["vim:insert"])
            .with_category("mode")
            .with_description("Exit insert mode (Ctrl-C)"),
        KeybindingRegistration::new("<C-[>", vim::EXIT_INSERT)
            .with_modes(&["vim:insert"])
            .with_category("mode")
            .with_description("Exit insert mode (Ctrl-[)"),
        // ====================================================================
        // Navigation in insert mode
        // ====================================================================
        KeybindingRegistration::new("<Left>", editor::CURSOR_LEFT)
            .with_modes(&["vim:insert"])
            .with_category("motion")
            .with_description("Move cursor left"),
        KeybindingRegistration::new("<Right>", editor::CURSOR_RIGHT)
            .with_modes(&["vim:insert"])
            .with_category("motion")
            .with_description("Move cursor right"),
        KeybindingRegistration::new("<Up>", editor::CURSOR_UP)
            .with_modes(&["vim:insert"])
            .with_category("motion")
            .with_description("Move cursor up"),
        KeybindingRegistration::new("<Down>", editor::CURSOR_DOWN)
            .with_modes(&["vim:insert"])
            .with_category("motion")
            .with_description("Move cursor down"),
        KeybindingRegistration::new("<Home>", editor::LINE_START)
            .with_modes(&["vim:insert"])
            .with_category("motion")
            .with_description("Move to start of line"),
        KeybindingRegistration::new("<End>", editor::LINE_END)
            .with_modes(&["vim:insert"])
            .with_category("motion")
            .with_description("Move to end of line"),
        // ====================================================================
        // Deletion in insert mode
        // ====================================================================
        KeybindingRegistration::new("<BS>", editor::DELETE_CHAR_BEFORE)
            .with_modes(&["vim:insert"])
            .with_category("edit")
            .with_description("Delete character before cursor"),
        KeybindingRegistration::new("<Del>", editor::DELETE_CHAR)
            .with_modes(&["vim:insert"])
            .with_category("edit")
            .with_description("Delete character under cursor"),
        KeybindingRegistration::new("<C-h>", editor::DELETE_CHAR_BEFORE)
            .with_modes(&["vim:insert"])
            .with_category("edit")
            .with_description("Delete character before cursor"),
        KeybindingRegistration::new("<C-w>", editor::DELETE_WORD_BEFORE)
            .with_modes(&["vim:insert"])
            .with_category("edit")
            .with_description("Delete word before cursor"),
        KeybindingRegistration::new("<C-u>", editor::DELETE_TO_BOL)
            .with_modes(&["vim:insert"])
            .with_category("edit")
            .with_description("Delete to start of line"),
        // ====================================================================
        // Insert operations
        // ====================================================================
        KeybindingRegistration::new("<CR>", editor::INSERT_NEWLINE)
            .with_modes(&["vim:insert"])
            .with_category("edit")
            .with_description("Insert newline"),
        KeybindingRegistration::new("<Tab>", editor::INSERT_TAB)
            .with_modes(&["vim:insert"])
            .with_category("edit")
            .with_description("Insert tab/indent"),
        // ====================================================================
        // Completion
        // ====================================================================
        KeybindingRegistration::new("<C-n>", editor::COMPLETION_NEXT)
            .with_modes(&["vim:insert"])
            .with_category("completion")
            .with_description("Next completion"),
        KeybindingRegistration::new("<C-p>", editor::COMPLETION_PREV)
            .with_modes(&["vim:insert"])
            .with_category("completion")
            .with_description("Previous completion"),
        KeybindingRegistration::new("<C-Space>", editor::COMPLETION_TRIGGER)
            .with_modes(&["vim:insert"])
            .with_category("completion")
            .with_description("Trigger completion"),
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
    fn test_all_bindings_in_insert_mode() {
        for binding in bindings() {
            assert!(
                binding.modes.contains(&"vim:insert"),
                "Binding '{}' should be in vim:insert mode",
                binding.keys
            );
        }
    }

    #[test]
    fn test_exit_keys_exist() {
        let b = bindings();
        let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
        assert!(keys.contains(&"<Esc>"), "Missing '<Esc>' binding");
        assert!(keys.contains(&"<C-c>"), "Missing '<C-c>' binding");
        assert!(keys.contains(&"<C-[>"), "Missing '<C-[>' binding");
    }

    #[test]
    fn test_navigation_keys_exist() {
        let b = bindings();
        let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
        assert!(keys.contains(&"<Left>"), "Missing '<Left>' binding");
        assert!(keys.contains(&"<Right>"), "Missing '<Right>' binding");
        assert!(keys.contains(&"<Up>"), "Missing '<Up>' binding");
        assert!(keys.contains(&"<Down>"), "Missing '<Down>' binding");
        assert!(keys.contains(&"<Home>"), "Missing '<Home>' binding");
        assert!(keys.contains(&"<End>"), "Missing '<End>' binding");
    }

    #[test]
    fn test_deletion_keys_exist() {
        let b = bindings();
        let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
        assert!(keys.contains(&"<BS>"), "Missing '<BS>' binding");
        assert!(keys.contains(&"<Del>"), "Missing '<Del>' binding");
        assert!(keys.contains(&"<C-h>"), "Missing '<C-h>' binding");
        assert!(keys.contains(&"<C-w>"), "Missing '<C-w>' binding");
        assert!(keys.contains(&"<C-u>"), "Missing '<C-u>' binding");
    }

    #[test]
    fn test_insert_operation_keys_exist() {
        let b = bindings();
        let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
        assert!(keys.contains(&"<CR>"), "Missing '<CR>' binding");
        assert!(keys.contains(&"<Tab>"), "Missing '<Tab>' binding");
    }

    #[test]
    fn test_completion_keys_exist() {
        let b = bindings();
        let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
        assert!(keys.contains(&"<C-n>"), "Missing '<C-n>' binding");
        assert!(keys.contains(&"<C-p>"), "Missing '<C-p>' binding");
        assert!(keys.contains(&"<C-Space>"), "Missing '<C-Space>' binding");
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
