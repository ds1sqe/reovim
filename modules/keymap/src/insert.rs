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
            .with_modes(&["insert"])
            .with_category("mode")
            .with_description("Exit insert mode"),
        KeybindingRegistration::new("<C-c>", "exit-insert")
            .with_modes(&["insert"])
            .with_category("mode")
            .with_description("Exit insert mode (Ctrl-C)"),
        KeybindingRegistration::new("<C-[>", "exit-insert")
            .with_modes(&["insert"])
            .with_category("mode")
            .with_description("Exit insert mode (Ctrl-[)"),
        // ====================================================================
        // Navigation in insert mode
        // ====================================================================
        KeybindingRegistration::new("<Left>", "cursor-left")
            .with_modes(&["insert"])
            .with_category("motion")
            .with_description("Move cursor left"),
        KeybindingRegistration::new("<Right>", "cursor-right")
            .with_modes(&["insert"])
            .with_category("motion")
            .with_description("Move cursor right"),
        KeybindingRegistration::new("<Up>", "cursor-up")
            .with_modes(&["insert"])
            .with_category("motion")
            .with_description("Move cursor up"),
        KeybindingRegistration::new("<Down>", "cursor-down")
            .with_modes(&["insert"])
            .with_category("motion")
            .with_description("Move cursor down"),
        KeybindingRegistration::new("<Home>", "line-start")
            .with_modes(&["insert"])
            .with_category("motion")
            .with_description("Move to start of line"),
        KeybindingRegistration::new("<End>", "line-end")
            .with_modes(&["insert"])
            .with_category("motion")
            .with_description("Move to end of line"),
        // ====================================================================
        // Deletion in insert mode
        // ====================================================================
        KeybindingRegistration::new("<BS>", "delete-char-before")
            .with_modes(&["insert"])
            .with_category("edit")
            .with_description("Delete character before cursor"),
        KeybindingRegistration::new("<Del>", "delete-char")
            .with_modes(&["insert"])
            .with_category("edit")
            .with_description("Delete character under cursor"),
        KeybindingRegistration::new("<C-h>", "delete-char-before")
            .with_modes(&["insert"])
            .with_category("edit")
            .with_description("Delete character before cursor"),
        KeybindingRegistration::new("<C-w>", "delete-word-before")
            .with_modes(&["insert"])
            .with_category("edit")
            .with_description("Delete word before cursor"),
        KeybindingRegistration::new("<C-u>", "delete-to-bol")
            .with_modes(&["insert"])
            .with_category("edit")
            .with_description("Delete to start of line"),
        // ====================================================================
        // Insert operations
        // ====================================================================
        KeybindingRegistration::new("<CR>", "insert-newline")
            .with_modes(&["insert"])
            .with_category("edit")
            .with_description("Insert newline"),
        KeybindingRegistration::new("<Tab>", "insert-tab")
            .with_modes(&["insert"])
            .with_category("edit")
            .with_description("Insert tab/indent"),
        // ====================================================================
        // Completion
        // ====================================================================
        KeybindingRegistration::new("<C-n>", "completion-next")
            .with_modes(&["insert"])
            .with_category("completion")
            .with_description("Next completion"),
        KeybindingRegistration::new("<C-p>", "completion-prev")
            .with_modes(&["insert"])
            .with_category("completion")
            .with_description("Previous completion"),
        KeybindingRegistration::new("<C-Space>", "completion-trigger")
            .with_modes(&["insert"])
            .with_category("completion")
            .with_description("Trigger completion"),
    ]
}
