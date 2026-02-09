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
        KeybindingRegistration::new("<Esc>", vim::CANCEL_TO_NORMAL)
            .with_modes(&[DELETE_MODE])
            .with_category("mode")
            .with_description("Cancel delete"),
        KeybindingRegistration::new("<C-c>", vim::CANCEL_TO_NORMAL)
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
        KeybindingRegistration::new("<Esc>", vim::CANCEL_TO_NORMAL)
            .with_modes(&[YANK_MODE])
            .with_category("mode")
            .with_description("Cancel yank"),
        KeybindingRegistration::new("<C-c>", vim::CANCEL_TO_NORMAL)
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
        KeybindingRegistration::new("<Esc>", vim::CANCEL_TO_NORMAL)
            .with_modes(&[CHANGE_MODE])
            .with_category("mode")
            .with_description("Cancel change"),
        KeybindingRegistration::new("<C-c>", vim::CANCEL_TO_NORMAL)
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

#[allow(clippy::too_many_lines)]
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

#[allow(clippy::too_many_lines)]
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

#[allow(clippy::too_many_lines)]
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
            assert!(b.modes.contains(&YANK_MODE), "binding {} should be for vim:yank mode", b.keys);
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

    #[test]
    fn test_escape_bindings_in_all_modes() {
        let delete = delete_bindings();
        assert!(delete.iter().any(|b| b.keys == "<Esc>"), "delete mode should have <Esc>");

        let yank = yank_bindings();
        assert!(yank.iter().any(|b| b.keys == "<Esc>"), "yank mode should have <Esc>");

        let change = change_bindings();
        assert!(change.iter().any(|b| b.keys == "<Esc>"), "change mode should have <Esc>");
    }

    #[test]
    fn test_ctrl_c_bindings_in_all_modes() {
        let delete = delete_bindings();
        assert!(delete.iter().any(|b| b.keys == "<C-c>"), "delete mode should have <C-c>");

        let yank = yank_bindings();
        assert!(yank.iter().any(|b| b.keys == "<C-c>"), "yank mode should have <C-c>");

        let change = change_bindings();
        assert!(change.iter().any(|b| b.keys == "<C-c>"), "change mode should have <C-c>");
    }

    #[test]
    fn test_motion_bindings_present_in_delete() {
        let delete = delete_bindings();
        let keys: Vec<_> = delete.iter().map(|b| b.keys).collect();
        assert!(keys.contains(&"h"), "delete mode missing 'h' motion");
        assert!(keys.contains(&"j"), "delete mode missing 'j' motion");
        assert!(keys.contains(&"k"), "delete mode missing 'k' motion");
        assert!(keys.contains(&"l"), "delete mode missing 'l' motion");
        assert!(keys.contains(&"w"), "delete mode missing 'w' motion");
        assert!(keys.contains(&"b"), "delete mode missing 'b' motion");
        assert!(keys.contains(&"e"), "delete mode missing 'e' motion");
        assert!(keys.contains(&"0"), "delete mode missing '0' motion");
        assert!(keys.contains(&"$"), "delete mode missing '$' motion");
        assert!(keys.contains(&"gg"), "delete mode missing 'gg' motion");
        assert!(keys.contains(&"G"), "delete mode missing 'G' motion");
    }

    #[test]
    fn test_textobject_bindings_present_in_delete() {
        let delete = delete_bindings();
        let keys: Vec<_> = delete.iter().map(|b| b.keys).collect();
        assert!(keys.contains(&"iw"), "delete mode missing 'iw' textobj");
        assert!(keys.contains(&"aw"), "delete mode missing 'aw' textobj");
        assert!(keys.contains(&"i\""), "delete mode missing 'i\"' textobj");
        assert!(keys.contains(&"a\""), "delete mode missing 'a\"' textobj");
        assert!(keys.contains(&"i("), "delete mode missing 'i(' textobj");
        assert!(keys.contains(&"a("), "delete mode missing 'a(' textobj");
    }

    #[test]
    fn test_motion_bindings_present_in_yank() {
        let yank = yank_bindings();
        let keys: Vec<_> = yank.iter().map(|b| b.keys).collect();
        assert!(keys.contains(&"w"), "yank mode missing 'w' motion");
        assert!(keys.contains(&"b"), "yank mode missing 'b' motion");
        assert!(keys.contains(&"$"), "yank mode missing '$' motion");
        assert!(keys.contains(&"gg"), "yank mode missing 'gg' motion");
        assert!(keys.contains(&"G"), "yank mode missing 'G' motion");
    }

    #[test]
    fn test_motion_bindings_present_in_change() {
        let change = change_bindings();
        let keys: Vec<_> = change.iter().map(|b| b.keys).collect();
        assert!(keys.contains(&"w"), "change mode missing 'w' motion");
        assert!(keys.contains(&"b"), "change mode missing 'b' motion");
        assert!(keys.contains(&"$"), "change mode missing '$' motion");
        assert!(keys.contains(&"gg"), "change mode missing 'gg' motion");
        assert!(keys.contains(&"G"), "change mode missing 'G' motion");
    }

    #[test]
    fn test_textobject_bindings_consistent_across_modes() {
        // All three modes should have the same textobject bindings
        let delete_count = delete_bindings()
            .iter()
            .filter(|b| b.category == Some("textobj"))
            .count();
        let yank_count = yank_bindings()
            .iter()
            .filter(|b| b.category == Some("textobj"))
            .count();
        let change_count = change_bindings()
            .iter()
            .filter(|b| b.category == Some("textobj"))
            .count();

        assert_eq!(
            delete_count, yank_count,
            "delete and yank should have same number of textobj bindings"
        );
        assert_eq!(
            yank_count, change_count,
            "yank and change should have same number of textobj bindings"
        );
    }

    #[test]
    fn test_all_bindings_have_description() {
        for binding in all_operator_bindings() {
            assert!(
                !binding.description.is_empty(),
                "Binding '{}' should have a description",
                binding.keys
            );
        }
    }

    #[test]
    fn test_all_bindings_have_category() {
        for binding in all_operator_bindings() {
            assert!(
                binding.category.is_some(),
                "Binding '{}' should have a category",
                binding.keys
            );
        }
    }

    #[test]
    fn test_big_word_motions_in_operator_modes() {
        let delete = delete_bindings();
        let keys: Vec<_> = delete.iter().map(|b| b.keys).collect();
        assert!(keys.contains(&"W"), "delete mode missing 'W' motion");
        assert!(keys.contains(&"B"), "delete mode missing 'B' motion");
        assert!(keys.contains(&"E"), "delete mode missing 'E' motion");
        assert!(keys.contains(&"ge"), "delete mode missing 'ge' motion");
        assert!(keys.contains(&"gE"), "delete mode missing 'gE' motion");
    }

    #[test]
    fn test_first_non_blank_in_operator_modes() {
        let delete = delete_bindings();
        assert!(delete.iter().any(|b| b.keys == "^"), "delete mode missing '^' motion");

        let yank = yank_bindings();
        assert!(yank.iter().any(|b| b.keys == "^"), "yank mode missing '^' motion");

        let change = change_bindings();
        assert!(change.iter().any(|b| b.keys == "^"), "change mode missing '^' motion");
    }

    // ========================================================================
    // Binding consistency across operator modes
    // ========================================================================

    #[test]
    fn test_motion_bindings_consistent_across_modes() {
        let delete_count = delete_bindings()
            .iter()
            .filter(|b| b.category == Some("motion"))
            .count();
        let yank_count = yank_bindings()
            .iter()
            .filter(|b| b.category == Some("motion"))
            .count();
        let change_count = change_bindings()
            .iter()
            .filter(|b| b.category == Some("motion"))
            .count();

        assert_eq!(
            delete_count, yank_count,
            "delete and yank should have same number of motion bindings"
        );
        assert_eq!(
            yank_count, change_count,
            "yank and change should have same number of motion bindings"
        );
    }

    #[test]
    fn test_textobject_bindings_in_yank() {
        let yank = yank_bindings();
        let keys: Vec<_> = yank.iter().map(|b| b.keys).collect();
        assert!(keys.contains(&"iw"), "yank mode missing 'iw' textobj");
        assert!(keys.contains(&"aw"), "yank mode missing 'aw' textobj");
        assert!(keys.contains(&"i\""), "yank mode missing 'i\"' textobj");
        assert!(keys.contains(&"a\""), "yank mode missing 'a\"' textobj");
        assert!(keys.contains(&"i("), "yank mode missing 'i(' textobj");
        assert!(keys.contains(&"a("), "yank mode missing 'a(' textobj");
    }

    #[test]
    fn test_textobject_bindings_in_change() {
        let change = change_bindings();
        let keys: Vec<_> = change.iter().map(|b| b.keys).collect();
        assert!(keys.contains(&"iw"), "change mode missing 'iw' textobj");
        assert!(keys.contains(&"aw"), "change mode missing 'aw' textobj");
        assert!(keys.contains(&"i\""), "change mode missing 'i\"' textobj");
        assert!(keys.contains(&"a\""), "change mode missing 'a\"' textobj");
        assert!(keys.contains(&"i("), "change mode missing 'i(' textobj");
        assert!(keys.contains(&"a("), "change mode missing 'a(' textobj");
    }

    #[test]
    fn test_backtick_textobjects_in_all_modes() {
        let delete = delete_bindings();
        let delete_keys: Vec<_> = delete.iter().map(|b| b.keys).collect();
        assert!(delete_keys.contains(&"i`"), "delete mode missing 'i`' textobj");
        assert!(delete_keys.contains(&"a`"), "delete mode missing 'a`' textobj");

        let yank = yank_bindings();
        let yank_keys: Vec<_> = yank.iter().map(|b| b.keys).collect();
        assert!(yank_keys.contains(&"i`"), "yank mode missing 'i`' textobj");
        assert!(yank_keys.contains(&"a`"), "yank mode missing 'a`' textobj");

        let change = change_bindings();
        let change_keys: Vec<_> = change.iter().map(|b| b.keys).collect();
        assert!(change_keys.contains(&"i`"), "change mode missing 'i`' textobj");
        assert!(change_keys.contains(&"a`"), "change mode missing 'a`' textobj");
    }

    #[test]
    fn test_tag_textobjects_in_all_modes() {
        let delete = delete_bindings();
        let delete_keys: Vec<_> = delete.iter().map(|b| b.keys).collect();
        assert!(delete_keys.contains(&"it"), "delete mode missing 'it' textobj");
        assert!(delete_keys.contains(&"at"), "delete mode missing 'at' textobj");

        let yank = yank_bindings();
        let yank_keys: Vec<_> = yank.iter().map(|b| b.keys).collect();
        assert!(yank_keys.contains(&"it"), "yank mode missing 'it' textobj");
        assert!(yank_keys.contains(&"at"), "yank mode missing 'at' textobj");

        let change = change_bindings();
        let change_keys: Vec<_> = change.iter().map(|b| b.keys).collect();
        assert!(change_keys.contains(&"it"), "change mode missing 'it' textobj");
        assert!(change_keys.contains(&"at"), "change mode missing 'at' textobj");
    }

    #[test]
    fn test_sentence_textobjects_in_all_modes() {
        let delete = delete_bindings();
        let delete_keys: Vec<_> = delete.iter().map(|b| b.keys).collect();
        assert!(delete_keys.contains(&"is"), "delete mode missing 'is' textobj");
        assert!(delete_keys.contains(&"as"), "delete mode missing 'as' textobj");

        let yank = yank_bindings();
        let yank_keys: Vec<_> = yank.iter().map(|b| b.keys).collect();
        assert!(yank_keys.contains(&"is"), "yank mode missing 'is' textobj");
        assert!(yank_keys.contains(&"as"), "yank mode missing 'as' textobj");

        let change = change_bindings();
        let change_keys: Vec<_> = change.iter().map(|b| b.keys).collect();
        assert!(change_keys.contains(&"is"), "change mode missing 'is' textobj");
        assert!(change_keys.contains(&"as"), "change mode missing 'as' textobj");
    }

    #[test]
    fn test_paragraph_textobjects_in_all_modes() {
        let delete = delete_bindings();
        let delete_keys: Vec<_> = delete.iter().map(|b| b.keys).collect();
        assert!(delete_keys.contains(&"ip"), "delete mode missing 'ip' textobj");
        assert!(delete_keys.contains(&"ap"), "delete mode missing 'ap' textobj");

        let yank = yank_bindings();
        let yank_keys: Vec<_> = yank.iter().map(|b| b.keys).collect();
        assert!(yank_keys.contains(&"ip"), "yank mode missing 'ip' textobj");
        assert!(yank_keys.contains(&"ap"), "yank mode missing 'ap' textobj");

        let change = change_bindings();
        let change_keys: Vec<_> = change.iter().map(|b| b.keys).collect();
        assert!(change_keys.contains(&"ip"), "change mode missing 'ip' textobj");
        assert!(change_keys.contains(&"ap"), "change mode missing 'ap' textobj");
    }

    #[test]
    fn test_big_word_motions_in_yank() {
        let yank = yank_bindings();
        let keys: Vec<_> = yank.iter().map(|b| b.keys).collect();
        assert!(keys.contains(&"W"), "yank mode missing 'W' motion");
        assert!(keys.contains(&"B"), "yank mode missing 'B' motion");
        assert!(keys.contains(&"E"), "yank mode missing 'E' motion");
        assert!(keys.contains(&"ge"), "yank mode missing 'ge' motion");
        assert!(keys.contains(&"gE"), "yank mode missing 'gE' motion");
    }

    #[test]
    fn test_big_word_motions_in_change() {
        let change = change_bindings();
        let keys: Vec<_> = change.iter().map(|b| b.keys).collect();
        assert!(keys.contains(&"W"), "change mode missing 'W' motion");
        assert!(keys.contains(&"B"), "change mode missing 'B' motion");
        assert!(keys.contains(&"E"), "change mode missing 'E' motion");
        assert!(keys.contains(&"ge"), "change mode missing 'ge' motion");
        assert!(keys.contains(&"gE"), "change mode missing 'gE' motion");
    }

    #[test]
    fn test_all_operator_bindings_not_empty_description() {
        for binding in all_operator_bindings() {
            assert!(
                !binding.description.is_empty(),
                "Binding '{}' should have a non-empty description",
                binding.keys
            );
        }
    }

    #[test]
    fn test_angle_bracket_textobjects_in_all_modes() {
        let delete = delete_bindings();
        let delete_keys: Vec<_> = delete.iter().map(|b| b.keys).collect();
        assert!(delete_keys.contains(&"i<lt>"), "delete mode missing 'i<lt>' textobj");
        assert!(delete_keys.contains(&"i<gt>"), "delete mode missing 'i<gt>' textobj");
        assert!(delete_keys.contains(&"a<lt>"), "delete mode missing 'a<lt>' textobj");
        assert!(delete_keys.contains(&"a<gt>"), "delete mode missing 'a<gt>' textobj");

        let yank = yank_bindings();
        let yank_keys: Vec<_> = yank.iter().map(|b| b.keys).collect();
        assert!(yank_keys.contains(&"i<lt>"), "yank mode missing 'i<lt>' textobj");
        assert!(yank_keys.contains(&"a<gt>"), "yank mode missing 'a<gt>' textobj");

        let change = change_bindings();
        let change_keys: Vec<_> = change.iter().map(|b| b.keys).collect();
        assert!(change_keys.contains(&"i<lt>"), "change mode missing 'i<lt>' textobj");
        assert!(change_keys.contains(&"a<gt>"), "change mode missing 'a<gt>' textobj");
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_brace_textobjects_in_all_modes() {
        for mode_name in ["delete", "yank", "change"] {
            let bindings = match mode_name {
                "delete" => delete_bindings(),
                "yank" => yank_bindings(),
                "change" => change_bindings(),
                _ => unreachable!(),
            };
            let keys: Vec<_> = bindings.iter().map(|b| b.keys).collect();
            assert!(keys.contains(&"i{"), "{mode_name} mode missing 'i{{' textobj");
            assert!(keys.contains(&"i}"), "{mode_name} mode missing 'i}}' textobj");
            assert!(keys.contains(&"iB"), "{mode_name} mode missing 'iB' textobj");
            assert!(keys.contains(&"a{"), "{mode_name} mode missing 'a{{' textobj");
            assert!(keys.contains(&"a}"), "{mode_name} mode missing 'a}}' textobj");
            assert!(keys.contains(&"aB"), "{mode_name} mode missing 'aB' textobj");
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_bracket_textobjects_in_all_modes() {
        for mode_name in ["delete", "yank", "change"] {
            let bindings = match mode_name {
                "delete" => delete_bindings(),
                "yank" => yank_bindings(),
                "change" => change_bindings(),
                _ => unreachable!(),
            };
            let keys: Vec<_> = bindings.iter().map(|b| b.keys).collect();
            assert!(keys.contains(&"i["), "{mode_name} mode missing 'i[' textobj");
            assert!(keys.contains(&"i]"), "{mode_name} mode missing 'i]' textobj");
            assert!(keys.contains(&"a["), "{mode_name} mode missing 'a[' textobj");
            assert!(keys.contains(&"a]"), "{mode_name} mode missing 'a]' textobj");
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_paren_textobjects_with_aliases_in_all_modes() {
        for mode_name in ["delete", "yank", "change"] {
            let bindings = match mode_name {
                "delete" => delete_bindings(),
                "yank" => yank_bindings(),
                "change" => change_bindings(),
                _ => unreachable!(),
            };
            let keys: Vec<_> = bindings.iter().map(|b| b.keys).collect();
            assert!(keys.contains(&"i("), "{mode_name} mode missing 'i(' textobj");
            assert!(keys.contains(&"i)"), "{mode_name} mode missing 'i)' textobj");
            assert!(keys.contains(&"ib"), "{mode_name} mode missing 'ib' textobj");
            assert!(keys.contains(&"a("), "{mode_name} mode missing 'a(' textobj");
            assert!(keys.contains(&"a)"), "{mode_name} mode missing 'a)' textobj");
            assert!(keys.contains(&"ab"), "{mode_name} mode missing 'ab' textobj");
        }
    }
}
