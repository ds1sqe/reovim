//! Insert mode keybindings.
//!
//! Reference: lib/core/src/bind/mod.rs (concept-extraction, not migration)

use reovim_kernel::api::v1::KeybindingRegistration;

/// Insert mode keybindings.
pub fn bindings() -> Vec<KeybindingRegistration> {
    vec![
        // ====================================================================
        // Exit insert mode
        // ====================================================================
        KeybindingRegistration::new("<Esc>", "exit-insert")
            .with_modes(&["vim:insert"])
            .with_category("mode")
            .with_description("Exit insert mode"),
        KeybindingRegistration::new("<C-c>", "exit-insert")
            .with_modes(&["vim:insert"])
            .with_category("mode")
            .with_description("Exit insert mode (Ctrl-C)"),
        KeybindingRegistration::new("<C-[>", "exit-insert")
            .with_modes(&["vim:insert"])
            .with_category("mode")
            .with_description("Exit insert mode (Ctrl-[)"),
        // ====================================================================
        // Navigation in insert mode
        // ====================================================================
        KeybindingRegistration::new("<Left>", "editor:cursor-left")
            .with_modes(&["vim:insert"])
            .with_category("motion")
            .with_description("Move cursor left"),
        KeybindingRegistration::new("<Right>", "editor:cursor-right")
            .with_modes(&["vim:insert"])
            .with_category("motion")
            .with_description("Move cursor right"),
        KeybindingRegistration::new("<Up>", "editor:cursor-up")
            .with_modes(&["vim:insert"])
            .with_category("motion")
            .with_description("Move cursor up"),
        KeybindingRegistration::new("<Down>", "editor:cursor-down")
            .with_modes(&["vim:insert"])
            .with_category("motion")
            .with_description("Move cursor down"),
        KeybindingRegistration::new("<Home>", "editor:line-start")
            .with_modes(&["vim:insert"])
            .with_category("motion")
            .with_description("Move to start of line"),
        KeybindingRegistration::new("<End>", "editor:line-end")
            .with_modes(&["vim:insert"])
            .with_category("motion")
            .with_description("Move to end of line"),
        // ====================================================================
        // Deletion in insert mode
        // ====================================================================
        KeybindingRegistration::new("<BS>", "editor:delete-char-before")
            .with_modes(&["vim:insert"])
            .with_category("edit")
            .with_description("Delete character before cursor"),
        KeybindingRegistration::new("<Del>", "editor:delete-char")
            .with_modes(&["vim:insert"])
            .with_category("edit")
            .with_description("Delete character under cursor"),
        KeybindingRegistration::new("<C-h>", "editor:delete-char-before")
            .with_modes(&["vim:insert"])
            .with_category("edit")
            .with_description("Delete character before cursor"),
        KeybindingRegistration::new("<C-w>", "editor:delete-word-before")
            .with_modes(&["vim:insert"])
            .with_category("edit")
            .with_description("Delete word before cursor"),
        KeybindingRegistration::new("<C-u>", "editor:delete-to-bol")
            .with_modes(&["vim:insert"])
            .with_category("edit")
            .with_description("Delete to start of line"),
        // ====================================================================
        // Insert operations
        // ====================================================================
        KeybindingRegistration::new("<CR>", "editor:insert-newline")
            .with_modes(&["vim:insert"])
            .with_category("edit")
            .with_description("Insert newline"),
        KeybindingRegistration::new("<Tab>", "editor:insert-tab")
            .with_modes(&["vim:insert"])
            .with_category("edit")
            .with_description("Insert tab/indent"),
        // ====================================================================
        // Completion
        // ====================================================================
        KeybindingRegistration::new("<C-n>", "editor:completion-next")
            .with_modes(&["vim:insert"])
            .with_category("completion")
            .with_description("Next completion"),
        KeybindingRegistration::new("<C-p>", "editor:completion-prev")
            .with_modes(&["vim:insert"])
            .with_category("completion")
            .with_description("Previous completion"),
        KeybindingRegistration::new("<C-Space>", "editor:completion-trigger")
            .with_modes(&["vim:insert"])
            .with_category("completion")
            .with_description("Trigger completion"),
    ]
}
