//! Visual mode keybindings.
//!
//! Reference: lib/core/src/bind/mod.rs (concept-extraction, not migration)

use {
    reovim_kernel::api::v1::KeybindingRegistration, reovim_module_editor::ids as editor,
    reovim_module_motions::ids as motions,
};

use crate::ids as vim;

/// Visual mode keybindings.
pub fn bindings() -> Vec<KeybindingRegistration> {
    vec![
        // ====================================================================
        // Exit visual mode
        // ====================================================================
        KeybindingRegistration::new("<Esc>", vim::EXIT_VISUAL)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("mode")
            .with_description("Exit visual mode"),
        KeybindingRegistration::new("<C-c>", vim::EXIT_VISUAL)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("mode")
            .with_description("Exit visual mode"),
        // ====================================================================
        // Movement (extends selection)
        // ====================================================================
        KeybindingRegistration::new("h", editor::CURSOR_LEFT)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("motion")
            .with_description("Extend selection left"),
        KeybindingRegistration::new("j", editor::CURSOR_DOWN)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("motion")
            .with_description("Extend selection down"),
        KeybindingRegistration::new("k", editor::CURSOR_UP)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("motion")
            .with_description("Extend selection up"),
        KeybindingRegistration::new("l", editor::CURSOR_RIGHT)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("motion")
            .with_description("Extend selection right"),
        KeybindingRegistration::new("w", motions::WORD_FORWARD)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("motion")
            .with_description("Extend to next word"),
        KeybindingRegistration::new("b", motions::WORD_BACKWARD)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("motion")
            .with_description("Extend to previous word"),
        KeybindingRegistration::new("e", motions::WORD_END)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("motion")
            .with_description("Extend to end of word"),
        KeybindingRegistration::new("0", motions::LINE_START)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("motion")
            .with_description("Extend to start of line"),
        KeybindingRegistration::new("$", motions::LINE_END)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("motion")
            .with_description("Extend to end of line"),
        KeybindingRegistration::new("gg", motions::DOCUMENT_START)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("motion")
            .with_description("Extend to start of document"),
        KeybindingRegistration::new("G", motions::DOCUMENT_END)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("motion")
            .with_description("Extend to end of document"),
        // ====================================================================
        // Selection operations
        // ====================================================================
        KeybindingRegistration::new("o", vim::VISUAL_SWAP_ANCHOR)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("selection")
            .with_description("Swap cursor and anchor"),
        KeybindingRegistration::new("O", vim::VISUAL_SWAP_CORNER)
            .with_modes(&["vim:visual-block"])
            .with_category("selection")
            .with_description("Swap cursor to opposite corner"),
        // ====================================================================
        // Mode switching within visual
        // ====================================================================
        KeybindingRegistration::new("v", vim::TOGGLE_VISUAL_CHAR)
            .with_modes(&["vim:visual-line", "vim:visual-block"])
            .with_category("mode")
            .with_description("Switch to character-wise visual"),
        KeybindingRegistration::new("V", vim::TOGGLE_VISUAL_LINE)
            .with_modes(&["vim:visual", "vim:visual-block"])
            .with_category("mode")
            .with_description("Switch to line-wise visual"),
        KeybindingRegistration::new("<C-v>", vim::TOGGLE_VISUAL_BLOCK)
            .with_modes(&["vim:visual", "vim:visual-line"])
            .with_category("mode")
            .with_description("Switch to block-wise visual"),
        // ====================================================================
        // Operators on selection
        // ====================================================================
        KeybindingRegistration::new("d", vim::DELETE_SELECTION)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("operator")
            .with_description("Delete selection"),
        KeybindingRegistration::new("y", vim::YANK_SELECTION)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("operator")
            .with_description("Yank selection"),
        KeybindingRegistration::new("c", vim::CHANGE_SELECTION)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("operator")
            .with_description("Change selection"),
        KeybindingRegistration::new("x", vim::DELETE_SELECTION)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("operator")
            .with_description("Delete selection (alias)"),
        KeybindingRegistration::new("<gt>", vim::INDENT_SELECTION)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("operator")
            .with_description("Indent selection"),
        KeybindingRegistration::new("<lt>", vim::DEDENT_SELECTION)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("operator")
            .with_description("Dedent selection"),
        KeybindingRegistration::new("~", vim::TOGGLE_CASE_SELECTION)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("operator")
            .with_description("Toggle case of selection"),
        KeybindingRegistration::new("u", vim::LOWERCASE_SELECTION)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("operator")
            .with_description("Lowercase selection"),
        KeybindingRegistration::new("U", vim::UPPERCASE_SELECTION)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("operator")
            .with_description("Uppercase selection"),
        // ====================================================================
        // Join
        // ====================================================================
        KeybindingRegistration::new("J", vim::JOIN_SELECTION)
            .with_modes(&["vim:visual", "vim:visual-line"])
            .with_category("edit")
            .with_description("Join selected lines"),
        // ====================================================================
        // Enter command mode with selection
        // ====================================================================
        KeybindingRegistration::new(":", vim::COMMAND_WITH_SELECTION)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("mode")
            .with_description("Enter command mode with selection range"),
        // ====================================================================
        // Blocked keys (explicit no-op) - Issue #145
        // ====================================================================
        KeybindingRegistration::new("i", vim::VISUAL_NOOP)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("blocked")
            .with_description("No-op (insert key blocked in visual mode)"),
        KeybindingRegistration::new("a", vim::VISUAL_NOOP)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("blocked")
            .with_description("No-op (append key blocked in visual mode)"),
        // ====================================================================
        // Visual insert commands (I/A) - Issue #145
        // ====================================================================
        KeybindingRegistration::new("I", vim::VISUAL_INSERT_START)
            .with_modes(&["vim:visual", "vim:visual-line"])
            .with_category("mode")
            .with_description("Exit visual, move to line start, enter insert"),
        KeybindingRegistration::new("A", vim::VISUAL_INSERT_END)
            .with_modes(&["vim:visual", "vim:visual-line"])
            .with_category("mode")
            .with_description("Exit visual, move to line end, enter insert"),
        // Block mode I/A - Issue #146
        KeybindingRegistration::new("I", vim::BLOCK_INSERT_START)
            .with_modes(&["vim:visual-block"])
            .with_category("mode")
            .with_description("Insert at block left column on all lines"),
        KeybindingRegistration::new("A", vim::BLOCK_INSERT_END)
            .with_modes(&["vim:visual-block"])
            .with_category("mode")
            .with_description("Append at block right column on all lines"),
    ]
}
