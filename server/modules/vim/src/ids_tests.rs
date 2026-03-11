use crate::ids::*;

// ========================================================================
// OperatorId tests
// ========================================================================

#[test]
fn test_operator_id_constants() {
    assert_eq!(DELETE.name(), "delete");
    assert_eq!(YANK.name(), "yank");
    assert_eq!(CHANGE.name(), "change");
}

#[test]
fn test_operator_id_module() {
    assert_eq!(DELETE.module().as_str(), "vim");
    assert_eq!(YANK.module().as_str(), "vim");
    assert_eq!(CHANGE.module().as_str(), "vim");
}

#[test]
fn test_operator_id_display() {
    assert_eq!(format!("{DELETE}"), "vim:delete");
    assert_eq!(format!("{YANK}"), "vim:yank");
    assert_eq!(format!("{CHANGE}"), "vim:change");
}

#[test]
fn test_operator_id_equality() {
    let delete2 = OperatorId::new(MODULE, "delete");
    assert_eq!(DELETE, delete2);
    assert_ne!(DELETE, YANK);
}

#[test]
fn test_operator_id_from_qualified_leaked() {
    let op = OperatorId::from_qualified_leaked("vim:delete".to_string());
    assert_eq!(op.module().as_str(), "vim");
    assert_eq!(op.name(), "delete");
}

#[test]
fn test_operator_id_from_qualified_leaked_no_colon() {
    let op = OperatorId::from_qualified_leaked("delete".to_string());
    assert_eq!(op.module().as_str(), "unknown");
    assert_eq!(op.name(), "delete");
}

#[test]
fn test_operator_id_hash() {
    use std::collections::HashSet;

    let op1 = OperatorId::new(MODULE, "delete");
    let op2 = OperatorId::new(MODULE, "delete");
    let op3 = OperatorId::new(MODULE, "yank");

    let mut set = HashSet::new();
    set.insert(op1);
    assert!(set.contains(&op2)); // Same name
    assert!(!set.contains(&op3)); // Different name
}

// ========================================================================
// CommandId tests
// ========================================================================

#[test]
fn test_command_id_constants() {
    assert_eq!(ENTER_INSERT.name(), "enter-insert");
    assert_eq!(EXIT_INSERT.name(), "exit-insert");
}

#[test]
fn test_command_id_module() {
    assert_eq!(ENTER_INSERT.module().as_str(), "vim");
}

// ========================================================================
// Insert mode command IDs
// ========================================================================

#[test]
fn test_insert_mode_command_ids() {
    assert_eq!(ENTER_INSERT.name(), "enter-insert");
    assert_eq!(ENTER_INSERT_AFTER.name(), "enter-insert-after");
    assert_eq!(ENTER_INSERT_EOL.name(), "enter-insert-eol");
    assert_eq!(ENTER_INSERT_BOL.name(), "enter-insert-bol");
    assert_eq!(OPEN_LINE_BELOW.name(), "open-line-below");
    assert_eq!(OPEN_LINE_ABOVE.name(), "open-line-above");
    assert_eq!(EXIT_INSERT.name(), "exit-insert");
}

#[test]
fn test_insert_mode_command_ids_module() {
    assert_eq!(ENTER_INSERT_AFTER.module().as_str(), "vim");
    assert_eq!(ENTER_INSERT_EOL.module().as_str(), "vim");
    assert_eq!(ENTER_INSERT_BOL.module().as_str(), "vim");
    assert_eq!(OPEN_LINE_BELOW.module().as_str(), "vim");
    assert_eq!(OPEN_LINE_ABOVE.module().as_str(), "vim");
}

// ========================================================================
// Visual mode command IDs
// ========================================================================

#[test]
fn test_visual_mode_command_ids() {
    assert_eq!(ENTER_VISUAL.name(), "enter-visual");
    assert_eq!(ENTER_VISUAL_LINE.name(), "enter-visual-line");
    assert_eq!(ENTER_VISUAL_BLOCK.name(), "enter-visual-block");
    assert_eq!(EXIT_VISUAL.name(), "exit-visual");
}

#[test]
fn test_visual_mode_command_ids_module() {
    assert_eq!(ENTER_VISUAL.module().as_str(), "vim");
    assert_eq!(ENTER_VISUAL_LINE.module().as_str(), "vim");
    assert_eq!(ENTER_VISUAL_BLOCK.module().as_str(), "vim");
    assert_eq!(EXIT_VISUAL.module().as_str(), "vim");
}

// ========================================================================
// Other mode command IDs
// ========================================================================

#[test]
fn test_other_mode_command_ids() {
    assert_eq!(ENTER_COMMANDLINE.name(), "enter-commandline");
    assert_eq!(CANCEL_COMMANDLINE.name(), "cancel-commandline");
    assert_eq!(EXIT_COMMANDLINE.name(), "exit-commandline");
    assert_eq!(ENTER_WINDOW_MODE.name(), "enter-window-mode");
    assert_eq!(CANCEL_TO_NORMAL.name(), "cancel-to-normal");
}

