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
            .with_modes(&["operator-pending"])
            .with_category("mode")
            .with_description("Cancel operator"),
        KeybindingRegistration::new("<C-c>", "exit-operator-pending")
            .with_modes(&["operator-pending"])
            .with_category("mode")
            .with_description("Cancel operator"),
        // ====================================================================
        // Motions (complete the operator)
        // ====================================================================
        KeybindingRegistration::new("h", "motion-left")
            .with_modes(&["operator-pending"])
            .with_category("motion")
            .with_description("Left motion"),
        KeybindingRegistration::new("j", "motion-down")
            .with_modes(&["operator-pending"])
            .with_category("motion")
            .with_description("Down motion"),
        KeybindingRegistration::new("k", "motion-up")
            .with_modes(&["operator-pending"])
            .with_category("motion")
            .with_description("Up motion"),
        KeybindingRegistration::new("l", "motion-right")
            .with_modes(&["operator-pending"])
            .with_category("motion")
            .with_description("Right motion"),
        KeybindingRegistration::new("w", "motion-word-forward")
            .with_modes(&["operator-pending"])
            .with_category("motion")
            .with_description("Word forward motion"),
        KeybindingRegistration::new("b", "motion-word-backward")
            .with_modes(&["operator-pending"])
            .with_category("motion")
            .with_description("Word backward motion"),
        KeybindingRegistration::new("e", "motion-word-end")
            .with_modes(&["operator-pending"])
            .with_category("motion")
            .with_description("Word end motion"),
        KeybindingRegistration::new("W", "motion-word-forward-big")
            .with_modes(&["operator-pending"])
            .with_category("motion")
            .with_description("WORD forward motion"),
        KeybindingRegistration::new("B", "motion-word-backward-big")
            .with_modes(&["operator-pending"])
            .with_category("motion")
            .with_description("WORD backward motion"),
        KeybindingRegistration::new("E", "motion-word-end-big")
            .with_modes(&["operator-pending"])
            .with_category("motion")
            .with_description("WORD end motion"),
        KeybindingRegistration::new("0", "motion-line-start")
            .with_modes(&["operator-pending"])
            .with_category("motion")
            .with_description("Line start motion"),
        KeybindingRegistration::new("$", "motion-line-end")
            .with_modes(&["operator-pending"])
            .with_category("motion")
            .with_description("Line end motion"),
        KeybindingRegistration::new("^", "motion-first-non-blank")
            .with_modes(&["operator-pending"])
            .with_category("motion")
            .with_description("First non-blank motion"),
        KeybindingRegistration::new("gg", "motion-document-start")
            .with_modes(&["operator-pending"])
            .with_category("motion")
            .with_description("Document start motion"),
        KeybindingRegistration::new("G", "motion-document-end")
            .with_modes(&["operator-pending"])
            .with_category("motion")
            .with_description("Document end motion"),
        // ====================================================================
        // Text objects - inner
        // ====================================================================
        KeybindingRegistration::new("iw", "textobj-inner-word")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Inner word"),
        KeybindingRegistration::new("iW", "textobj-inner-word-big")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Inner WORD"),
        KeybindingRegistration::new("i\"", "textobj-inner-double-quote")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Inner double quotes"),
        KeybindingRegistration::new("i'", "textobj-inner-single-quote")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Inner single quotes"),
        KeybindingRegistration::new("i`", "textobj-inner-backtick")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Inner backticks"),
        KeybindingRegistration::new("i(", "textobj-inner-paren")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Inner parentheses"),
        KeybindingRegistration::new("i)", "textobj-inner-paren")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Inner parentheses"),
        KeybindingRegistration::new("ib", "textobj-inner-paren")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Inner block (parentheses)"),
        KeybindingRegistration::new("i[", "textobj-inner-bracket")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Inner brackets"),
        KeybindingRegistration::new("i]", "textobj-inner-bracket")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Inner brackets"),
        KeybindingRegistration::new("i{", "textobj-inner-brace")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Inner braces"),
        KeybindingRegistration::new("i}", "textobj-inner-brace")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Inner braces"),
        KeybindingRegistration::new("iB", "textobj-inner-brace")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Inner Block (braces)"),
        KeybindingRegistration::new("i<", "textobj-inner-angle")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Inner angle brackets"),
        KeybindingRegistration::new("i>", "textobj-inner-angle")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Inner angle brackets"),
        KeybindingRegistration::new("it", "textobj-inner-tag")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Inner tag"),
        KeybindingRegistration::new("is", "textobj-inner-sentence")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Inner sentence"),
        KeybindingRegistration::new("ip", "textobj-inner-paragraph")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Inner paragraph"),
        // ====================================================================
        // Text objects - around
        // ====================================================================
        KeybindingRegistration::new("aw", "textobj-around-word")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Around word"),
        KeybindingRegistration::new("aW", "textobj-around-word-big")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Around WORD"),
        KeybindingRegistration::new("a\"", "textobj-around-double-quote")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Around double quotes"),
        KeybindingRegistration::new("a'", "textobj-around-single-quote")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Around single quotes"),
        KeybindingRegistration::new("a`", "textobj-around-backtick")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Around backticks"),
        KeybindingRegistration::new("a(", "textobj-around-paren")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Around parentheses"),
        KeybindingRegistration::new("a)", "textobj-around-paren")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Around parentheses"),
        KeybindingRegistration::new("ab", "textobj-around-paren")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Around block (parentheses)"),
        KeybindingRegistration::new("a[", "textobj-around-bracket")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Around brackets"),
        KeybindingRegistration::new("a]", "textobj-around-bracket")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Around brackets"),
        KeybindingRegistration::new("a{", "textobj-around-brace")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Around braces"),
        KeybindingRegistration::new("a}", "textobj-around-brace")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Around braces"),
        KeybindingRegistration::new("aB", "textobj-around-brace")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Around Block (braces)"),
        KeybindingRegistration::new("a<", "textobj-around-angle")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Around angle brackets"),
        KeybindingRegistration::new("a>", "textobj-around-angle")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Around angle brackets"),
        KeybindingRegistration::new("at", "textobj-around-tag")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Around tag"),
        KeybindingRegistration::new("as", "textobj-around-sentence")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Around sentence"),
        KeybindingRegistration::new("ap", "textobj-around-paragraph")
            .with_modes(&["operator-pending"])
            .with_category("textobj")
            .with_description("Around paragraph"),
    ]
}
