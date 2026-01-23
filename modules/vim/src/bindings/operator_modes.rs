//! Keybindings for operator modes (delete, yank, change).
//!
//! This module provides keybindings for the dedicated operator modes introduced
//! in Epic #415. Each operator mode has the same motion/text-object bindings,
//! but scoped to its specific mode.
//!
//! # Architecture
//!
//! Instead of duplicating bindings across three files, we use helper functions
//! that generate bindings for a given mode. This ensures consistency and makes
//! it easy to add new motions.

use reovim_kernel::api::v1::KeybindingRegistration;

use {
    crate::ids as vim, reovim_module_editor::ids as editor, reovim_module_motions::ids as motions,
    reovim_module_textobjects::ids as textobjects,
};

// Mode string constants
const DELETE_MODE: &str = "vim:delete";
const YANK_MODE: &str = "vim:yank";
const CHANGE_MODE: &str = "vim:change";

// =============================================================================
// Delete Mode Keybindings
// =============================================================================

/// Delete mode keybindings.
///
/// These bindings are active when in `vim:delete` mode, waiting for a motion
/// or text object to complete the delete operation.
#[must_use]
pub fn delete_bindings() -> Vec<KeybindingRegistration> {
    let mut bindings = vec![
        // Exit operator mode
        KeybindingRegistration::new("<Esc>", vim::EXIT_OPERATOR_PENDING)
            .with_modes(&[DELETE_MODE])
            .with_category("mode")
            .with_description("Cancel delete"),
        KeybindingRegistration::new("<C-c>", vim::EXIT_OPERATOR_PENDING)
            .with_modes(&[DELETE_MODE])
            .with_category("mode")
            .with_description("Cancel delete"),
        // Line operator (dd)
        KeybindingRegistration::new("d", motions::WHOLE_LINE)
            .with_modes(&[DELETE_MODE])
            .with_category("motion")
            .with_description("Delete whole line (dd)"),
    ];

    bindings.extend(delete_motion_bindings());
    bindings.extend(delete_textobject_bindings());
    bindings
}

// =============================================================================
// Yank Mode Keybindings
// =============================================================================

/// Yank mode keybindings.
///
/// These bindings are active when in `vim:yank` mode, waiting for a motion
/// or text object to complete the yank operation.
#[must_use]
pub fn yank_bindings() -> Vec<KeybindingRegistration> {
    let mut bindings = vec![
        // Exit operator mode
        KeybindingRegistration::new("<Esc>", vim::EXIT_OPERATOR_PENDING)
            .with_modes(&[YANK_MODE])
            .with_category("mode")
            .with_description("Cancel yank"),
        KeybindingRegistration::new("<C-c>", vim::EXIT_OPERATOR_PENDING)
            .with_modes(&[YANK_MODE])
            .with_category("mode")
            .with_description("Cancel yank"),
        // Line operator (yy)
        KeybindingRegistration::new("y", motions::WHOLE_LINE)
            .with_modes(&[YANK_MODE])
            .with_category("motion")
            .with_description("Yank whole line (yy)"),
    ];

    bindings.extend(yank_motion_bindings());
    bindings.extend(yank_textobject_bindings());
    bindings
}

// =============================================================================
// Change Mode Keybindings
// =============================================================================

/// Change mode keybindings.
///
/// These bindings are active when in `vim:change` mode, waiting for a motion
/// or text object to complete the change operation.
#[must_use]
pub fn change_bindings() -> Vec<KeybindingRegistration> {
    let mut bindings = vec![
        // Exit operator mode
        KeybindingRegistration::new("<Esc>", vim::EXIT_OPERATOR_PENDING)
            .with_modes(&[CHANGE_MODE])
            .with_category("mode")
            .with_description("Cancel change"),
        KeybindingRegistration::new("<C-c>", vim::EXIT_OPERATOR_PENDING)
            .with_modes(&[CHANGE_MODE])
            .with_category("mode")
            .with_description("Cancel change"),
        // Line operator (cc)
        KeybindingRegistration::new("c", motions::WHOLE_LINE)
            .with_modes(&[CHANGE_MODE])
            .with_category("motion")
            .with_description("Change whole line (cc)"),
    ];

    bindings.extend(change_motion_bindings());
    bindings.extend(change_textobject_bindings());
    bindings
}

