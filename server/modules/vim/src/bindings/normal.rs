//! Normal mode keybindings.
//!
//! Reference: lib/core/src/bind/mod.rs (concept-extraction, not migration)

use {
    reovim_kernel::api::v1::KeybindingRegistration, reovim_module_editor::ids as editor,
    reovim_module_motions::ids as motions, reovim_module_window_ops::ids as window_ops,
};

use crate::ids as vim;

/// Normal mode keybindings.
///
/// These are POLICY decisions - which keys trigger which commands.
/// The kernel provides the mechanisms (motion calculation, buffer ops).
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn bindings() -> Vec<KeybindingRegistration> {
    vec![
        // ====================================================================
        // Movement - hjkl
        // ====================================================================
        KeybindingRegistration::new("h", editor::CURSOR_LEFT)
            .with_modes(&["vim:normal"])
            .with_category("motion")
            .with_description("Move cursor left"),
        KeybindingRegistration::new("j", editor::CURSOR_DOWN)
            .with_modes(&["vim:normal"])
            .with_category("motion")
            .with_description("Move cursor down"),
        KeybindingRegistration::new("k", editor::CURSOR_UP)
            .with_modes(&["vim:normal"])
            .with_category("motion")
            .with_description("Move cursor up"),
        KeybindingRegistration::new("l", editor::CURSOR_RIGHT)
            .with_modes(&["vim:normal"])
            .with_category("motion")
            .with_description("Move cursor right"),
        // ====================================================================
        // Display line movement (gj/gk)
        // ====================================================================
        KeybindingRegistration::new("gj", editor::CURSOR_DISPLAY_DOWN)
            .with_modes(&["vim:normal"])
            .with_category("motion")
            .with_description("Move cursor down one display line"),
        KeybindingRegistration::new("gk", editor::CURSOR_DISPLAY_UP)
            .with_modes(&["vim:normal"])
            .with_category("motion")
            .with_description("Move cursor up one display line"),
        // ====================================================================
        // Word motions (provided by motions module)
        // ====================================================================
        KeybindingRegistration::new("w", motions::WORD_FORWARD)
            .with_modes(&["vim:normal"])
            .with_category("motion")
            .with_description("Move to next word"),
        KeybindingRegistration::new("b", motions::WORD_BACKWARD)
            .with_modes(&["vim:normal"])
            .with_category("motion")
            .with_description("Move to previous word"),
        KeybindingRegistration::new("e", motions::WORD_END)
            .with_modes(&["vim:normal"])
            .with_category("motion")
            .with_description("Move to end of word"),
        KeybindingRegistration::new("W", motions::WORD_FORWARD_BIG)
            .with_modes(&["vim:normal"])
            .with_category("motion")
            .with_description("Move to next WORD"),
        KeybindingRegistration::new("B", motions::WORD_BACKWARD_BIG)
            .with_modes(&["vim:normal"])
            .with_category("motion")
            .with_description("Move to previous WORD"),
        KeybindingRegistration::new("E", motions::WORD_END_BIG)
            .with_modes(&["vim:normal"])
            .with_category("motion")
            .with_description("Move to end of WORD"),
        KeybindingRegistration::new("ge", motions::WORD_END_BACKWARD)
            .with_modes(&["vim:normal"])
            .with_category("motion")
            .with_description("Move to end of previous word"),
        KeybindingRegistration::new("gE", motions::WORD_END_BACKWARD_BIG)
            .with_modes(&["vim:normal"])
            .with_category("motion")
            .with_description("Move to end of previous WORD"),
        // ====================================================================
        // Line motions (provided by motions module)
        // ====================================================================
        KeybindingRegistration::new("0", motions::LINE_START)
            .with_modes(&["vim:normal"])
            .with_category("motion")
            .with_description("Move to start of line"),
        KeybindingRegistration::new("$", motions::LINE_END)
            .with_modes(&["vim:normal"])
            .with_category("motion")
            .with_description("Move to end of line"),
        KeybindingRegistration::new("^", motions::FIRST_NON_BLANK)
            .with_modes(&["vim:normal"])
            .with_category("motion")
            .with_description("Move to first non-blank character"),
        // ====================================================================
        // Document motions (provided by motions module)
        // ====================================================================
        KeybindingRegistration::new("gg", motions::DOCUMENT_START)
            .with_modes(&["vim:normal"])
            .with_category("motion")
            .with_description("Go to start of document"),
        KeybindingRegistration::new("G", motions::DOCUMENT_END)
            .with_modes(&["vim:normal"])
            .with_category("motion")
            .with_description("Go to end of document"),
        // Screen position motions (#669)
        KeybindingRegistration::new("H", motions::SCREEN_HIGH)
            .with_modes(&["vim:normal"])
            .with_category("motion")
            .with_description("Move to top of screen"),
        KeybindingRegistration::new("M", motions::SCREEN_MIDDLE)
            .with_modes(&["vim:normal"])
            .with_category("motion")
            .with_description("Move to middle of screen"),
        KeybindingRegistration::new("L", motions::SCREEN_LOW)
            .with_modes(&["vim:normal"])
            .with_category("motion")
            .with_description("Move to bottom of screen"),
        // ====================================================================
        // Mode switching
        // ====================================================================
        KeybindingRegistration::new("i", vim::ENTER_INSERT)
            .with_modes(&["vim:normal"])
            .with_category("mode")
            .with_description("Enter insert mode"),
        KeybindingRegistration::new("a", vim::ENTER_INSERT_AFTER)
            .with_modes(&["vim:normal"])
            .with_category("mode")
            .with_description("Enter insert mode after cursor"),
        KeybindingRegistration::new("A", vim::ENTER_INSERT_EOL)
            .with_modes(&["vim:normal"])
            .with_category("mode")
            .with_description("Enter insert mode at end of line"),
        KeybindingRegistration::new("I", vim::ENTER_INSERT_BOL)
            .with_modes(&["vim:normal"])
            .with_category("mode")
            .with_description("Enter insert mode at start of line"),
        KeybindingRegistration::new("o", vim::OPEN_LINE_BELOW)
            .with_modes(&["vim:normal"])
            .with_category("mode")
            .with_description("Open line below and enter insert mode"),
        KeybindingRegistration::new("O", vim::OPEN_LINE_ABOVE)
            .with_modes(&["vim:normal"])
            .with_category("mode")
            .with_description("Open line above and enter insert mode"),
        KeybindingRegistration::new("v", vim::ENTER_VISUAL)
            .with_modes(&["vim:normal"])
            .with_category("mode")
            .with_description("Enter visual mode"),
        KeybindingRegistration::new("V", vim::ENTER_VISUAL_LINE)
            .with_modes(&["vim:normal"])
            .with_category("mode")
            .with_description("Enter visual line mode"),
        KeybindingRegistration::new("<C-v>", vim::ENTER_VISUAL_BLOCK)
            .with_modes(&["vim:normal"])
            .with_category("mode")
            .with_description("Enter visual block mode"),
        KeybindingRegistration::new(":", vim::ENTER_COMMANDLINE)
            .with_modes(&["vim:normal"])
            .with_category("mode")
            .with_description("Enter command-line mode"),
        // ====================================================================
        // Operators (enter operator-pending mode)
        // ====================================================================
        KeybindingRegistration::new("d", editor::ENTER_DELETE_OPERATOR)
            .with_modes(&["vim:normal"])
            .with_category("operator")
            .with_description("Delete operator"),
        KeybindingRegistration::new("y", editor::ENTER_YANK_OPERATOR)
            .with_modes(&["vim:normal"])
            .with_category("operator")
            .with_description("Yank operator"),
        KeybindingRegistration::new("c", editor::ENTER_CHANGE_OPERATOR)
            .with_modes(&["vim:normal"])
            .with_category("operator")
            .with_description("Change operator"),
        KeybindingRegistration::new("<gt>", editor::ENTER_INDENT_OPERATOR)
            .with_modes(&["vim:normal"])
            .with_category("operator")
            .with_description("Indent operator"),
        KeybindingRegistration::new("<lt>", editor::ENTER_DEDENT_OPERATOR)
            .with_modes(&["vim:normal"])
            .with_category("operator")
            .with_description("Dedent operator"),
        // ====================================================================
        // Line operators (immediate)
        // ====================================================================
        KeybindingRegistration::new("dd", editor::DELETE_LINE)
            .with_modes(&["vim:normal"])
            .with_category("operator")
            .with_description("Delete line"),
        KeybindingRegistration::new("yy", editor::YANK_LINE)
            .with_modes(&["vim:normal"])
            .with_category("operator")
            .with_description("Yank line"),
        KeybindingRegistration::new("cc", vim::CHANGE_LINE)
            .with_modes(&["vim:normal"])
            .with_category("operator")
            .with_description("Change line"),
        KeybindingRegistration::new("Y", editor::YANK_LINE)
            .with_modes(&["vim:normal"])
            .with_category("operator")
            .with_description("Yank line (alias)"),
        KeybindingRegistration::new("D", editor::DELETE_TO_EOL)
            .with_modes(&["vim:normal"])
            .with_category("operator")
            .with_description("Delete to end of line"),
        KeybindingRegistration::new("C", vim::CHANGE_TO_EOL)
            .with_modes(&["vim:normal"])
            .with_category("operator")
            .with_description("Change to end of line"),
        // ====================================================================
        // Single character operations
        // ====================================================================
        KeybindingRegistration::new("x", editor::DELETE_CHAR)
            .with_modes(&["vim:normal"])
            .with_category("edit")
            .with_description("Delete character under cursor"),
        KeybindingRegistration::new("X", editor::DELETE_CHAR_BEFORE)
            .with_modes(&["vim:normal"])
            .with_category("edit")
            .with_description("Delete character before cursor"),
        KeybindingRegistration::new("r", editor::REPLACE_CHAR_START)
            .with_modes(&["vim:normal"])
            .with_category("edit")
            .with_description("Replace character"),
        KeybindingRegistration::new("~", editor::TOGGLE_CASE)
            .with_modes(&["vim:normal"])
            .with_category("edit")
            .with_description("Toggle case"),
        KeybindingRegistration::new(".", vim::DOT_REPEAT)
            .with_modes(&["vim:normal"])
            .with_category("edit")
            .with_description("Repeat last change"),
        // ====================================================================
        // Undo/Redo
        // ====================================================================
        KeybindingRegistration::new("u", editor::UNDO)
            .with_modes(&["vim:normal"])
            .with_category("history")
            .with_description("Undo"),
        KeybindingRegistration::new("<C-r>", editor::REDO)
            .with_modes(&["vim:normal"])
            .with_category("history")
            .with_description("Redo"),
        // ====================================================================
        // Paste
        // ====================================================================
        KeybindingRegistration::new("p", editor::PASTE_AFTER)
            .with_modes(&["vim:normal"])
            .with_category("clipboard")
            .with_description("Paste after cursor"),
        KeybindingRegistration::new("P", editor::PASTE_BEFORE)
            .with_modes(&["vim:normal"])
            .with_category("clipboard")
            .with_description("Paste before cursor"),
        // ====================================================================
        // Scroll
        // ====================================================================
        KeybindingRegistration::new("<C-u>", editor::SCROLL_HALF_UP)
            .with_modes(&["vim:normal"])
            .with_category("scroll")
            .with_description("Scroll half page up"),
        KeybindingRegistration::new("<C-d>", editor::SCROLL_HALF_DOWN)
            .with_modes(&["vim:normal"])
            .with_category("scroll")
            .with_description("Scroll half page down"),
        KeybindingRegistration::new("<C-b>", editor::SCROLL_PAGE_UP)
            .with_modes(&["vim:normal"])
            .with_category("scroll")
            .with_description("Scroll page up"),
        KeybindingRegistration::new("<C-f>", editor::SCROLL_PAGE_DOWN)
            .with_modes(&["vim:normal"])
            .with_category("scroll")
            .with_description("Scroll page down"),
        KeybindingRegistration::new("zz", editor::SCROLL_CENTER)
            .with_modes(&["vim:normal"])
            .with_category("scroll")
            .with_description("Center cursor line"),
        KeybindingRegistration::new("zt", editor::SCROLL_TOP)
            .with_modes(&["vim:normal"])
            .with_category("scroll")
            .with_description("Scroll cursor line to top"),
        KeybindingRegistration::new("zb", editor::SCROLL_BOTTOM)
            .with_modes(&["vim:normal"])
            .with_category("scroll")
            .with_description("Scroll cursor line to bottom"),
        // ====================================================================
        // Search (#435 - Search Input Mode)
        // ====================================================================
        KeybindingRegistration::new("/", vim::ENTER_SEARCH_FORWARD)
            .with_modes(&["vim:normal"])
            .with_category("search")
            .with_description("Search forward"),
        KeybindingRegistration::new("?", vim::ENTER_SEARCH_BACKWARD)
            .with_modes(&["vim:normal"])
            .with_category("search")
            .with_description("Search backward"),
        KeybindingRegistration::new("n", motions::SEARCH_NEXT)
            .with_modes(&["vim:normal"])
            .with_category("search")
            .with_description("Next search result"),
        KeybindingRegistration::new("N", motions::SEARCH_PREV)
            .with_modes(&["vim:normal"])
            .with_category("search")
            .with_description("Previous search result"),
        KeybindingRegistration::new("*", motions::SEARCH_WORD_FORWARD)
            .with_modes(&["vim:normal"])
            .with_category("search")
            .with_description("Search word under cursor forward"),
        KeybindingRegistration::new("#", motions::SEARCH_WORD_BACKWARD)
            .with_modes(&["vim:normal"])
            .with_category("search")
            .with_description("Search word under cursor backward"),
        // ====================================================================
        // Find/till character motions (#554)
        // ====================================================================
        KeybindingRegistration::new("f", motions::FIND_CHAR_FORWARD)
            .with_modes(&["vim:normal"])
            .with_category("motion")
            .with_description("Find character forward"),
        KeybindingRegistration::new("F", motions::FIND_CHAR_BACKWARD)
            .with_modes(&["vim:normal"])
            .with_category("motion")
            .with_description("Find character backward"),
        KeybindingRegistration::new("t", motions::TILL_CHAR_FORWARD)
            .with_modes(&["vim:normal"])
            .with_category("motion")
            .with_description("Till character forward"),
        KeybindingRegistration::new("T", motions::TILL_CHAR_BACKWARD)
            .with_modes(&["vim:normal"])
            .with_category("motion")
            .with_description("Till character backward"),
        KeybindingRegistration::new(";", motions::REPEAT_FIND_SAME)
            .with_modes(&["vim:normal"])
            .with_category("motion")
            .with_description("Repeat find same direction"),
        KeybindingRegistration::new(",", motions::REPEAT_FIND_REVERSE)
            .with_modes(&["vim:normal"])
            .with_category("motion")
            .with_description("Repeat find reverse direction"),
        // ====================================================================
        // Window
        // ====================================================================
        KeybindingRegistration::new("<C-w>", vim::ENTER_WINDOW_MODE)
            .with_modes(&["vim:normal"])
            .with_category("window")
            .with_description("Enter window mode"),
        // ====================================================================
        // Tabs (#401)
        // ====================================================================
        KeybindingRegistration::new("gt", window_ops::TAB_NEXT)
            .with_modes(&["vim:normal"])
            .with_category("tab")
            .with_description("Go to next tab"),
        KeybindingRegistration::new("gT", window_ops::TAB_PREV)
            .with_modes(&["vim:normal"])
            .with_category("tab")
            .with_description("Go to previous tab"),
        // ====================================================================
        // Marks
        // ====================================================================
        KeybindingRegistration::new("m", editor::SET_MARK)
            .with_modes(&["vim:normal"])
            .with_category("mark")
            .with_description("Set mark"),
        KeybindingRegistration::new("'", editor::GOTO_MARK_LINE)
            .with_modes(&["vim:normal"])
            .with_category("mark")
            .with_description("Go to mark (line)"),
        KeybindingRegistration::new("`", editor::GOTO_MARK_EXACT)
            .with_modes(&["vim:normal"])
            .with_category("mark")
            .with_description("Go to mark (exact position)"),
        // ====================================================================
        // Jump List
        // ====================================================================
        KeybindingRegistration::new("<C-o>", editor::JUMP_BACKWARD)
            .with_modes(&["vim:normal"])
            .with_category("jump")
            .with_description("Jump to older position"),
        KeybindingRegistration::new("<C-i>", editor::JUMP_FORWARD)
            .with_modes(&["vim:normal"])
            .with_category("jump")
            .with_description("Jump to newer position"),
        // ====================================================================
        // Join
        // ====================================================================
        KeybindingRegistration::new("J", editor::JOIN_LINES)
            .with_modes(&["vim:normal"])
            .with_category("edit")
            .with_description("Join lines"),
        // ====================================================================
        // Session Management (tmux-like, issue #350)
        // ====================================================================
        // Note: <C-b> prefix conflicts with scroll-page-up, but multi-key
        // sequences take precedence over single-key bindings.
        KeybindingRegistration::new("<C-b>d", vim::SESSION_DETACH)
            .with_modes(&["vim:normal"])
            .with_category("session")
            .with_description("Detach from server (server continues)"),
        KeybindingRegistration::new("<C-b>s", vim::SESSION_SERVERS)
            .with_modes(&["vim:normal"])
            .with_category("session")
            .with_description("List running server instances"),
        KeybindingRegistration::new("<C-b>&", vim::SESSION_KILL_SERVER)
            .with_modes(&["vim:normal"])
            .with_category("session")
            .with_description("Kill the current server"),
    ]
}
