//! Visual mode keybindings.
//!
//! Reference: lib/core/src/bind/mod.rs (concept-extraction, not migration)

use {
    reovim_kernel::api::v1::KeybindingRegistration, reovim_module_editor::ids as editor,
    reovim_module_motions::ids as motions, reovim_module_textobjects::ids as textobjects,
};

use crate::ids as vim;

/// Visual mode keybindings.
#[must_use]
#[allow(clippy::too_many_lines)]
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
        // Screen position motions (#669)
        KeybindingRegistration::new("H", motions::SCREEN_HIGH)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("motion")
            .with_description("Extend to top of screen"),
        KeybindingRegistration::new("M", motions::SCREEN_MIDDLE)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("motion")
            .with_description("Extend to middle of screen"),
        KeybindingRegistration::new("L", motions::SCREEN_LOW)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("motion")
            .with_description("Extend to bottom of screen"),
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
        // Text Objects (Epic #465 - Phase 3.4)
        // ====================================================================
        // Inner text objects
        KeybindingRegistration::new("iw", textobjects::INNER_WORD)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select inner word"),
        KeybindingRegistration::new("iW", textobjects::INNER_WORD_BIG)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select inner WORD"),
        KeybindingRegistration::new("i\"", textobjects::INNER_DOUBLE_QUOTE)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select inner double-quoted string"),
        KeybindingRegistration::new("i'", textobjects::INNER_SINGLE_QUOTE)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select inner single-quoted string"),
        KeybindingRegistration::new("i(", textobjects::INNER_PAREN)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select inner parentheses"),
        KeybindingRegistration::new("i)", textobjects::INNER_PAREN)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select inner parentheses"),
        KeybindingRegistration::new("ib", textobjects::INNER_PAREN)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select inner parentheses"),
        KeybindingRegistration::new("i[", textobjects::INNER_BRACKET)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select inner brackets"),
        KeybindingRegistration::new("i]", textobjects::INNER_BRACKET)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select inner brackets"),
        KeybindingRegistration::new("i{", textobjects::INNER_BRACE)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select inner braces"),
        KeybindingRegistration::new("i}", textobjects::INNER_BRACE)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select inner braces"),
        KeybindingRegistration::new("iB", textobjects::INNER_BRACE)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select inner braces"),
        KeybindingRegistration::new("i<lt>", textobjects::INNER_ANGLE)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select inner angle brackets"),
        KeybindingRegistration::new("i>", textobjects::INNER_ANGLE)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select inner angle brackets"),
        // Around text objects
        KeybindingRegistration::new("aw", textobjects::AROUND_WORD)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select around word"),
        KeybindingRegistration::new("aW", textobjects::AROUND_WORD_BIG)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select around WORD"),
        KeybindingRegistration::new("a\"", textobjects::AROUND_DOUBLE_QUOTE)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select around double-quoted string"),
        KeybindingRegistration::new("a'", textobjects::AROUND_SINGLE_QUOTE)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select around single-quoted string"),
        KeybindingRegistration::new("a(", textobjects::AROUND_PAREN)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select around parentheses"),
        KeybindingRegistration::new("a)", textobjects::AROUND_PAREN)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select around parentheses"),
        KeybindingRegistration::new("ab", textobjects::AROUND_PAREN)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select around parentheses"),
        KeybindingRegistration::new("a[", textobjects::AROUND_BRACKET)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select around brackets"),
        KeybindingRegistration::new("a]", textobjects::AROUND_BRACKET)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select around brackets"),
        KeybindingRegistration::new("a{", textobjects::AROUND_BRACE)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select around braces"),
        KeybindingRegistration::new("a}", textobjects::AROUND_BRACE)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select around braces"),
        KeybindingRegistration::new("aB", textobjects::AROUND_BRACE)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select around braces"),
        KeybindingRegistration::new("a<lt>", textobjects::AROUND_ANGLE)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select around angle brackets"),
        KeybindingRegistration::new("a>", textobjects::AROUND_ANGLE)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select around angle brackets"),
        // Semantic text objects (treesitter-based)
        KeybindingRegistration::new("if", textobjects::INNER_FUNCTION)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select inner function"),
        KeybindingRegistration::new("af", textobjects::AROUND_FUNCTION)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select around function"),
        KeybindingRegistration::new("ic", textobjects::INNER_CLASS)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select inner class"),
        KeybindingRegistration::new("ac", textobjects::AROUND_CLASS)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select around class"),
        KeybindingRegistration::new("ia", textobjects::INNER_ARGUMENT)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select inner argument"),
        KeybindingRegistration::new("aa", textobjects::AROUND_ARGUMENT)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select around argument"),
        KeybindingRegistration::new("io", textobjects::INNER_CONDITIONAL)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select inner conditional"),
        KeybindingRegistration::new("ao", textobjects::AROUND_CONDITIONAL)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select around conditional"),
        KeybindingRegistration::new("il", textobjects::INNER_LOOP)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select inner loop"),
        KeybindingRegistration::new("al", textobjects::AROUND_LOOP)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select around loop"),
        KeybindingRegistration::new("i/", textobjects::INNER_COMMENT)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select inner comment"),
        KeybindingRegistration::new("a/", textobjects::AROUND_COMMENT)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select around comment"),
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