// =============================================================================
// Delete Mode Motion and Text Object Bindings
// =============================================================================

fn delete_motion_bindings() -> Vec<KeybindingRegistration> {
    vec![
        // Basic cursor motions
        KeybindingRegistration::new("h", editor::CURSOR_LEFT)
            .with_modes(&[DELETE_MODE])
            .with_category("motion")
            .with_description("Left motion"),
        KeybindingRegistration::new("j", editor::CURSOR_DOWN)
            .with_modes(&[DELETE_MODE])
            .with_category("motion")
            .with_description("Down motion"),
        KeybindingRegistration::new("k", editor::CURSOR_UP)
            .with_modes(&[DELETE_MODE])
            .with_category("motion")
            .with_description("Up motion"),
        KeybindingRegistration::new("l", editor::CURSOR_RIGHT)
            .with_modes(&[DELETE_MODE])
            .with_category("motion")
            .with_description("Right motion"),
        // Word motions
        KeybindingRegistration::new("w", motions::WORD_FORWARD)
            .with_modes(&[DELETE_MODE])
            .with_category("motion")
            .with_description("Word forward motion"),
        KeybindingRegistration::new("b", motions::WORD_BACKWARD)
            .with_modes(&[DELETE_MODE])
            .with_category("motion")
            .with_description("Word backward motion"),
        KeybindingRegistration::new("e", motions::WORD_END)
            .with_modes(&[DELETE_MODE])
            .with_category("motion")
            .with_description("Word end motion"),
        KeybindingRegistration::new("W", motions::WORD_FORWARD_BIG)
            .with_modes(&[DELETE_MODE])
            .with_category("motion")
            .with_description("WORD forward motion"),
        KeybindingRegistration::new("B", motions::WORD_BACKWARD_BIG)
            .with_modes(&[DELETE_MODE])
            .with_category("motion")
            .with_description("WORD backward motion"),
        KeybindingRegistration::new("E", motions::WORD_END_BIG)
            .with_modes(&[DELETE_MODE])
            .with_category("motion")
            .with_description("WORD end motion"),
        KeybindingRegistration::new("ge", motions::WORD_END_BACKWARD)
            .with_modes(&[DELETE_MODE])
            .with_category("motion")
            .with_description("Word end backward motion"),
        KeybindingRegistration::new("gE", motions::WORD_END_BACKWARD_BIG)
            .with_modes(&[DELETE_MODE])
            .with_category("motion")
            .with_description("WORD end backward motion"),
        // Line motions
        KeybindingRegistration::new("0", motions::LINE_START)
            .with_modes(&[DELETE_MODE])
            .with_category("motion")
            .with_description("Line start motion"),
        KeybindingRegistration::new("$", motions::LINE_END)
            .with_modes(&[DELETE_MODE])
            .with_category("motion")
            .with_description("Line end motion"),
        KeybindingRegistration::new("^", motions::FIRST_NON_BLANK)
            .with_modes(&[DELETE_MODE])
            .with_category("motion")
            .with_description("First non-blank motion"),
        // Document motions
        KeybindingRegistration::new("gg", motions::DOCUMENT_START)
            .with_modes(&[DELETE_MODE])
            .with_category("motion")
            .with_description("Document start motion"),
        KeybindingRegistration::new("G", motions::DOCUMENT_END)
            .with_modes(&[DELETE_MODE])
            .with_category("motion")
            .with_description("Document end motion"),
    ]
}