#[test]
fn test_other_mode_command_ids_module() {
    assert_eq!(ENTER_COMMANDLINE.module().as_str(), "vim");
    assert_eq!(CANCEL_COMMANDLINE.module().as_str(), "vim");
    assert_eq!(EXIT_COMMANDLINE.module().as_str(), "vim");
    assert_eq!(ENTER_WINDOW_MODE.module().as_str(), "vim");
    assert_eq!(CANCEL_TO_NORMAL.module().as_str(), "vim");
}

// ========================================================================
// Search command IDs
// ========================================================================

#[test]
fn test_search_command_ids() {
    assert_eq!(ENTER_SEARCH_FORWARD.name(), "enter-search-forward");
    assert_eq!(ENTER_SEARCH_BACKWARD.name(), "enter-search-backward");
}

#[test]
fn test_search_command_ids_module() {
    assert_eq!(ENTER_SEARCH_FORWARD.module().as_str(), "vim");
    assert_eq!(ENTER_SEARCH_BACKWARD.module().as_str(), "vim");
}

// ========================================================================
// Find-char command ID
// ========================================================================

#[test]
fn test_find_char_command_id() {
    assert_eq!(EXECUTE_FIND_CHAR.name(), "execute-find-char");
    assert_eq!(EXECUTE_FIND_CHAR.module().as_str(), "vim");
}

// ========================================================================
// Visual manipulation command IDs
// ========================================================================

#[test]
fn test_visual_manipulation_command_ids() {
    assert_eq!(VISUAL_SWAP_ANCHOR.name(), "visual-swap-anchor");
    assert_eq!(TOGGLE_VISUAL_CHAR.name(), "toggle-visual-char");
    assert_eq!(TOGGLE_VISUAL_LINE.name(), "toggle-visual-line");
    assert_eq!(TOGGLE_VISUAL_BLOCK.name(), "toggle-visual-block");
    assert_eq!(RESELECT_LAST.name(), "reselect-last");
}

// ========================================================================
// Visual operator command IDs
// ========================================================================

#[test]
fn test_visual_operator_command_ids() {
    assert_eq!(DELETE_SELECTION.name(), "delete-selection");
    assert_eq!(YANK_SELECTION.name(), "yank-selection");
    assert_eq!(CHANGE_SELECTION.name(), "change-selection");
    assert_eq!(INDENT_SELECTION.name(), "indent-selection");
    assert_eq!(DEDENT_SELECTION.name(), "dedent-selection");
}

// ========================================================================
// Change operation command IDs
// ========================================================================

#[test]
fn test_change_operation_command_ids() {
    assert_eq!(CHANGE_LINE.name(), "change-line");
    assert_eq!(CHANGE_TO_EOL.name(), "change-to-eol");
}

#[test]
fn test_change_operation_command_ids_module() {
    assert_eq!(CHANGE_LINE.module().as_str(), "vim");
    assert_eq!(CHANGE_TO_EOL.module().as_str(), "vim");
}

// ========================================================================
// Dot repeat and macro command IDs
// ========================================================================

#[test]
fn test_dot_repeat_command_id() {
    assert_eq!(DOT_REPEAT.name(), "dot-repeat");
    assert_eq!(DOT_REPEAT.module().as_str(), "vim");
}

#[test]
fn test_macro_command_ids() {
    assert_eq!(PLAY_MACRO.name(), "play-macro");
    assert_eq!(REPEAT_MACRO.name(), "repeat-macro");
    assert_eq!(PLAY_MACRO.module().as_str(), "vim");
    assert_eq!(REPEAT_MACRO.module().as_str(), "vim");
}

// ========================================================================
// Session management command IDs
// ========================================================================

#[test]
fn test_session_command_ids() {
    assert_eq!(SESSION_DETACH.name(), "session-detach");
    assert_eq!(SESSION_SERVERS.name(), "session-servers");
    assert_eq!(SESSION_KILL_SERVER.name(), "session-kill-server");
}

// ========================================================================
// Visual additional operation command IDs
// ========================================================================

#[test]
fn test_visual_additional_command_ids() {
    assert_eq!(VISUAL_SWAP_CORNER.name(), "visual-swap-corner");
    assert_eq!(TOGGLE_CASE_SELECTION.name(), "toggle-case-selection");
    assert_eq!(LOWERCASE_SELECTION.name(), "lowercase-selection");
    assert_eq!(UPPERCASE_SELECTION.name(), "uppercase-selection");
    assert_eq!(JOIN_SELECTION.name(), "join-selection");
    assert_eq!(COMMAND_WITH_SELECTION.name(), "command-with-selection");
    assert_eq!(VISUAL_NOOP.name(), "visual-noop");
    assert_eq!(VISUAL_INSERT_START.name(), "visual-insert-start");
    assert_eq!(VISUAL_INSERT_END.name(), "visual-insert-end");
    assert_eq!(BLOCK_INSERT_START.name(), "block-insert-start");
    assert_eq!(BLOCK_INSERT_END.name(), "block-insert-end");
}

