//! Command ID constants for the editor module.
//!
//! These constants enable compile-time verification of command IDs
//! referenced in keybindings. Import these when defining keybindings
//! instead of using string literals.

use reovim_kernel::api::v1::{CommandId, ModuleId};

/// Editor module ID.
pub const MODULE: ModuleId = ModuleId::new("editor");

// =============================================================================
// Cursor Movement
// =============================================================================

/// Move cursor up (k).
pub const CURSOR_UP: CommandId = CommandId::new(MODULE, "cursor-up");

/// Move cursor down (j).
pub const CURSOR_DOWN: CommandId = CommandId::new(MODULE, "cursor-down");

/// Move cursor left (h).
pub const CURSOR_LEFT: CommandId = CommandId::new(MODULE, "cursor-left");

/// Move cursor right (l).
pub const CURSOR_RIGHT: CommandId = CommandId::new(MODULE, "cursor-right");

/// Move cursor down by display line (gj).
pub const CURSOR_DISPLAY_DOWN: CommandId = CommandId::new(MODULE, "cursor-display-down");

/// Move cursor up by display line (gk).
pub const CURSOR_DISPLAY_UP: CommandId = CommandId::new(MODULE, "cursor-display-up");

// =============================================================================
// Insert Mode Edits
// =============================================================================

/// Insert newline.
pub const INSERT_NEWLINE: CommandId = CommandId::new(MODULE, "insert-newline");

/// Insert tab.
pub const INSERT_TAB: CommandId = CommandId::new(MODULE, "insert-tab");

/// Delete word before cursor (Ctrl-w in insert).
pub const DELETE_WORD_BEFORE: CommandId = CommandId::new(MODULE, "delete-word-before");

/// Delete to beginning of line (Ctrl-u in insert).
pub const DELETE_TO_BOL: CommandId = CommandId::new(MODULE, "delete-to-bol");

// =============================================================================
// Line Navigation (for insert mode Home/End)
// =============================================================================

/// Move to start of line (Home).
pub const LINE_START: CommandId = CommandId::new(MODULE, "line-start");

/// Move to end of line (End).
pub const LINE_END: CommandId = CommandId::new(MODULE, "line-end");

// =============================================================================
// Completion
// =============================================================================

/// Next completion item (Ctrl-n).
pub const COMPLETION_NEXT: CommandId = CommandId::new(MODULE, "completion-next");

/// Previous completion item (Ctrl-p).
pub const COMPLETION_PREV: CommandId = CommandId::new(MODULE, "completion-prev");

/// Trigger completion (Ctrl-Space).
pub const COMPLETION_TRIGGER: CommandId = CommandId::new(MODULE, "completion-trigger");

// =============================================================================
// Delete Operations
// =============================================================================

/// Delete character under cursor (x).
pub const DELETE_CHAR: CommandId = CommandId::new(MODULE, "delete-char");

/// Delete character before cursor (X).
pub const DELETE_CHAR_BEFORE: CommandId = CommandId::new(MODULE, "delete-char-before");

/// Delete line (dd).
pub const DELETE_LINE: CommandId = CommandId::new(MODULE, "delete-line");

/// Delete to end of line (D).
pub const DELETE_TO_EOL: CommandId = CommandId::new(MODULE, "delete-to-eol");

// =============================================================================
// Operators
// =============================================================================

/// Enter delete operator mode (d).
pub const ENTER_DELETE_OPERATOR: CommandId = CommandId::new(MODULE, "enter-delete-operator");

/// Enter yank operator mode (y).
pub const ENTER_YANK_OPERATOR: CommandId = CommandId::new(MODULE, "enter-yank-operator");

/// Enter change operator mode (c).
pub const ENTER_CHANGE_OPERATOR: CommandId = CommandId::new(MODULE, "enter-change-operator");

/// Enter indent operator mode (>).
pub const ENTER_INDENT_OPERATOR: CommandId = CommandId::new(MODULE, "enter-indent-operator");