fn delete_textobject_bindings() -> Vec<KeybindingRegistration> {
    vec![
        // Inner text objects
        KeybindingRegistration::new("iw", textobjects::INNER_WORD)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Inner word"),
        KeybindingRegistration::new("iW", textobjects::INNER_WORD_BIG)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Inner WORD"),
        KeybindingRegistration::new("i\"", textobjects::INNER_DOUBLE_QUOTE)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Inner double quotes"),
        KeybindingRegistration::new("i'", textobjects::INNER_SINGLE_QUOTE)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Inner single quotes"),
        KeybindingRegistration::new("i`", textobjects::INNER_BACKTICK)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Inner backticks"),
        KeybindingRegistration::new("i(", textobjects::INNER_PAREN)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Inner parentheses"),
        KeybindingRegistration::new("i)", textobjects::INNER_PAREN)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Inner parentheses"),
        KeybindingRegistration::new("ib", textobjects::INNER_PAREN)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Inner block (parentheses)"),
        KeybindingRegistration::new("i[", textobjects::INNER_BRACKET)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Inner brackets"),
        KeybindingRegistration::new("i]", textobjects::INNER_BRACKET)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Inner brackets"),
        KeybindingRegistration::new("i{", textobjects::INNER_BRACE)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Inner braces"),
        KeybindingRegistration::new("i}", textobjects::INNER_BRACE)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Inner braces"),
        KeybindingRegistration::new("iB", textobjects::INNER_BRACE)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Inner Block (braces)"),
        KeybindingRegistration::new("i<lt>", textobjects::INNER_ANGLE)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Inner angle brackets"),
        KeybindingRegistration::new("i<gt>", textobjects::INNER_ANGLE)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Inner angle brackets"),
        KeybindingRegistration::new("it", textobjects::INNER_TAG)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Inner tag"),
        KeybindingRegistration::new("is", textobjects::INNER_SENTENCE)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Inner sentence"),
        KeybindingRegistration::new("ip", textobjects::INNER_PARAGRAPH)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Inner paragraph"),
        // Around text objects
        KeybindingRegistration::new("aw", textobjects::AROUND_WORD)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Around word"),
        KeybindingRegistration::new("aW", textobjects::AROUND_WORD_BIG)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Around WORD"),
        KeybindingRegistration::new("a\"", textobjects::AROUND_DOUBLE_QUOTE)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Around double quotes"),
        KeybindingRegistration::new("a'", textobjects::AROUND_SINGLE_QUOTE)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Around single quotes"),
        KeybindingRegistration::new("a`", textobjects::AROUND_BACKTICK)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Around backticks"),
        KeybindingRegistration::new("a(", textobjects::AROUND_PAREN)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Around parentheses"),
        KeybindingRegistration::new("a)", textobjects::AROUND_PAREN)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Around parentheses"),
        KeybindingRegistration::new("ab", textobjects::AROUND_PAREN)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Around block (parentheses)"),
        KeybindingRegistration::new("a[", textobjects::AROUND_BRACKET)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Around brackets"),
        KeybindingRegistration::new("a]", textobjects::AROUND_BRACKET)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Around brackets"),
        KeybindingRegistration::new("a{", textobjects::AROUND_BRACE)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Around braces"),
        KeybindingRegistration::new("a}", textobjects::AROUND_BRACE)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Around braces"),
        KeybindingRegistration::new("aB", textobjects::AROUND_BRACE)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Around Block (braces)"),
        KeybindingRegistration::new("a<lt>", textobjects::AROUND_ANGLE)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Around angle brackets"),
        KeybindingRegistration::new("a<gt>", textobjects::AROUND_ANGLE)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Around angle brackets"),
        KeybindingRegistration::new("at", textobjects::AROUND_TAG)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Around tag"),
        KeybindingRegistration::new("as", textobjects::AROUND_SENTENCE)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Around sentence"),
        KeybindingRegistration::new("ap", textobjects::AROUND_PARAGRAPH)
            .with_modes(&[DELETE_MODE])
            .with_category("textobj")
            .with_description("Around paragraph"),
    ]
}

// =============================================================================
// Yank Mode Motion and Text Object Bindings
// =============================================================================

