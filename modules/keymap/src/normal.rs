//! Normal mode keybindings.
//!
//! Reference: lib/core/src/bind/mod.rs (concept-extraction, not migration)

use reovim_kernel::api::v1::KeybindingRegistration;

/// Normal mode keybindings.
///
/// These are POLICY decisions - which keys trigger which commands.
/// The kernel provides the mechanisms (motion calculation, buffer ops).
pub fn bindings() -> Vec<KeybindingRegistration> {
    vec![
        // ====================================================================
        // Movement - hjkl
        // ====================================================================
        KeybindingRegistration::new("h", "cursor-left")
            .with_modes(&["editor:normal"])
            .with_category("motion")
            .with_description("Move cursor left"),
        KeybindingRegistration::new("j", "cursor-down")
            .with_modes(&["editor:normal"])
            .with_category("motion")
            .with_description("Move cursor down"),
        KeybindingRegistration::new("k", "cursor-up")
            .with_modes(&["editor:normal"])
            .with_category("motion")
            .with_description("Move cursor up"),
        KeybindingRegistration::new("l", "cursor-right")
            .with_modes(&["editor:normal"])
            .with_category("motion")
            .with_description("Move cursor right"),
        // ====================================================================
        // Display line movement (gj/gk)
        // ====================================================================
        KeybindingRegistration::new("gj", "cursor-display-down")
            .with_modes(&["editor:normal"])
            .with_category("motion")
            .with_description("Move cursor down one display line"),
        KeybindingRegistration::new("gk", "cursor-display-up")
            .with_modes(&["editor:normal"])
            .with_category("motion")
            .with_description("Move cursor up one display line"),
        // ====================================================================
        // Word motions (provided by motions module)
        // ====================================================================
        KeybindingRegistration::new("w", "motions:word-forward")
            .with_modes(&["editor:normal"])
            .with_category("motion")
            .with_description("Move to next word"),
        KeybindingRegistration::new("b", "motions:word-backward")
            .with_modes(&["editor:normal"])
            .with_category("motion")
            .with_description("Move to previous word"),
        KeybindingRegistration::new("e", "motions:word-end")
            .with_modes(&["editor:normal"])
            .with_category("motion")
            .with_description("Move to end of word"),
        KeybindingRegistration::new("W", "motions:word-forward-big")
            .with_modes(&["editor:normal"])
            .with_category("motion")
            .with_description("Move to next WORD"),
        KeybindingRegistration::new("B", "motions:word-backward-big")
            .with_modes(&["editor:normal"])
            .with_category("motion")
            .with_description("Move to previous WORD"),
        KeybindingRegistration::new("E", "motions:word-end-big")
            .with_modes(&["editor:normal"])
            .with_category("motion")
            .with_description("Move to end of WORD"),
        KeybindingRegistration::new("ge", "motions:word-end-backward")
            .with_modes(&["editor:normal"])
            .with_category("motion")
            .with_description("Move to end of previous word"),
        KeybindingRegistration::new("gE", "motions:word-end-backward-big")
            .with_modes(&["editor:normal"])
            .with_category("motion")
            .with_description("Move to end of previous WORD"),
        // ====================================================================
        // Line motions (provided by motions module)
        // ====================================================================
        KeybindingRegistration::new("0", "motions:line-start")
            .with_modes(&["editor:normal"])
            .with_category("motion")
            .with_description("Move to start of line"),
        KeybindingRegistration::new("$", "motions:line-end")
            .with_modes(&["editor:normal"])
            .with_category("motion")
            .with_description("Move to end of line"),
        KeybindingRegistration::new("^", "motions:first-non-blank")
            .with_modes(&["editor:normal"])
            .with_category("motion")
            .with_description("Move to first non-blank character"),
        // ====================================================================
        // Document motions (provided by motions module)
        // ====================================================================
        KeybindingRegistration::new("gg", "motions:document-start")
            .with_modes(&["editor:normal"])
            .with_category("motion")
            .with_description("Go to start of document"),
        KeybindingRegistration::new("G", "motions:document-end")
            .with_modes(&["editor:normal"])
            .with_category("motion")
            .with_description("Go to end of document"),
        // ====================================================================
        // Mode switching
        // ====================================================================
        KeybindingRegistration::new("i", "enter-insert")
            .with_modes(&["editor:normal"])
            .with_category("mode")
            .with_description("Enter insert mode"),
        KeybindingRegistration::new("a", "enter-insert-after")
            .with_modes(&["editor:normal"])
            .with_category("mode")
            .with_description("Enter insert mode after cursor"),
        KeybindingRegistration::new("A", "enter-insert-eol")
            .with_modes(&["editor:normal"])
            .with_category("mode")
            .with_description("Enter insert mode at end of line"),
        KeybindingRegistration::new("I", "enter-insert-bol")
            .with_modes(&["editor:normal"])
            .with_category("mode")
            .with_description("Enter insert mode at start of line"),
        KeybindingRegistration::new("o", "open-line-below")
            .with_modes(&["editor:normal"])
            .with_category("mode")
            .with_description("Open line below and enter insert mode"),
        KeybindingRegistration::new("O", "open-line-above")
            .with_modes(&["editor:normal"])
            .with_category("mode")
            .with_description("Open line above and enter insert mode"),
        KeybindingRegistration::new("v", "enter-visual")
            .with_modes(&["editor:normal"])
            .with_category("mode")
            .with_description("Enter visual mode"),
        KeybindingRegistration::new("V", "enter-visual-line")
            .with_modes(&["editor:normal"])
            .with_category("mode")
            .with_description("Enter visual line mode"),
        KeybindingRegistration::new("<C-v>", "enter-visual-block")
            .with_modes(&["editor:normal"])
            .with_category("mode")
            .with_description("Enter visual block mode"),
        KeybindingRegistration::new(":", "enter-command")
            .with_modes(&["editor:normal"])
            .with_category("mode")
            .with_description("Enter command mode"),
        // ====================================================================
        // Operators (enter operator-pending mode)
        // ====================================================================
        KeybindingRegistration::new("d", "enter-delete-operator")
            .with_modes(&["editor:normal"])
            .with_category("operator")
            .with_description("Delete operator"),
        KeybindingRegistration::new("y", "enter-yank-operator")
            .with_modes(&["editor:normal"])
            .with_category("operator")
            .with_description("Yank operator"),
        KeybindingRegistration::new("c", "enter-change-operator")
            .with_modes(&["editor:normal"])
            .with_category("operator")
            .with_description("Change operator"),
        KeybindingRegistration::new(">", "enter-indent-operator")
            .with_modes(&["editor:normal"])
            .with_category("operator")
            .with_description("Indent operator"),
        KeybindingRegistration::new("<", "enter-dedent-operator")
            .with_modes(&["editor:normal"])
            .with_category("operator")
            .with_description("Dedent operator"),
        // ====================================================================
        // Line operators (immediate)
        // ====================================================================
        KeybindingRegistration::new("dd", "delete-line")
            .with_modes(&["editor:normal"])
            .with_category("operator")
            .with_description("Delete line"),
        KeybindingRegistration::new("yy", "yank-line")
            .with_modes(&["editor:normal"])
            .with_category("operator")
            .with_description("Yank line"),
        KeybindingRegistration::new("cc", "change-line")
            .with_modes(&["editor:normal"])
            .with_category("operator")
            .with_description("Change line"),
        KeybindingRegistration::new("Y", "yank-line")
            .with_modes(&["editor:normal"])
            .with_category("operator")
            .with_description("Yank line (alias)"),
        KeybindingRegistration::new("D", "delete-to-eol")
            .with_modes(&["editor:normal"])
            .with_category("operator")
            .with_description("Delete to end of line"),
        KeybindingRegistration::new("C", "change-to-eol")
            .with_modes(&["editor:normal"])
            .with_category("operator")
            .with_description("Change to end of line"),
        // ====================================================================
        // Single character operations
        // ====================================================================
        KeybindingRegistration::new("x", "delete-char")
            .with_modes(&["editor:normal"])
            .with_category("edit")
            .with_description("Delete character under cursor"),
        KeybindingRegistration::new("X", "delete-char-before")
            .with_modes(&["editor:normal"])
            .with_category("edit")
            .with_description("Delete character before cursor"),
        KeybindingRegistration::new("r", "replace-char")
            .with_modes(&["editor:normal"])
            .with_category("edit")
            .with_description("Replace character"),
        KeybindingRegistration::new("~", "toggle-case")
            .with_modes(&["editor:normal"])
            .with_category("edit")
            .with_description("Toggle case"),
        // ====================================================================
        // Undo/Redo
        // ====================================================================
        KeybindingRegistration::new("u", "undo")
            .with_modes(&["editor:normal"])
            .with_category("history")
            .with_description("Undo"),
        KeybindingRegistration::new("<C-r>", "redo")
            .with_modes(&["editor:normal"])
            .with_category("history")
            .with_description("Redo"),
        // ====================================================================
        // Paste
        // ====================================================================
        KeybindingRegistration::new("p", "paste-after")
            .with_modes(&["editor:normal"])
            .with_category("clipboard")
            .with_description("Paste after cursor"),
        KeybindingRegistration::new("P", "paste-before")
            .with_modes(&["editor:normal"])
            .with_category("clipboard")
            .with_description("Paste before cursor"),
        // ====================================================================
        // Scroll
        // ====================================================================
        KeybindingRegistration::new("<C-u>", "scroll-half-up")
            .with_modes(&["editor:normal"])
            .with_category("scroll")
            .with_description("Scroll half page up"),
        KeybindingRegistration::new("<C-d>", "scroll-half-down")
            .with_modes(&["editor:normal"])
            .with_category("scroll")
            .with_description("Scroll half page down"),
        KeybindingRegistration::new("<C-b>", "scroll-page-up")
            .with_modes(&["editor:normal"])
            .with_category("scroll")
            .with_description("Scroll page up"),
        KeybindingRegistration::new("<C-f>", "scroll-page-down")
            .with_modes(&["editor:normal"])
            .with_category("scroll")
            .with_description("Scroll page down"),
        KeybindingRegistration::new("zz", "scroll-center")
            .with_modes(&["editor:normal"])
            .with_category("scroll")
            .with_description("Center cursor line"),
        KeybindingRegistration::new("zt", "scroll-top")
            .with_modes(&["editor:normal"])
            .with_category("scroll")
            .with_description("Scroll cursor line to top"),
        KeybindingRegistration::new("zb", "scroll-bottom")
            .with_modes(&["editor:normal"])
            .with_category("scroll")
            .with_description("Scroll cursor line to bottom"),
        // ====================================================================
        // Search
        // ====================================================================
        KeybindingRegistration::new("/", "motions:search-forward")
            .with_modes(&["editor:normal"])
            .with_category("search")
            .with_description("Search forward"),
        KeybindingRegistration::new("?", "motions:search-backward")
            .with_modes(&["editor:normal"])
            .with_category("search")
            .with_description("Search backward"),
        KeybindingRegistration::new("n", "motions:search-next")
            .with_modes(&["editor:normal"])
            .with_category("search")
            .with_description("Next search result"),
        KeybindingRegistration::new("N", "motions:search-prev")
            .with_modes(&["editor:normal"])
            .with_category("search")
            .with_description("Previous search result"),
        KeybindingRegistration::new("*", "motions:search-word-forward")
            .with_modes(&["editor:normal"])
            .with_category("search")
            .with_description("Search word under cursor forward"),
        KeybindingRegistration::new("#", "motions:search-word-backward")
            .with_modes(&["editor:normal"])
            .with_category("search")
            .with_description("Search word under cursor backward"),
        // ====================================================================
        // Window
        // ====================================================================
        KeybindingRegistration::new("<C-w>", "enter-window-mode")
            .with_modes(&["editor:normal"])
            .with_category("window")
            .with_description("Enter window mode"),
        // ====================================================================
        // Marks
        // ====================================================================
        KeybindingRegistration::new("m", "set-mark")
            .with_modes(&["editor:normal"])
            .with_category("mark")
            .with_description("Set mark"),
        KeybindingRegistration::new("'", "goto-mark-line")
            .with_modes(&["editor:normal"])
            .with_category("mark")
            .with_description("Go to mark (line)"),
        KeybindingRegistration::new("`", "goto-mark-exact")
            .with_modes(&["editor:normal"])
            .with_category("mark")
            .with_description("Go to mark (exact position)"),
        // ====================================================================
        // Join
        // ====================================================================
        KeybindingRegistration::new("J", "join-lines")
            .with_modes(&["editor:normal"])
            .with_category("edit")
            .with_description("Join lines"),
    ]
}
