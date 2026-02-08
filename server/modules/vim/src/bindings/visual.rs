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
        KeybindingRegistration::new("i<", textobjects::INNER_ANGLE)
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
        KeybindingRegistration::new("a<", textobjects::AROUND_ANGLE)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select around angle brackets"),
        KeybindingRegistration::new("a>", textobjects::AROUND_ANGLE)
            .with_modes(&["vim:visual", "vim:visual-line", "vim:visual-block"])
            .with_category("textobject")
            .with_description("Select around angle brackets"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bindings_not_empty() {
        let b = bindings();
        assert!(!b.is_empty());
    }

    #[test]
    fn test_all_bindings_in_visual_modes() {
        for binding in bindings() {
            let in_visual = binding.modes.contains(&"vim:visual")
                || binding.modes.contains(&"vim:visual-line")
                || binding.modes.contains(&"vim:visual-block");
            assert!(
                in_visual,
                "Binding '{}' should be in a visual mode, got {:?}",
                binding.keys, binding.modes
            );
        }
    }

    #[test]
    fn test_exit_keys_exist() {
        let b = bindings();
        let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
        assert!(keys.contains(&"<Esc>"), "Missing '<Esc>' binding");
        assert!(keys.contains(&"<C-c>"), "Missing '<C-c>' binding");
    }

    #[test]
    fn test_movement_keys_exist() {
        let b = bindings();
        let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
        assert!(keys.contains(&"h"), "Missing 'h' binding");
        assert!(keys.contains(&"j"), "Missing 'j' binding");
        assert!(keys.contains(&"k"), "Missing 'k' binding");
        assert!(keys.contains(&"l"), "Missing 'l' binding");
        assert!(keys.contains(&"w"), "Missing 'w' binding");
        assert!(keys.contains(&"b"), "Missing 'b' binding");
        assert!(keys.contains(&"e"), "Missing 'e' binding");
    }

    #[test]
    fn test_line_motions_exist() {
        let b = bindings();
        let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
        assert!(keys.contains(&"0"), "Missing '0' binding");
        assert!(keys.contains(&"$"), "Missing '$' binding");
    }

    #[test]
    fn test_document_motions_exist() {
        let b = bindings();
        let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
        assert!(keys.contains(&"gg"), "Missing 'gg' binding");
        assert!(keys.contains(&"G"), "Missing 'G' binding");
    }

    #[test]
    fn test_selection_operation_keys_exist() {
        let b = bindings();
        let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
        assert!(keys.contains(&"o"), "Missing 'o' (swap anchor) binding");
        assert!(keys.contains(&"O"), "Missing 'O' (swap corner) binding");
    }

    #[test]
    fn test_mode_toggle_keys_exist() {
        let b = bindings();
        let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
        assert!(keys.contains(&"v"), "Missing 'v' binding");
        assert!(keys.contains(&"V"), "Missing 'V' binding");
        assert!(keys.contains(&"<C-v>"), "Missing '<C-v>' binding");
    }

    #[test]
    fn test_operator_keys_exist() {
        let b = bindings();
        let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
        assert!(keys.contains(&"d"), "Missing 'd' binding");
        assert!(keys.contains(&"y"), "Missing 'y' binding");
        assert!(keys.contains(&"c"), "Missing 'c' binding");
        assert!(keys.contains(&"x"), "Missing 'x' binding");
        assert!(keys.contains(&"<gt>"), "Missing '<gt>' binding");
        assert!(keys.contains(&"<lt>"), "Missing '<lt>' binding");
    }

    #[test]
    fn test_case_operator_keys_exist() {
        let b = bindings();
        let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
        assert!(keys.contains(&"~"), "Missing '~' binding");
        assert!(keys.contains(&"u"), "Missing 'u' binding");
        assert!(keys.contains(&"U"), "Missing 'U' binding");
    }

    #[test]
    fn test_join_key_exists() {
        let b = bindings();
        assert!(b.iter().any(|kb| kb.keys == "J"), "Missing 'J' binding");
    }

    #[test]
    fn test_command_mode_key_exists() {
        let b = bindings();
        assert!(b.iter().any(|kb| kb.keys == ":"), "Missing ':' binding");
    }

    #[test]
    fn test_inner_text_object_keys_exist() {
        let b = bindings();
        let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
        assert!(keys.contains(&"iw"), "Missing 'iw' binding");
        assert!(keys.contains(&"iW"), "Missing 'iW' binding");
        assert!(keys.contains(&"i\""), "Missing 'i\"' binding");
        assert!(keys.contains(&"i'"), "Missing \"i'\" binding");
        assert!(keys.contains(&"i("), "Missing 'i(' binding");
        assert!(keys.contains(&"i)"), "Missing 'i)' binding");
        assert!(keys.contains(&"ib"), "Missing 'ib' binding");
        assert!(keys.contains(&"i["), "Missing 'i[' binding");
        assert!(keys.contains(&"i]"), "Missing 'i]' binding");
        assert!(keys.contains(&"i{"), "Missing 'i{{' binding");
        assert!(keys.contains(&"i}"), "Missing 'i}}' binding");
        assert!(keys.contains(&"iB"), "Missing 'iB' binding");
        assert!(keys.contains(&"i<"), "Missing 'i<' binding");
        assert!(keys.contains(&"i>"), "Missing 'i>' binding");
    }

    #[test]
    fn test_around_text_object_keys_exist() {
        let b = bindings();
        let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
        assert!(keys.contains(&"aw"), "Missing 'aw' binding");
        assert!(keys.contains(&"aW"), "Missing 'aW' binding");
        assert!(keys.contains(&"a\""), "Missing 'a\"' binding");
        assert!(keys.contains(&"a'"), "Missing \"a'\" binding");
        assert!(keys.contains(&"a("), "Missing 'a(' binding");
        assert!(keys.contains(&"a)"), "Missing 'a)' binding");
        assert!(keys.contains(&"ab"), "Missing 'ab' binding");
        assert!(keys.contains(&"a["), "Missing 'a[' binding");
        assert!(keys.contains(&"a]"), "Missing 'a]' binding");
        assert!(keys.contains(&"a{"), "Missing 'a{{' binding");
        assert!(keys.contains(&"a}"), "Missing 'a}}' binding");
        assert!(keys.contains(&"aB"), "Missing 'aB' binding");
        assert!(keys.contains(&"a<"), "Missing 'a<' binding");
        assert!(keys.contains(&"a>"), "Missing 'a>' binding");
    }

    #[test]
    fn test_visual_insert_keys_exist() {
        let b = bindings();
        // I and A should be present (multiple times for different modes)
        assert!(b.iter().any(|kb| kb.keys == "I"), "Missing 'I' binding for visual insert");

        assert!(b.iter().any(|kb| kb.keys == "A"), "Missing 'A' binding for visual insert");
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