fn yank_motion_bindings() -> Vec<KeybindingRegistration> {
    vec![
        KeybindingRegistration::new("h", editor::CURSOR_LEFT)
            .with_modes(&[YANK_MODE])
            .with_category("motion")
            .with_description("Left motion"),
        KeybindingRegistration::new("j", editor::CURSOR_DOWN)
            .with_modes(&[YANK_MODE])
            .with_category("motion")
            .with_description("Down motion"),
        KeybindingRegistration::new("k", editor::CURSOR_UP)
            .with_modes(&[YANK_MODE])
            .with_category("motion")
            .with_description("Up motion"),
        KeybindingRegistration::new("l", editor::CURSOR_RIGHT)
            .with_modes(&[YANK_MODE])
            .with_category("motion")
            .with_description("Right motion"),
        KeybindingRegistration::new("w", motions::WORD_FORWARD)
            .with_modes(&[YANK_MODE])
            .with_category("motion")
            .with_description("Word forward motion"),
        KeybindingRegistration::new("b", motions::WORD_BACKWARD)
            .with_modes(&[YANK_MODE])
            .with_category("motion")
            .with_description("Word backward motion"),
        KeybindingRegistration::new("e", motions::WORD_END)
            .with_modes(&[YANK_MODE])
            .with_category("motion")
            .with_description("Word end motion"),
        KeybindingRegistration::new("W", motions::WORD_FORWARD_BIG)
            .with_modes(&[YANK_MODE])
            .with_category("motion")
            .with_description("WORD forward motion"),
        KeybindingRegistration::new("B", motions::WORD_BACKWARD_BIG)
            .with_modes(&[YANK_MODE])
            .with_category("motion")
            .with_description("WORD backward motion"),
        KeybindingRegistration::new("E", motions::WORD_END_BIG)
            .with_modes(&[YANK_MODE])
            .with_category("motion")
            .with_description("WORD end motion"),
        KeybindingRegistration::new("ge", motions::WORD_END_BACKWARD)
            .with_modes(&[YANK_MODE])
            .with_category("motion")
            .with_description("Word end backward motion"),
        KeybindingRegistration::new("gE", motions::WORD_END_BACKWARD_BIG)
            .with_modes(&[YANK_MODE])
            .with_category("motion")
            .with_description("WORD end backward motion"),
        KeybindingRegistration::new("0", motions::LINE_START)
            .with_modes(&[YANK_MODE])
            .with_category("motion")
            .with_description("Line start motion"),
        KeybindingRegistration::new("$", motions::LINE_END)
            .with_modes(&[YANK_MODE])
            .with_category("motion")
            .with_description("Line end motion"),
        KeybindingRegistration::new("^", motions::FIRST_NON_BLANK)
            .with_modes(&[YANK_MODE])
            .with_category("motion")
            .with_description("First non-blank motion"),
        KeybindingRegistration::new("gg", motions::DOCUMENT_START)
            .with_modes(&[YANK_MODE])
            .with_category("motion")
            .with_description("Document start motion"),
        KeybindingRegistration::new("G", motions::DOCUMENT_END)
            .with_modes(&[YANK_MODE])
            .with_category("motion")
            .with_description("Document end motion"),
    ]
}