/// Enter dedent operator mode (<).
pub const ENTER_DEDENT_OPERATOR: CommandId = CommandId::new(MODULE, "enter-dedent-operator");

// =============================================================================
// Yank/Paste
// =============================================================================

/// Yank line (yy, Y).
pub const YANK_LINE: CommandId = CommandId::new(MODULE, "yank-line");

/// Paste after cursor (p).
pub const PASTE_AFTER: CommandId = CommandId::new(MODULE, "paste-after");

/// Paste before cursor (P).
pub const PASTE_BEFORE: CommandId = CommandId::new(MODULE, "paste-before");

// =============================================================================
// Undo/Redo
// =============================================================================

/// Undo (u).
pub const UNDO: CommandId = CommandId::new(MODULE, "undo");

/// Redo (ctrl-r).
pub const REDO: CommandId = CommandId::new(MODULE, "redo");

// =============================================================================
// Replace/Repeat
// =============================================================================

/// Start replace character (r).
pub const REPLACE_CHAR_START: CommandId = CommandId::new(MODULE, "replace-char-start");

/// Repeat last change (.).
pub const REPEAT_DOT: CommandId = CommandId::new(MODULE, "repeat-dot");

/// Join lines (J).
pub const JOIN_LINES: CommandId = CommandId::new(MODULE, "join-lines");

// =============================================================================
// File Operations
// =============================================================================

/// Write buffer to file (:w).
pub const WRITE: CommandId = CommandId::new(MODULE, "write");

// =============================================================================
// Scroll Operations (not yet implemented)
// =============================================================================

/// Scroll half page up (Ctrl-u).
pub const SCROLL_HALF_UP: CommandId = CommandId::new(MODULE, "scroll-half-up");

/// Scroll half page down (Ctrl-d).
pub const SCROLL_HALF_DOWN: CommandId = CommandId::new(MODULE, "scroll-half-down");

/// Scroll page up (Ctrl-b).
pub const SCROLL_PAGE_UP: CommandId = CommandId::new(MODULE, "scroll-page-up");

/// Scroll page down (Ctrl-f).
pub const SCROLL_PAGE_DOWN: CommandId = CommandId::new(MODULE, "scroll-page-down");

/// Center cursor line (zz).
pub const SCROLL_CENTER: CommandId = CommandId::new(MODULE, "scroll-center");

/// Scroll cursor line to top (zt).
pub const SCROLL_TOP: CommandId = CommandId::new(MODULE, "scroll-top");

/// Scroll cursor line to bottom (zb).
pub const SCROLL_BOTTOM: CommandId = CommandId::new(MODULE, "scroll-bottom");

// =============================================================================
// Mark Operations (not yet implemented)
// =============================================================================

/// Set mark (m).
pub const SET_MARK: CommandId = CommandId::new(MODULE, "set-mark");

/// Go to mark line (').
pub const GOTO_MARK_LINE: CommandId = CommandId::new(MODULE, "goto-mark-line");

/// Go to mark exact position (backtick).
pub const GOTO_MARK_EXACT: CommandId = CommandId::new(MODULE, "goto-mark-exact");

// =============================================================================
// Case Operations (not yet implemented)
// =============================================================================

/// Replace character (r).
pub const REPLACE_CHAR: CommandId = CommandId::new(MODULE, "replace-char");

