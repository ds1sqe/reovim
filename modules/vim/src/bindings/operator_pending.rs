//! Operator-pending mode keybindings.
//!
//! Reference: lib/core/src/bind/mod.rs (concept-extraction, not migration)
//!
//! Operator-pending mode is entered after pressing an operator (d, y, c)
//! and waits for a motion or text object to complete the operation.

use reovim_kernel::api::v1::KeybindingRegistration;

/// Operator-pending mode keybindings.
pub fn bindings() -> Vec<KeybindingRegistration> {
    vec![
        // ====================================================================
        // Exit operator-pending mode
        // ====================================================================
        KeybindingRegistration::new("<Esc>", "exit-operator-pending")
            .with_modes(&["editor:operator-pending"])
            .with_category("mode")
            .with_description("Cancel operator"),
        KeybindingRegistration::new("<C-c>", "exit-operator-pending")
            .with_modes(&["editor:operator-pending"])
            .with_category("mode")
            .with_description("Cancel operator"),
        // ====================================================================
        // Operator doubling (dd, yy, cc)
        // ====================================================================
        // When an operator key is pressed again in operator-pending mode,
        // it operates on the current line (whole-line motion).
        KeybindingRegistration::new("d", "motions:whole-line")
            .with_modes(&["editor:operator-pending"])
            .with_category("motion")
            .with_description("Whole line (for dd)"),
        KeybindingRegistration::new("y", "motions:whole-line")
            .with_modes(&["editor:operator-pending"])
            .with_category("motion")
            .with_description("Whole line (for yy)"),
        KeybindingRegistration::new("c", "motions:whole-line")
            .with_modes(&["editor:operator-pending"])
            .with_category("motion")
            .with_description("Whole line (for cc)"),
        // ====================================================================
        // Motions (complete the operator)
        // ====================================================================
        KeybindingRegistration::new("h", "cursor-left")
            .with_modes(&["editor:operator-pending"])
            .with_category("motion")
            .with_description("Left motion"),
        KeybindingRegistration::new("j", "cursor-down")
            .with_modes(&["editor:operator-pending"])
            .with_category("motion")
            .with_description("Down motion"),
        KeybindingRegistration::new("k", "cursor-up")
            .with_modes(&["editor:operator-pending"])
            .with_category("motion")
            .with_description("Up motion"),
        KeybindingRegistration::new("l", "cursor-right")
            .with_modes(&["editor:operator-pending"])
            .with_category("motion")
            .with_description("Right motion"),
        KeybindingRegistration::new("w", "motions:word-forward")
            .with_modes(&["editor:operator-pending"])
            .with_category("motion")
            .with_description("Word forward motion"),
        KeybindingRegistration::new("b", "motions:word-backward")
            .with_modes(&["editor:operator-pending"])
            .with_category("motion")
            .with_description("Word backward motion"),
        KeybindingRegistration::new("e", "motions:word-end")
            .with_modes(&["editor:operator-pending"])
            .with_category("motion")
            .with_description("Word end motion"),
        KeybindingRegistration::new("W", "motions:word-forward-big")
            .with_modes(&["editor:operator-pending"])
            .with_category("motion")
            .with_description("WORD forward motion"),
        KeybindingRegistration::new("B", "motions:word-backward-big")
            .with_modes(&["editor:operator-pending"])
            .with_category("motion")
            .with_description("WORD backward motion"),
        KeybindingRegistration::new("E", "motions:word-end-big")
            .with_modes(&["editor:operator-pending"])
            .with_category("motion")
            .with_description("WORD end motion"),
        KeybindingRegistration::new("ge", "motions:word-end-backward")
            .with_modes(&["editor:operator-pending"])
            .with_category("motion")
            .with_description("Word end backward motion"),
        KeybindingRegistration::new("gE", "motions:word-end-backward-big")
            .with_modes(&["editor:operator-pending"])
            .with_category("motion")
            .with_description("WORD end backward motion"),
        KeybindingRegistration::new("0", "motions:line-start")
            .with_modes(&["editor:operator-pending"])
            .with_category("motion")
            .with_description("Line start motion"),
        KeybindingRegistration::new("$", "motions:line-end")
            .with_modes(&["editor:operator-pending"])
            .with_category("motion")
            .with_description("Line end motion"),
        KeybindingRegistration::new("^", "motions:first-non-blank")
            .with_modes(&["editor:operator-pending"])
            .with_category("motion")
            .with_description("First non-blank motion"),
        KeybindingRegistration::new("gg", "motions:document-start")
            .with_modes(&["editor:operator-pending"])
            .with_category("motion")
            .with_description("Document start motion"),
        KeybindingRegistration::new("G", "motions:document-end")
            .with_modes(&["editor:operator-pending"])
            .with_category("motion")
            .with_description("Document end motion"),
        // ====================================================================
        // Text objects - inner
        // ====================================================================
        KeybindingRegistration::new("iw", "textobjects:inner-word")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Inner word"),
        KeybindingRegistration::new("iW", "textobjects:inner-word-big")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Inner WORD"),
        KeybindingRegistration::new("i\"", "textobjects:inner-double-quote")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Inner double quotes"),
        KeybindingRegistration::new("i'", "textobjects:inner-single-quote")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Inner single quotes"),
        KeybindingRegistration::new("i`", "textobjects:inner-backtick")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Inner backticks"),
        KeybindingRegistration::new("i(", "textobjects:inner-paren")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Inner parentheses"),
        KeybindingRegistration::new("i)", "textobjects:inner-paren")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Inner parentheses"),
        KeybindingRegistration::new("ib", "textobjects:inner-paren")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Inner block (parentheses)"),
        KeybindingRegistration::new("i[", "textobjects:inner-bracket")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Inner brackets"),
        KeybindingRegistration::new("i]", "textobjects:inner-bracket")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Inner brackets"),
        KeybindingRegistration::new("i{", "textobjects:inner-brace")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Inner braces"),
        KeybindingRegistration::new("i}", "textobjects:inner-brace")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Inner braces"),
        KeybindingRegistration::new("iB", "textobjects:inner-brace")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Inner Block (braces)"),
        KeybindingRegistration::new("i<", "textobjects:inner-angle")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Inner angle brackets"),
        KeybindingRegistration::new("i>", "textobjects:inner-angle")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Inner angle brackets"),
        KeybindingRegistration::new("it", "textobjects:inner-tag")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Inner tag"),
        KeybindingRegistration::new("is", "textobjects:inner-sentence")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Inner sentence"),
        KeybindingRegistration::new("ip", "textobjects:inner-paragraph")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Inner paragraph"),
        // ====================================================================
        // Text objects - around
        // ====================================================================
        KeybindingRegistration::new("aw", "textobjects:around-word")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Around word"),
        KeybindingRegistration::new("aW", "textobjects:around-word-big")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Around WORD"),
        KeybindingRegistration::new("a\"", "textobjects:around-double-quote")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Around double quotes"),
        KeybindingRegistration::new("a'", "textobjects:around-single-quote")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Around single quotes"),
        KeybindingRegistration::new("a`", "textobjects:around-backtick")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Around backticks"),
        KeybindingRegistration::new("a(", "textobjects:around-paren")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Around parentheses"),
        KeybindingRegistration::new("a)", "textobjects:around-paren")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Around parentheses"),
        KeybindingRegistration::new("ab", "textobjects:around-paren")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Around block (parentheses)"),
        KeybindingRegistration::new("a[", "textobjects:around-bracket")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Around brackets"),
        KeybindingRegistration::new("a]", "textobjects:around-bracket")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Around brackets"),
        KeybindingRegistration::new("a{", "textobjects:around-brace")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Around braces"),
        KeybindingRegistration::new("a}", "textobjects:around-brace")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Around braces"),
        KeybindingRegistration::new("aB", "textobjects:around-brace")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Around Block (braces)"),
        KeybindingRegistration::new("a<", "textobjects:around-angle")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Around angle brackets"),
        KeybindingRegistration::new("a>", "textobjects:around-angle")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Around angle brackets"),
        KeybindingRegistration::new("at", "textobjects:around-tag")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Around tag"),
        KeybindingRegistration::new("as", "textobjects:around-sentence")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Around sentence"),
        KeybindingRegistration::new("ap", "textobjects:around-paragraph")
            .with_modes(&["editor:operator-pending"])
            .with_category("textobj")
            .with_description("Around paragraph"),
    ]
}