fn yank_textobject_bindings() -> Vec<KeybindingRegistration> {
    vec![
        KeybindingRegistration::new("iw", textobjects::INNER_WORD)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Inner word"),
        KeybindingRegistration::new("iW", textobjects::INNER_WORD_BIG)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Inner WORD"),
        KeybindingRegistration::new("i\"", textobjects::INNER_DOUBLE_QUOTE)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Inner double quotes"),
        KeybindingRegistration::new("i'", textobjects::INNER_SINGLE_QUOTE)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Inner single quotes"),
        KeybindingRegistration::new("i`", textobjects::INNER_BACKTICK)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Inner backticks"),
        KeybindingRegistration::new("i(", textobjects::INNER_PAREN)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Inner parentheses"),
        KeybindingRegistration::new("i)", textobjects::INNER_PAREN)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Inner parentheses"),
        KeybindingRegistration::new("ib", textobjects::INNER_PAREN)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Inner block (parentheses)"),
        KeybindingRegistration::new("i[", textobjects::INNER_BRACKET)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Inner brackets"),
        KeybindingRegistration::new("i]", textobjects::INNER_BRACKET)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Inner brackets"),
        KeybindingRegistration::new("i{", textobjects::INNER_BRACE)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Inner braces"),
        KeybindingRegistration::new("i}", textobjects::INNER_BRACE)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Inner braces"),
        KeybindingRegistration::new("iB", textobjects::INNER_BRACE)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Inner Block (braces)"),
        KeybindingRegistration::new("i<lt>", textobjects::INNER_ANGLE)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Inner angle brackets"),
        KeybindingRegistration::new("i<gt>", textobjects::INNER_ANGLE)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Inner angle brackets"),
        KeybindingRegistration::new("it", textobjects::INNER_TAG)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Inner tag"),
        KeybindingRegistration::new("is", textobjects::INNER_SENTENCE)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Inner sentence"),
        KeybindingRegistration::new("ip", textobjects::INNER_PARAGRAPH)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Inner paragraph"),
        KeybindingRegistration::new("aw", textobjects::AROUND_WORD)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Around word"),
        KeybindingRegistration::new("aW", textobjects::AROUND_WORD_BIG)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Around WORD"),
        KeybindingRegistration::new("a\"", textobjects::AROUND_DOUBLE_QUOTE)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Around double quotes"),
        KeybindingRegistration::new("a'", textobjects::AROUND_SINGLE_QUOTE)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Around single quotes"),
        KeybindingRegistration::new("a`", textobjects::AROUND_BACKTICK)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Around backticks"),
        KeybindingRegistration::new("a(", textobjects::AROUND_PAREN)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Around parentheses"),
        KeybindingRegistration::new("a)", textobjects::AROUND_PAREN)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Around parentheses"),
        KeybindingRegistration::new("ab", textobjects::AROUND_PAREN)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Around block (parentheses)"),
        KeybindingRegistration::new("a[", textobjects::AROUND_BRACKET)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Around brackets"),
        KeybindingRegistration::new("a]", textobjects::AROUND_BRACKET)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Around brackets"),
        KeybindingRegistration::new("a{", textobjects::AROUND_BRACE)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Around braces"),
        KeybindingRegistration::new("a}", textobjects::AROUND_BRACE)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Around braces"),
        KeybindingRegistration::new("aB", textobjects::AROUND_BRACE)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Around Block (braces)"),
        KeybindingRegistration::new("a<lt>", textobjects::AROUND_ANGLE)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Around angle brackets"),
        KeybindingRegistration::new("a<gt>", textobjects::AROUND_ANGLE)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Around angle brackets"),
        KeybindingRegistration::new("at", textobjects::AROUND_TAG)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Around tag"),
        KeybindingRegistration::new("as", textobjects::AROUND_SENTENCE)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Around sentence"),
        KeybindingRegistration::new("ap", textobjects::AROUND_PARAGRAPH)
            .with_modes(&[YANK_MODE])
            .with_category("textobj")
            .with_description("Around paragraph"),
    ]
}

// =============================================================================
// Change Mode Motion and Text Object Bindings
// =============================================================================

