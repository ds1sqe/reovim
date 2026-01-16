//! Visual mode keybindings.
//!
//! Reference: lib/core/src/bind/mod.rs (concept-extraction, not migration)

use reovim_kernel::api::v1::KeybindingRegistration;

/// Visual mode keybindings.
pub fn bindings() -> Vec<KeybindingRegistration> {
    vec![
        // ====================================================================
        // Exit visual mode
        // ====================================================================
        KeybindingRegistration::new("<Esc>", "exit-visual")
            .with_modes(&["visual", "visual-line", "visual-block"])
            .with_category("mode")
            .with_description("Exit visual mode"),
        KeybindingRegistration::new("<C-c>", "exit-visual")
            .with_modes(&["visual", "visual-line", "visual-block"])
            .with_category("mode")
            .with_description("Exit visual mode"),
        // ====================================================================
        // Movement (extends selection)
        // ====================================================================
        KeybindingRegistration::new("h", "cursor-left")
            .with_modes(&["visual", "visual-line", "visual-block"])
            .with_category("motion")
            .with_description("Extend selection left"),
        KeybindingRegistration::new("j", "cursor-down")
            .with_modes(&["visual", "visual-line", "visual-block"])
            .with_category("motion")
            .with_description("Extend selection down"),
        KeybindingRegistration::new("k", "cursor-up")
            .with_modes(&["visual", "visual-line", "visual-block"])
            .with_category("motion")
            .with_description("Extend selection up"),
        KeybindingRegistration::new("l", "cursor-right")
            .with_modes(&["visual", "visual-line", "visual-block"])
            .with_category("motion")
            .with_description("Extend selection right"),
        KeybindingRegistration::new("w", "word-forward")
            .with_modes(&["visual", "visual-line", "visual-block"])
            .with_category("motion")
            .with_description("Extend to next word"),
        KeybindingRegistration::new("b", "word-backward")
            .with_modes(&["visual", "visual-line", "visual-block"])
            .with_category("motion")
            .with_description("Extend to previous word"),
        KeybindingRegistration::new("e", "word-end")
            .with_modes(&["visual", "visual-line", "visual-block"])
            .with_category("motion")
            .with_description("Extend to end of word"),
        KeybindingRegistration::new("0", "line-start")
            .with_modes(&["visual", "visual-line", "visual-block"])
            .with_category("motion")
            .with_description("Extend to start of line"),
        KeybindingRegistration::new("$", "line-end")
            .with_modes(&["visual", "visual-line", "visual-block"])
            .with_category("motion")
            .with_description("Extend to end of line"),
        KeybindingRegistration::new("gg", "document-start")
            .with_modes(&["visual", "visual-line", "visual-block"])
            .with_category("motion")
            .with_description("Extend to start of document"),
        KeybindingRegistration::new("G", "document-end")
            .with_modes(&["visual", "visual-line", "visual-block"])
            .with_category("motion")
            .with_description("Extend to end of document"),
        // ====================================================================
        // Selection operations
        // ====================================================================
        KeybindingRegistration::new("o", "visual-swap-anchor")
            .with_modes(&["visual", "visual-line", "visual-block"])
            .with_category("selection")
            .with_description("Swap cursor and anchor"),
        KeybindingRegistration::new("O", "visual-swap-corner")
            .with_modes(&["visual-block"])
            .with_category("selection")
            .with_description("Swap cursor to opposite corner"),
        // ====================================================================
        // Mode switching within visual
        // ====================================================================
        KeybindingRegistration::new("v", "toggle-visual-char")
            .with_modes(&["visual-line", "visual-block"])
            .with_category("mode")
            .with_description("Switch to character-wise visual"),
        KeybindingRegistration::new("V", "toggle-visual-line")
            .with_modes(&["visual", "visual-block"])
            .with_category("mode")
            .with_description("Switch to line-wise visual"),
        KeybindingRegistration::new("<C-v>", "toggle-visual-block")
            .with_modes(&["visual", "visual-line"])
            .with_category("mode")
            .with_description("Switch to block-wise visual"),
        // ====================================================================
        // Operators on selection
        // ====================================================================
        KeybindingRegistration::new("d", "delete-selection")
            .with_modes(&["visual", "visual-line", "visual-block"])
            .with_category("operator")
            .with_description("Delete selection"),
        KeybindingRegistration::new("y", "yank-selection")
            .with_modes(&["visual", "visual-line", "visual-block"])
            .with_category("operator")
            .with_description("Yank selection"),
        KeybindingRegistration::new("c", "change-selection")
            .with_modes(&["visual", "visual-line", "visual-block"])
            .with_category("operator")
            .with_description("Change selection"),
        KeybindingRegistration::new("x", "delete-selection")
            .with_modes(&["visual", "visual-line", "visual-block"])
            .with_category("operator")
            .with_description("Delete selection (alias)"),
        KeybindingRegistration::new(">", "indent-selection")
            .with_modes(&["visual", "visual-line", "visual-block"])
            .with_category("operator")
            .with_description("Indent selection"),
        KeybindingRegistration::new("<", "dedent-selection")
            .with_modes(&["visual", "visual-line", "visual-block"])
            .with_category("operator")
            .with_description("Dedent selection"),
        KeybindingRegistration::new("~", "toggle-case-selection")
            .with_modes(&["visual", "visual-line", "visual-block"])
            .with_category("operator")
            .with_description("Toggle case of selection"),
        KeybindingRegistration::new("u", "lowercase-selection")
            .with_modes(&["visual", "visual-line", "visual-block"])
            .with_category("operator")
            .with_description("Lowercase selection"),
        KeybindingRegistration::new("U", "uppercase-selection")
            .with_modes(&["visual", "visual-line", "visual-block"])
            .with_category("operator")
            .with_description("Uppercase selection"),
        // ====================================================================
        // Join
        // ====================================================================
        KeybindingRegistration::new("J", "join-selection")
            .with_modes(&["visual", "visual-line"])
            .with_category("edit")
            .with_description("Join selected lines"),
        // ====================================================================
        // Enter command mode with selection
        // ====================================================================
        KeybindingRegistration::new(":", "command-with-selection")
            .with_modes(&["visual", "visual-line", "visual-block"])
            .with_category("mode")
            .with_description("Enter command mode with selection range"),
        // ====================================================================
        // Blocked keys (explicit no-op) - Issue #145
        // ====================================================================
        KeybindingRegistration::new("i", "visual-noop")
            .with_modes(&["visual", "visual-line", "visual-block"])
            .with_category("blocked")
            .with_description("No-op (insert key blocked in visual mode)"),
        KeybindingRegistration::new("a", "visual-noop")
            .with_modes(&["visual", "visual-line", "visual-block"])
            .with_category("blocked")
            .with_description("No-op (append key blocked in visual mode)"),
        // ====================================================================
        // Visual insert commands (I/A) - Issue #145
        // ====================================================================
        KeybindingRegistration::new("I", "visual-insert-start")
            .with_modes(&["visual", "visual-line"])
            .with_category("mode")
            .with_description("Exit visual, move to line start, enter insert"),
        KeybindingRegistration::new("A", "visual-insert-end")
            .with_modes(&["visual", "visual-line"])
            .with_category("mode")
            .with_description("Exit visual, move to line end, enter insert"),
        // Block mode I/A - Issue #146
        KeybindingRegistration::new("I", "block-insert-start")
            .with_modes(&["visual-block"])
            .with_category("mode")
            .with_description("Insert at block left column on all lines"),
        KeybindingRegistration::new("A", "block-insert-end")
            .with_modes(&["visual-block"])
            .with_category("mode")
            .with_description("Append at block right column on all lines"),
    ]
}