/// Toggle case (~).
pub const TOGGLE_CASE: CommandId = CommandId::new(MODULE, "toggle-case");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_id() {
        assert_eq!(MODULE.as_str(), "editor");
    }

    // Cursor movement IDs
    #[test]
    fn test_cursor_up_id() {
        assert_eq!(CURSOR_UP.module().as_str(), "editor");
        assert_eq!(CURSOR_UP.name(), "cursor-up");
    }

    #[test]
    fn test_cursor_down_id() {
        assert_eq!(CURSOR_DOWN.name(), "cursor-down");
    }

    #[test]
    fn test_cursor_left_id() {
        assert_eq!(CURSOR_LEFT.name(), "cursor-left");
    }

    #[test]
    fn test_cursor_right_id() {
        assert_eq!(CURSOR_RIGHT.name(), "cursor-right");
    }

    #[test]
    fn test_cursor_display_down_id() {
        assert_eq!(CURSOR_DISPLAY_DOWN.name(), "cursor-display-down");
    }

    #[test]
    fn test_cursor_display_up_id() {
        assert_eq!(CURSOR_DISPLAY_UP.name(), "cursor-display-up");
    }

    // Insert mode IDs
    #[test]
    fn test_insert_newline_id() {
        assert_eq!(INSERT_NEWLINE.name(), "insert-newline");
    }

    #[test]
    fn test_insert_tab_id() {
        assert_eq!(INSERT_TAB.name(), "insert-tab");
    }

    #[test]
    fn test_delete_word_before_id() {
        assert_eq!(DELETE_WORD_BEFORE.name(), "delete-word-before");
    }

    #[test]
    fn test_delete_to_bol_id() {
        assert_eq!(DELETE_TO_BOL.name(), "delete-to-bol");
    }

    // Line navigation IDs
    #[test]
    fn test_line_start_id() {
        assert_eq!(LINE_START.name(), "line-start");
    }

    #[test]
    fn test_line_end_id() {
        assert_eq!(LINE_END.name(), "line-end");
    }

    // Completion IDs
    #[test]
    fn test_completion_next_id() {
        assert_eq!(COMPLETION_NEXT.name(), "completion-next");
    }

    #[test]
    fn test_completion_prev_id() {
        assert_eq!(COMPLETION_PREV.name(), "completion-prev");
    }

    #[test]
    fn test_completion_trigger_id() {
        assert_eq!(COMPLETION_TRIGGER.name(), "completion-trigger");
    }

    // Delete IDs
    #[test]
    fn test_delete_char_id() {
        assert_eq!(DELETE_CHAR.name(), "delete-char");
    }

    #[test]
    fn test_delete_char_before_id() {
        assert_eq!(DELETE_CHAR_BEFORE.name(), "delete-char-before");
    }

    #[test]
    fn test_delete_line_id() {
        assert_eq!(DELETE_LINE.name(), "delete-line");
    }

    #[test]
    fn test_delete_to_eol_id() {
        assert_eq!(DELETE_TO_EOL.name(), "delete-to-eol");
    }

    // Operator IDs
    #[test]
    fn test_enter_delete_operator_id() {
        assert_eq!(ENTER_DELETE_OPERATOR.name(), "enter-delete-operator");
    }

    #[test]
    fn test_enter_yank_operator_id() {
        assert_eq!(ENTER_YANK_OPERATOR.name(), "enter-yank-operator");
    }

    #[test]
    fn test_enter_change_operator_id() {
        assert_eq!(ENTER_CHANGE_OPERATOR.name(), "enter-change-operator");
    }

    #[test]
    fn test_enter_indent_operator_id() {
        assert_eq!(ENTER_INDENT_OPERATOR.name(), "enter-indent-operator");
    }

    #[test]
    fn test_enter_dedent_operator_id() {
        assert_eq!(ENTER_DEDENT_OPERATOR.name(), "enter-dedent-operator");
    }

    // Yank/Paste IDs
    #[test]
    fn test_yank_line_id() {
        assert_eq!(YANK_LINE.name(), "yank-line");
    }

    #[test]
    fn test_paste_after_id() {
        assert_eq!(PASTE_AFTER.name(), "paste-after");
    }

    #[test]
    fn test_paste_before_id() {
        assert_eq!(PASTE_BEFORE.name(), "paste-before");
    }

    // Undo/Redo IDs
    #[test]
    fn test_undo_id() {
        assert_eq!(UNDO.name(), "undo");
    }

    #[test]
    fn test_redo_id() {
        assert_eq!(REDO.name(), "redo");
    }

    // Replace/Repeat IDs
    #[test]
    fn test_replace_char_start_id() {
        assert_eq!(REPLACE_CHAR_START.name(), "replace-char-start");
    }

    #[test]
    fn test_repeat_dot_id() {
        assert_eq!(REPEAT_DOT.name(), "repeat-dot");
    }

    #[test]
    fn test_join_lines_id() {
        assert_eq!(JOIN_LINES.name(), "join-lines");
    }

    // File IDs
    #[test]
    fn test_write_id() {
        assert_eq!(WRITE.name(), "write");
    }

    // Scroll IDs
    #[test]
    fn test_scroll_half_up_id() {
        assert_eq!(SCROLL_HALF_UP.name(), "scroll-half-up");
    }

    #[test]
    fn test_scroll_half_down_id() {
        assert_eq!(SCROLL_HALF_DOWN.name(), "scroll-half-down");
    }

    #[test]
    fn test_scroll_page_up_id() {
        assert_eq!(SCROLL_PAGE_UP.name(), "scroll-page-up");
    }

    #[test]
    fn test_scroll_page_down_id() {
        assert_eq!(SCROLL_PAGE_DOWN.name(), "scroll-page-down");
    }

    #[test]
    fn test_scroll_center_id() {
        assert_eq!(SCROLL_CENTER.name(), "scroll-center");
    }

    #[test]
    fn test_scroll_top_id() {
        assert_eq!(SCROLL_TOP.name(), "scroll-top");
    }

    #[test]
    fn test_scroll_bottom_id() {
        assert_eq!(SCROLL_BOTTOM.name(), "scroll-bottom");
    }

    // Mark IDs
    #[test]
    fn test_set_mark_id() {
        assert_eq!(SET_MARK.name(), "set-mark");
    }

    #[test]
    fn test_goto_mark_line_id() {
        assert_eq!(GOTO_MARK_LINE.name(), "goto-mark-line");
    }

    #[test]
    fn test_goto_mark_exact_id() {
        assert_eq!(GOTO_MARK_EXACT.name(), "goto-mark-exact");
    }

    // Case IDs
    #[test]
    fn test_replace_char_id() {
        assert_eq!(REPLACE_CHAR.name(), "replace-char");
    }

    #[test]
    fn test_toggle_case_id() {
        assert_eq!(TOGGLE_CASE.name(), "toggle-case");
    }

    #[test]
    fn test_all_ids_belong_to_editor_module() {
        let ids = [
            CURSOR_UP,
            CURSOR_DOWN,
            CURSOR_LEFT,
            CURSOR_RIGHT,
            CURSOR_DISPLAY_DOWN,
            CURSOR_DISPLAY_UP,
            INSERT_NEWLINE,
            INSERT_TAB,
            DELETE_WORD_BEFORE,
            DELETE_TO_BOL,
            LINE_START,
            LINE_END,
            COMPLETION_NEXT,
            COMPLETION_PREV,
            COMPLETION_TRIGGER,
            DELETE_CHAR,
            DELETE_CHAR_BEFORE,
            DELETE_LINE,
            DELETE_TO_EOL,
            ENTER_DELETE_OPERATOR,
            ENTER_YANK_OPERATOR,
            ENTER_CHANGE_OPERATOR,
            ENTER_INDENT_OPERATOR,
            ENTER_DEDENT_OPERATOR,
            YANK_LINE,
            PASTE_AFTER,
            PASTE_BEFORE,
            UNDO,
            REDO,
            REPLACE_CHAR_START,
            REPEAT_DOT,
            JOIN_LINES,
            WRITE,
            SCROLL_HALF_UP,
            SCROLL_HALF_DOWN,
            SCROLL_PAGE_UP,
            SCROLL_PAGE_DOWN,
            SCROLL_CENTER,
            SCROLL_TOP,
            SCROLL_BOTTOM,
            SET_MARK,
            GOTO_MARK_LINE,
            GOTO_MARK_EXACT,
            REPLACE_CHAR,
            TOGGLE_CASE,
        ];
        for id in &ids {
            assert_eq!(id.module().as_str(), "editor", "ID {id:?} has wrong module");
        }
    }
}