fn change_motion_bindings() -> Vec<KeybindingRegistration> {
    vec![
        KeybindingRegistration::new("h", editor::CURSOR_LEFT)
            .with_modes(&[CHANGE_MODE])
            .with_category("motion")
            .with_description("Left motion"),
        KeybindingRegistration::new("j", editor::CURSOR_DOWN)
            .with_modes(&[CHANGE_MODE])
            .with_category("motion")
            .with_description("Down motion"),
        KeybindingRegistration::new("k", editor::CURSOR_UP)
            .with_modes(&[CHANGE_MODE])
            .with_category("motion")
            .with_description("Up motion"),
        KeybindingRegistration::new("l", editor::CURSOR_RIGHT)
            .with_modes(&[CHANGE_MODE])
            .with_category("motion")
            .with_description("Right motion"),
        KeybindingRegistration::new("w", motions::WORD_FORWARD)
            .with_modes(&[CHANGE_MODE])
            .with_category("motion")
            .with_description("Word forward motion"),
        KeybindingRegistration::new("b", motions::WORD_BACKWARD)
            .with_modes(&[CHANGE_MODE])
            .with_category("motion")
            .with_description("Word backward motion"),
        KeybindingRegistration::new("e", motions::WORD_END)
            .with_modes(&[CHANGE_MODE])
            .with_category("motion")
            .with_description("Word end motion"),
        KeybindingRegistration::new("W", motions::WORD_FORWARD_BIG)
            .with_modes(&[CHANGE_MODE])
            .with_category("motion")
            .with_description("WORD forward motion"),
        KeybindingRegistration::new("B", motions::WORD_BACKWARD_BIG)
            .with_modes(&[CHANGE_MODE])
            .with_category("motion")
            .with_description("WORD backward motion"),
        KeybindingRegistration::new("E", motions::WORD_END_BIG)
            .with_modes(&[CHANGE_MODE])
            .with_category("motion")
            .with_description("WORD end motion"),
        KeybindingRegistration::new("ge", motions::WORD_END_BACKWARD)
            .with_modes(&[CHANGE_MODE])
            .with_category("motion")
            .with_description("Word end backward motion"),
        KeybindingRegistration::new("gE", motions::WORD_END_BACKWARD_BIG)
            .with_modes(&[CHANGE_MODE])
            .with_category("motion")
            .with_description("WORD end backward motion"),
        KeybindingRegistration::new("0", motions::LINE_START)
            .with_modes(&[CHANGE_MODE])
            .with_category("motion")
            .with_description("Line start motion"),
        KeybindingRegistration::new("$", motions::LINE_END)
            .with_modes(&[CHANGE_MODE])
            .with_category("motion")
            .with_description("Line end motion"),
        KeybindingRegistration::new("^", motions::FIRST_NON_BLANK)
            .with_modes(&[CHANGE_MODE])
            .with_category("motion")
            .with_description("First non-blank motion"),
        KeybindingRegistration::new("gg", motions::DOCUMENT_START)
            .with_modes(&[CHANGE_MODE])
            .with_category("motion")
            .with_description("Document start motion"),
        KeybindingRegistration::new("G", motions::DOCUMENT_END)
            .with_modes(&[CHANGE_MODE])
            .with_category("motion")
            .with_description("Document end motion"),
    ]
}

fn change_textobject_bindings() -> Vec<KeybindingRegistration> {
    vec![
        KeybindingRegistration::new("iw", textobjects::INNER_WORD)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Inner word"),
        KeybindingRegistration::new("iW", textobjects::INNER_WORD_BIG)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Inner WORD"),
        KeybindingRegistration::new("i\"", textobjects::INNER_DOUBLE_QUOTE)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Inner double quotes"),
        KeybindingRegistration::new("i'", textobjects::INNER_SINGLE_QUOTE)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Inner single quotes"),
        KeybindingRegistration::new("i`", textobjects::INNER_BACKTICK)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Inner backticks"),
        KeybindingRegistration::new("i(", textobjects::INNER_PAREN)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Inner parentheses"),
        KeybindingRegistration::new("i)", textobjects::INNER_PAREN)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Inner parentheses"),
        KeybindingRegistration::new("ib", textobjects::INNER_PAREN)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Inner block (parentheses)"),
        KeybindingRegistration::new("i[", textobjects::INNER_BRACKET)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Inner brackets"),
        KeybindingRegistration::new("i]", textobjects::INNER_BRACKET)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Inner brackets"),
        KeybindingRegistration::new("i{", textobjects::INNER_BRACE)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Inner braces"),
        KeybindingRegistration::new("i}", textobjects::INNER_BRACE)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Inner braces"),
        KeybindingRegistration::new("iB", textobjects::INNER_BRACE)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Inner Block (braces)"),
        KeybindingRegistration::new("i<lt>", textobjects::INNER_ANGLE)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Inner angle brackets"),
        KeybindingRegistration::new("i<gt>", textobjects::INNER_ANGLE)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Inner angle brackets"),
        KeybindingRegistration::new("it", textobjects::INNER_TAG)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Inner tag"),
        KeybindingRegistration::new("is", textobjects::INNER_SENTENCE)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Inner sentence"),
        KeybindingRegistration::new("ip", textobjects::INNER_PARAGRAPH)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Inner paragraph"),
        KeybindingRegistration::new("aw", textobjects::AROUND_WORD)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Around word"),
        KeybindingRegistration::new("aW", textobjects::AROUND_WORD_BIG)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Around WORD"),
        KeybindingRegistration::new("a\"", textobjects::AROUND_DOUBLE_QUOTE)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Around double quotes"),
        KeybindingRegistration::new("a'", textobjects::AROUND_SINGLE_QUOTE)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Around single quotes"),
        KeybindingRegistration::new("a`", textobjects::AROUND_BACKTICK)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Around backticks"),
        KeybindingRegistration::new("a(", textobjects::AROUND_PAREN)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Around parentheses"),
        KeybindingRegistration::new("a)", textobjects::AROUND_PAREN)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Around parentheses"),
        KeybindingRegistration::new("ab", textobjects::AROUND_PAREN)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Around block (parentheses)"),
        KeybindingRegistration::new("a[", textobjects::AROUND_BRACKET)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Around brackets"),
        KeybindingRegistration::new("a]", textobjects::AROUND_BRACKET)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Around brackets"),
        KeybindingRegistration::new("a{", textobjects::AROUND_BRACE)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Around braces"),
        KeybindingRegistration::new("a}", textobjects::AROUND_BRACE)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Around braces"),
        KeybindingRegistration::new("aB", textobjects::AROUND_BRACE)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Around Block (braces)"),
        KeybindingRegistration::new("a<lt>", textobjects::AROUND_ANGLE)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Around angle brackets"),
        KeybindingRegistration::new("a<gt>", textobjects::AROUND_ANGLE)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Around angle brackets"),
        KeybindingRegistration::new("at", textobjects::AROUND_TAG)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Around tag"),
        KeybindingRegistration::new("as", textobjects::AROUND_SENTENCE)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Around sentence"),
        KeybindingRegistration::new("ap", textobjects::AROUND_PARAGRAPH)
            .with_modes(&[CHANGE_MODE])
            .with_category("textobj")
            .with_description("Around paragraph"),
    ]
}

