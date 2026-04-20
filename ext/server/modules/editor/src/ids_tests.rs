use crate::ids::*;

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