// ========================================================================
// Text object command IDs
// ========================================================================

#[test]
fn test_inner_text_object_command_ids() {
    assert_eq!(INNER_WORD.name(), "inner-word");
    assert_eq!(INNER_WORD_BIG.name(), "inner-word-big");
    assert_eq!(INNER_DOUBLE_QUOTE.name(), "inner-double-quote");
    assert_eq!(INNER_SINGLE_QUOTE.name(), "inner-single-quote");
    assert_eq!(INNER_BACKTICK.name(), "inner-backtick");
    assert_eq!(INNER_PAREN.name(), "inner-paren");
    assert_eq!(INNER_BRACKET.name(), "inner-bracket");
    assert_eq!(INNER_BRACE.name(), "inner-brace");
    assert_eq!(INNER_ANGLE.name(), "inner-angle");
    assert_eq!(INNER_TAG.name(), "inner-tag");
    assert_eq!(INNER_SENTENCE.name(), "inner-sentence");
    assert_eq!(INNER_PARAGRAPH.name(), "inner-paragraph");
}

#[test]
fn test_around_text_object_command_ids() {
    assert_eq!(AROUND_WORD.name(), "around-word");
    assert_eq!(AROUND_WORD_BIG.name(), "around-word-big");
    assert_eq!(AROUND_DOUBLE_QUOTE.name(), "around-double-quote");
    assert_eq!(AROUND_SINGLE_QUOTE.name(), "around-single-quote");
    assert_eq!(AROUND_BACKTICK.name(), "around-backtick");
    assert_eq!(AROUND_PAREN.name(), "around-paren");
    assert_eq!(AROUND_BRACKET.name(), "around-bracket");
    assert_eq!(AROUND_BRACE.name(), "around-brace");
    assert_eq!(AROUND_ANGLE.name(), "around-angle");
    assert_eq!(AROUND_TAG.name(), "around-tag");
    assert_eq!(AROUND_SENTENCE.name(), "around-sentence");
    assert_eq!(AROUND_PARAGRAPH.name(), "around-paragraph");
}

#[test]
fn test_text_object_command_ids_all_vim_module() {
    assert_eq!(INNER_WORD.module().as_str(), "vim");
    assert_eq!(AROUND_WORD.module().as_str(), "vim");
    assert_eq!(INNER_PAREN.module().as_str(), "vim");
    assert_eq!(AROUND_PAREN.module().as_str(), "vim");
}

// ========================================================================
// Module ID test
// ========================================================================

#[test]
fn test_module_constant() {
    assert_eq!(MODULE.as_str(), "vim");
}

// ========================================================================
// OperatorId additional tests
// ========================================================================

#[test]
fn test_operator_id_debug() {
    let debug_str = format!("{DELETE:?}");
    assert!(debug_str.contains("OperatorId"));
    assert!(debug_str.contains("delete"));
}

#[test]
#[allow(clippy::redundant_clone)]
fn test_operator_id_clone() {
    let cloned = DELETE.clone();
    assert_eq!(cloned, DELETE);
}

// ========================================================================
// Additional OperatorId tests
// ========================================================================

#[test]
fn test_operator_id_all_different() {
    assert_ne!(DELETE, YANK);
    assert_ne!(DELETE, CHANGE);
    assert_ne!(YANK, CHANGE);
}

#[test]
fn test_operator_id_from_qualified_leaked_different_module() {
    let op = OperatorId::from_qualified_leaked("custom:my-op".to_string());
    assert_eq!(op.module().as_str(), "custom");
    assert_eq!(op.name(), "my-op");
}

#[test]
fn test_operator_id_from_qualified_leaked_empty_name() {
    let op = OperatorId::from_qualified_leaked("vim:".to_string());
    assert_eq!(op.module().as_str(), "vim");
    assert_eq!(op.name(), "");
}

#[test]
fn test_operator_id_from_qualified_leaked_colon_only() {
    let op = OperatorId::from_qualified_leaked(":".to_string());
    assert_eq!(op.module().as_str(), "");
    assert_eq!(op.name(), "");
}

#[test]
fn test_operator_id_new_const() {
    const OP: OperatorId = OperatorId::new(MODULE, "test");
    assert_eq!(OP.name(), "test");
    assert_eq!(OP.module().as_str(), "vim");
}

