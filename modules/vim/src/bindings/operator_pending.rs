//! Operator-pending mode keybindings.
//!
//! Reference: lib/core/src/bind/mod.rs (concept-extraction, not migration)
//!
//! Operator-pending mode is entered after pressing an operator (d, y, c)
//! and waits for a motion or text object to complete the operation.

use {
    reovim_kernel::api::v1::KeybindingRegistration, reovim_module_editor::ids as editor,
    reovim_module_motions::ids as motions, reovim_module_textobjects::ids as textobjects,
};

use crate::ids as vim;

/// Operator-pending mode keybindings.
pub fn bindings() -> Vec<KeybindingRegistration> {
    vec![
        // ====================================================================
        // Exit operator-pending mode
        // ====================================================================
        KeybindingRegistration::new("<Esc>", vim::EXIT_OPERATOR_PENDING)
            .with_modes(&["vim:operator-pending"])
            .with_category("mode")
            .with_description("Cancel operator"),
        KeybindingRegistration::new("<C-c>", vim::EXIT_OPERATOR_PENDING)
            .with_modes(&["vim:operator-pending"])
            .with_category("mode")
            .with_description("Cancel operator"),
        // ====================================================================
        // Operator doubling (dd, yy, cc)
        // ====================================================================
        // When an operator key is pressed again in operator-pending mode,
        // it operates on the current line (whole-line motion).
        KeybindingRegistration::new("d", motions::WHOLE_LINE)
            .with_modes(&["vim:operator-pending"])
            .with_category("motion")
            .with_description("Whole line (for dd)"),
        KeybindingRegistration::new("y", motions::WHOLE_LINE)
            .with_modes(&["vim:operator-pending"])
            .with_category("motion")
            .with_description("Whole line (for yy)"),
        KeybindingRegistration::new("c", motions::WHOLE_LINE)
            .with_modes(&["vim:operator-pending"])
            .with_category("motion")
            .with_description("Whole line (for cc)"),
        // ====================================================================
        // Motions (complete the operator)
        // ====================================================================
        KeybindingRegistration::new("h", editor::CURSOR_LEFT)
            .with_modes(&["vim:operator-pending"])
            .with_category("motion")
            .with_description("Left motion"),
        KeybindingRegistration::new("j", editor::CURSOR_DOWN)
            .with_modes(&["vim:operator-pending"])
            .with_category("motion")
            .with_description("Down motion"),
        KeybindingRegistration::new("k", editor::CURSOR_UP)
            .with_modes(&["vim:operator-pending"])
            .with_category("motion")
            .with_description("Up motion"),
        KeybindingRegistration::new("l", editor::CURSOR_RIGHT)
            .with_modes(&["vim:operator-pending"])
            .with_category("motion")
            .with_description("Right motion"),
        KeybindingRegistration::new("w", motions::WORD_FORWARD)
            .with_modes(&["vim:operator-pending"])
            .with_category("motion")
            .with_description("Word forward motion"),
        KeybindingRegistration::new("b", motions::WORD_BACKWARD)
            .with_modes(&["vim:operator-pending"])
            .with_category("motion")
            .with_description("Word backward motion"),
        KeybindingRegistration::new("e", motions::WORD_END)
            .with_modes(&["vim:operator-pending"])
            .with_category("motion")
            .with_description("Word end motion"),
        KeybindingRegistration::new("W", motions::WORD_FORWARD_BIG)
            .with_modes(&["vim:operator-pending"])
            .with_category("motion")
            .with_description("WORD forward motion"),
        KeybindingRegistration::new("B", motions::WORD_BACKWARD_BIG)
            .with_modes(&["vim:operator-pending"])
            .with_category("motion")
            .with_description("WORD backward motion"),
        KeybindingRegistration::new("E", motions::WORD_END_BIG)
            .with_modes(&["vim:operator-pending"])
            .with_category("motion")
            .with_description("WORD end motion"),
        KeybindingRegistration::new("ge", motions::WORD_END_BACKWARD)
            .with_modes(&["vim:operator-pending"])
            .with_category("motion")
            .with_description("Word end backward motion"),
        KeybindingRegistration::new("gE", motions::WORD_END_BACKWARD_BIG)
            .with_modes(&["vim:operator-pending"])
            .with_category("motion")
            .with_description("WORD end backward motion"),
        KeybindingRegistration::new("0", motions::LINE_START)
            .with_modes(&["vim:operator-pending"])
            .with_category("motion")
            .with_description("Line start motion"),
        KeybindingRegistration::new("$", motions::LINE_END)
            .with_modes(&["vim:operator-pending"])
            .with_category("motion")
            .with_description("Line end motion"),
        KeybindingRegistration::new("^", motions::FIRST_NON_BLANK)
            .with_modes(&["vim:operator-pending"])
            .with_category("motion")
            .with_description("First non-blank motion"),
        KeybindingRegistration::new("gg", motions::DOCUMENT_START)
            .with_modes(&["vim:operator-pending"])
            .with_category("motion")
            .with_description("Document start motion"),
        KeybindingRegistration::new("G", motions::DOCUMENT_END)
            .with_modes(&["vim:operator-pending"])
            .with_category("motion")
            .with_description("Document end motion"),
        // ====================================================================
        // Text objects - inner
        // ====================================================================
        KeybindingRegistration::new("iw", textobjects::INNER_WORD)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Inner word"),
        KeybindingRegistration::new("iW", textobjects::INNER_WORD_BIG)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Inner WORD"),
        KeybindingRegistration::new("i\"", textobjects::INNER_DOUBLE_QUOTE)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Inner double quotes"),
        KeybindingRegistration::new("i'", textobjects::INNER_SINGLE_QUOTE)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Inner single quotes"),
        KeybindingRegistration::new("i`", textobjects::INNER_BACKTICK)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Inner backticks"),
        KeybindingRegistration::new("i(", textobjects::INNER_PAREN)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Inner parentheses"),
        KeybindingRegistration::new("i)", textobjects::INNER_PAREN)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Inner parentheses"),
        KeybindingRegistration::new("ib", textobjects::INNER_PAREN)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Inner block (parentheses)"),
        KeybindingRegistration::new("i[", textobjects::INNER_BRACKET)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Inner brackets"),
        KeybindingRegistration::new("i]", textobjects::INNER_BRACKET)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Inner brackets"),
        KeybindingRegistration::new("i{", textobjects::INNER_BRACE)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Inner braces"),
        KeybindingRegistration::new("i}", textobjects::INNER_BRACE)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Inner braces"),
        KeybindingRegistration::new("iB", textobjects::INNER_BRACE)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Inner Block (braces)"),
        KeybindingRegistration::new("i<lt>", textobjects::INNER_ANGLE)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Inner angle brackets"),
        KeybindingRegistration::new("i<gt>", textobjects::INNER_ANGLE)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Inner angle brackets"),
        KeybindingRegistration::new("it", textobjects::INNER_TAG)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Inner tag"),
        KeybindingRegistration::new("is", textobjects::INNER_SENTENCE)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Inner sentence"),
        KeybindingRegistration::new("ip", textobjects::INNER_PARAGRAPH)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Inner paragraph"),
        // ====================================================================
        // Text objects - around
        // ====================================================================
        KeybindingRegistration::new("aw", textobjects::AROUND_WORD)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Around word"),
        KeybindingRegistration::new("aW", textobjects::AROUND_WORD_BIG)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Around WORD"),
        KeybindingRegistration::new("a\"", textobjects::AROUND_DOUBLE_QUOTE)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Around double quotes"),
        KeybindingRegistration::new("a'", textobjects::AROUND_SINGLE_QUOTE)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Around single quotes"),
        KeybindingRegistration::new("a`", textobjects::AROUND_BACKTICK)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Around backticks"),
        KeybindingRegistration::new("a(", textobjects::AROUND_PAREN)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Around parentheses"),
        KeybindingRegistration::new("a)", textobjects::AROUND_PAREN)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Around parentheses"),
        KeybindingRegistration::new("ab", textobjects::AROUND_PAREN)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Around block (parentheses)"),
        KeybindingRegistration::new("a[", textobjects::AROUND_BRACKET)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Around brackets"),
        KeybindingRegistration::new("a]", textobjects::AROUND_BRACKET)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Around brackets"),
        KeybindingRegistration::new("a{", textobjects::AROUND_BRACE)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Around braces"),
        KeybindingRegistration::new("a}", textobjects::AROUND_BRACE)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Around braces"),
        KeybindingRegistration::new("aB", textobjects::AROUND_BRACE)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Around Block (braces)"),
        KeybindingRegistration::new("a<lt>", textobjects::AROUND_ANGLE)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Around angle brackets"),
        KeybindingRegistration::new("a<gt>", textobjects::AROUND_ANGLE)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Around angle brackets"),
        KeybindingRegistration::new("at", textobjects::AROUND_TAG)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Around tag"),
        KeybindingRegistration::new("as", textobjects::AROUND_SENTENCE)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Around sentence"),
        KeybindingRegistration::new("ap", textobjects::AROUND_PARAGRAPH)
            .with_modes(&["vim:operator-pending"])
            .with_category("textobj")
            .with_description("Around paragraph"),
    ]
}