// =============================================================================
// Combined Bindings
// =============================================================================

/// Returns all operator mode keybindings (delete, yank, change).
#[must_use]
pub fn all_operator_bindings() -> Vec<KeybindingRegistration> {
    let mut bindings = Vec::new();
    bindings.extend(delete_bindings());
    bindings.extend(yank_bindings());
    bindings.extend(change_bindings());
    bindings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delete_bindings_not_empty() {
        let bindings = delete_bindings();
        assert!(!bindings.is_empty());
    }

    #[test]
    fn test_yank_bindings_not_empty() {
        let bindings = yank_bindings();
        assert!(!bindings.is_empty());
    }

    #[test]
    fn test_change_bindings_not_empty() {
        let bindings = change_bindings();
        assert!(!bindings.is_empty());
    }

    #[test]
    fn test_all_operator_bindings_count() {
        let all = all_operator_bindings();
        let delete = delete_bindings();
        let yank = yank_bindings();
        let change = change_bindings();

        assert_eq!(all.len(), delete.len() + yank.len() + change.len());
    }

    #[test]
    fn test_delete_mode_specific() {
        let bindings = delete_bindings();
        for b in &bindings {
            assert!(
                b.modes.contains(&DELETE_MODE),
                "binding {} should be for vim:delete mode",
                b.keys
            );
        }
    }

    #[test]
    fn test_yank_mode_specific() {
        let bindings = yank_bindings();
        for b in &bindings {
            assert!(
                b.modes.contains(&YANK_MODE),
                "binding {} should be for vim:yank mode",
                b.keys
            );
        }
    }

    #[test]
    fn test_change_mode_specific() {
        let bindings = change_bindings();
        for b in &bindings {
            assert!(
                b.modes.contains(&CHANGE_MODE),
                "binding {} should be for vim:change mode",
                b.keys
            );
        }
    }

    #[test]
    fn test_line_operator_keys() {
        // Verify dd, yy, cc are present
        let delete = delete_bindings();
        assert!(delete.iter().any(|b| b.keys == "d"), "delete should have 'd' for dd");

        let yank = yank_bindings();
        assert!(yank.iter().any(|b| b.keys == "y"), "yank should have 'y' for yy");

        let change = change_bindings();
        assert!(change.iter().any(|b| b.keys == "c"), "change should have 'c' for cc");
    }
}