#[test]
fn test_operator_id_display_format() {
    let op = OperatorId::new(MODULE, "custom-op");
    assert_eq!(format!("{op}"), "vim:custom-op");
}

// ========================================================================
// All command IDs have vim module
// ========================================================================

#[test]
fn test_all_visual_manipulation_ids_vim_module() {
    assert_eq!(VISUAL_SWAP_ANCHOR.module().as_str(), "vim");
    assert_eq!(TOGGLE_VISUAL_CHAR.module().as_str(), "vim");
    assert_eq!(TOGGLE_VISUAL_LINE.module().as_str(), "vim");
    assert_eq!(TOGGLE_VISUAL_BLOCK.module().as_str(), "vim");
    assert_eq!(RESELECT_LAST.module().as_str(), "vim");
}

#[test]
fn test_all_visual_operator_ids_vim_module() {
    assert_eq!(DELETE_SELECTION.module().as_str(), "vim");
    assert_eq!(YANK_SELECTION.module().as_str(), "vim");
    assert_eq!(CHANGE_SELECTION.module().as_str(), "vim");
    assert_eq!(INDENT_SELECTION.module().as_str(), "vim");
    assert_eq!(DEDENT_SELECTION.module().as_str(), "vim");
}

#[test]
fn test_session_command_ids_vim_module() {
    assert_eq!(SESSION_DETACH.module().as_str(), "vim");
    assert_eq!(SESSION_SERVERS.module().as_str(), "vim");
    assert_eq!(SESSION_KILL_SERVER.module().as_str(), "vim");
}

#[test]
fn test_visual_additional_ids_vim_module() {
    assert_eq!(VISUAL_SWAP_CORNER.module().as_str(), "vim");
    assert_eq!(TOGGLE_CASE_SELECTION.module().as_str(), "vim");
    assert_eq!(LOWERCASE_SELECTION.module().as_str(), "vim");
    assert_eq!(UPPERCASE_SELECTION.module().as_str(), "vim");
    assert_eq!(JOIN_SELECTION.module().as_str(), "vim");
    assert_eq!(COMMAND_WITH_SELECTION.module().as_str(), "vim");
    assert_eq!(VISUAL_NOOP.module().as_str(), "vim");
    assert_eq!(VISUAL_INSERT_START.module().as_str(), "vim");
    assert_eq!(VISUAL_INSERT_END.module().as_str(), "vim");
    assert_eq!(BLOCK_INSERT_START.module().as_str(), "vim");
    assert_eq!(BLOCK_INSERT_END.module().as_str(), "vim");
}

#[test]
fn test_inner_text_object_ids_vim_module() {
    assert_eq!(INNER_WORD.module().as_str(), "vim");
    assert_eq!(INNER_WORD_BIG.module().as_str(), "vim");
    assert_eq!(INNER_DOUBLE_QUOTE.module().as_str(), "vim");
    assert_eq!(INNER_SINGLE_QUOTE.module().as_str(), "vim");
    assert_eq!(INNER_BACKTICK.module().as_str(), "vim");
    assert_eq!(INNER_PAREN.module().as_str(), "vim");
    assert_eq!(INNER_BRACKET.module().as_str(), "vim");
    assert_eq!(INNER_BRACE.module().as_str(), "vim");
    assert_eq!(INNER_ANGLE.module().as_str(), "vim");
    assert_eq!(INNER_TAG.module().as_str(), "vim");
    assert_eq!(INNER_SENTENCE.module().as_str(), "vim");
    assert_eq!(INNER_PARAGRAPH.module().as_str(), "vim");
}

#[test]
fn test_around_text_object_ids_vim_module() {
    assert_eq!(AROUND_WORD.module().as_str(), "vim");
    assert_eq!(AROUND_WORD_BIG.module().as_str(), "vim");
    assert_eq!(AROUND_DOUBLE_QUOTE.module().as_str(), "vim");
    assert_eq!(AROUND_SINGLE_QUOTE.module().as_str(), "vim");
    assert_eq!(AROUND_BACKTICK.module().as_str(), "vim");
    assert_eq!(AROUND_PAREN.module().as_str(), "vim");
    assert_eq!(AROUND_BRACKET.module().as_str(), "vim");
    assert_eq!(AROUND_BRACE.module().as_str(), "vim");
    assert_eq!(AROUND_ANGLE.module().as_str(), "vim");
    assert_eq!(AROUND_TAG.module().as_str(), "vim");
    assert_eq!(AROUND_SENTENCE.module().as_str(), "vim");
    assert_eq!(AROUND_PARAGRAPH.module().as_str(), "vim");
}

#[test]
fn test_macro_command_ids_names() {
    assert_eq!(PLAY_MACRO.name(), "play-macro");
    assert_eq!(REPEAT_MACRO.name(), "repeat-macro");
}
