#![allow(clippy::equatable_if_let)]

use {
    reovim_domain_text::Position,
    reovim_driver_command_types::ArgValue,
    reovim_driver_input::{
        KeyCode, KeyEvent, KeyLookupState, ModeTransition, Modifiers, PopResult,
    },
    reovim_kernel::api::v1::{CommandId, ModuleId},
};

use {super::super::operator_common::*, crate::modes::VimMode};

fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c))
}

#[test]
fn test_is_escape() {
    assert!(is_escape(&KeyEvent::new(KeyCode::Escape)));
    assert!(is_escape(&KeyEvent::with_modifiers(KeyCode::Char('['), Modifiers::CTRL)));
    assert!(!is_escape(&key('d')));
}

#[test]
fn test_is_count_digit() {
    // First digit
    assert!(is_count_digit(&key('1'), false));
    assert!(is_count_digit(&key('9'), false));
    assert!(!is_count_digit(&key('0'), false)); // 0 is motion when no count

    // Subsequent digit
    assert!(is_count_digit(&key('0'), true));

    // With modifiers - not a count digit
    assert!(!is_count_digit(
        &KeyEvent::with_modifiers(KeyCode::Char('3'), Modifiers::CTRL),
        false
    ));
}

#[test]
fn test_accumulate_count() {
    assert_eq!(accumulate_count(&key('3'), None), Some(3));
    assert_eq!(accumulate_count(&key('5'), Some(3)), Some(35));
    assert_eq!(accumulate_count(&key('0'), Some(35)), Some(350));
}

#[test]
fn test_is_line_operator_key() {
    assert!(is_line_operator_key(&key('d'), OperatorType::Delete));
    assert!(!is_line_operator_key(&key('y'), OperatorType::Delete));
    assert!(is_line_operator_key(&key('y'), OperatorType::Yank));
    assert!(is_line_operator_key(&key('c'), OperatorType::Change));
}

#[test]
fn test_is_linewise_motion() {
    use reovim_kernel::api::v1::ModuleId;

    let editor = ModuleId::new("editor");
    let motions = ModuleId::new("motions");

    // j, k are linewise
    assert!(is_linewise_motion(&CommandId::new(editor.clone(), "cursor-down")));
    assert!(is_linewise_motion(&CommandId::new(editor.clone(), "cursor-up")));

    // gg, G are linewise
    assert!(is_linewise_motion(&CommandId::new(motions.clone(), "document-start")));
    assert!(is_linewise_motion(&CommandId::new(motions.clone(), "document-end")));

    // whole-line for dd, yy, cc
    assert!(is_linewise_motion(&CommandId::new(motions.clone(), "whole-line")));

    // h, l are NOT linewise
    assert!(!is_linewise_motion(&CommandId::new(editor.clone(), "cursor-left")));
    assert!(!is_linewise_motion(&CommandId::new(editor, "cursor-right")));

    // word motions are NOT linewise
    assert!(!is_linewise_motion(&CommandId::new(motions, "word-forward")));
}

#[test]
fn test_operator_state_new() {
    let state = OperatorState::new(OperatorType::Delete);
    assert_eq!(state.operator, OperatorType::Delete);
    assert!(state.start_position.is_none());
    assert!(state.operator_count.is_none());
    assert!(state.motion_count.is_none());
    assert!(!state.initialized, "new state should not be initialized");
}

#[test]
fn test_operator_state_with_context() {
    let state = OperatorState::with_context(OperatorType::Delete, Some(3), Some('a'));
    assert_eq!(state.operator, OperatorType::Delete);
    assert_eq!(state.operator_count, Some(3));
    assert_eq!(state.register, Some('a'));
    assert!(state.initialized, "with_context should be pre-initialized");
}

#[test]
fn test_operator_state_reset() {
    let mut state = OperatorState::with_context(OperatorType::Yank, Some(2), Some('b'));
    state.start_position = Some(Position::new(5, 10));
    state.motion_count = Some(3);
    state.push_key(KeyEvent::new(KeyCode::Char('w')));

    state.reset();

    assert!(state.start_position.is_none());
    assert!(state.operator_count.is_none());
    assert!(state.motion_count.is_none());
    assert!(state.register.is_none());
    assert!(state.pending_keys.is_empty());
    assert!(!state.initialized, "reset should clear initialized flag");
}

#[test]
fn test_operator_state_effective_count() {
    let mut state = OperatorState::new(OperatorType::Yank);
    assert_eq!(state.effective_count(), 1); // No counts

    state.operator_count = Some(2);
    assert_eq!(state.effective_count(), 2);

    state.motion_count = Some(3);
    assert_eq!(state.effective_count(), 6); // 2 * 3
}

#[test]
fn test_operator_state_explicit_count() {
    // #429: explicit_count() returns None when no count specified
    let mut state = OperatorState::new(OperatorType::Delete);
    assert_eq!(state.explicit_count(), None); // No counts -> None

    state.operator_count = Some(2);
    assert_eq!(state.explicit_count(), Some(2)); // Only operator count

    state.operator_count = None;
    state.motion_count = Some(3);
    assert_eq!(state.explicit_count(), Some(3)); // Only motion count

    state.operator_count = Some(2);
    state.motion_count = Some(3);
    assert_eq!(state.explicit_count(), Some(6)); // Both -> product
}

#[test]
fn test_operator_type_mode_id() {
    assert_eq!(OperatorType::Delete.mode_id(), VimMode::DELETE_ID);
    assert_eq!(OperatorType::Yank.mode_id(), VimMode::YANK_ID);
    assert_eq!(OperatorType::Change.mode_id(), VimMode::CHANGE_ID);
}

// ========================================================================
// Additional OperatorType tests
// ========================================================================

#[test]
fn test_operator_type_operator_id() {
    let delete_id = OperatorType::Delete.operator_id();
    assert_eq!(delete_id.name(), "delete");
    assert_eq!(delete_id.module().as_str(), "vim");

    let yank_id = OperatorType::Yank.operator_id();
    assert_eq!(yank_id.name(), "yank");

    let change_id = OperatorType::Change.operator_id();
    assert_eq!(change_id.name(), "change");
}

#[test]
fn test_operator_type_line_key() {
    assert_eq!(OperatorType::Delete.line_key(), 'd');
    assert_eq!(OperatorType::Yank.line_key(), 'y');
    assert_eq!(OperatorType::Change.line_key(), 'c');
}

#[test]
fn test_operator_type_display_name() {
    assert_eq!(OperatorType::Delete.display_name(), "DELETE");
    assert_eq!(OperatorType::Yank.display_name(), "YANK");
    assert_eq!(OperatorType::Change.display_name(), "CHANGE");
}

#[test]
fn test_operator_type_copy_clone() {
    let op = OperatorType::Delete;
    let copied = op;
    #[allow(clippy::clone_on_copy)]
    let cloned = op.clone();
    assert_eq!(op, copied);
    assert_eq!(op, cloned);
}

// ========================================================================
// is_escape additional tests
// ========================================================================

#[test]
fn test_is_escape_regular_chars() {
    assert!(!is_escape(&key('a')));
    assert!(!is_escape(&key('['))); // [ without Ctrl is NOT escape
    assert!(!is_escape(&KeyEvent::new(KeyCode::Enter)));
    assert!(!is_escape(&KeyEvent::new(KeyCode::Backspace)));
}

// ========================================================================
// is_linewise_motion additional tests
// ========================================================================

#[test]
#[allow(clippy::redundant_clone)]
fn test_is_linewise_motion_paragraph() {
    let motions = ModuleId::new("motions");
    assert!(is_linewise_motion(&CommandId::new(motions.clone(), "paragraph-forward")));
    assert!(is_linewise_motion(&CommandId::new(motions.clone(), "paragraph-backward")));
}

#[test]
#[allow(clippy::redundant_clone)]
fn test_is_linewise_motion_screen() {
    let motions = ModuleId::new("motions");
    assert!(is_linewise_motion(&CommandId::new(motions.clone(), "screen-top")));
    assert!(is_linewise_motion(&CommandId::new(motions.clone(), "screen-middle")));
    assert!(is_linewise_motion(&CommandId::new(motions.clone(), "screen-bottom")));
}

#[test]
fn test_is_linewise_motion_next_prev_line() {
    let motions = ModuleId::new("motions");
    assert!(is_linewise_motion(&CommandId::new(motions.clone(), "next-line")));
    assert!(is_linewise_motion(&CommandId::new(motions, "prev-line")));
}

#[test]
fn test_is_linewise_motion_characterwise_motions() {
    let motions = ModuleId::new("motions");
    assert!(!is_linewise_motion(&CommandId::new(motions.clone(), "word-forward-big")));
    assert!(!is_linewise_motion(&CommandId::new(motions.clone(), "word-backward")));
    assert!(!is_linewise_motion(&CommandId::new(motions.clone(), "word-end")));
    assert!(!is_linewise_motion(&CommandId::new(motions.clone(), "line-start")));
    assert!(!is_linewise_motion(&CommandId::new(motions, "line-end")));
}

// ========================================================================
// is_inclusive_motion tests
// ========================================================================

#[test]
fn test_is_inclusive_motion_inclusive() {
    let motions = ModuleId::new("motions");
    assert!(is_inclusive_motion(&CommandId::new(motions.clone(), "line-end")));
    assert!(is_inclusive_motion(&CommandId::new(motions.clone(), "word-end")));
    assert!(is_inclusive_motion(&CommandId::new(motions.clone(), "word-end-big")));
    assert!(is_inclusive_motion(&CommandId::new(motions.clone(), "find-char")));
    assert!(is_inclusive_motion(&CommandId::new(motions.clone(), "find-char-back")));
    assert!(is_inclusive_motion(&CommandId::new(motions.clone(), "till-char")));
    assert!(is_inclusive_motion(&CommandId::new(motions.clone(), "till-char-back")));
    assert!(is_inclusive_motion(&CommandId::new(motions, "match-bracket")));
}

#[test]
#[allow(clippy::redundant_clone)]
fn test_is_inclusive_motion_exclusive() {
    let motions = ModuleId::new("motions");
    let editor = ModuleId::new("editor");
    assert!(!is_inclusive_motion(&CommandId::new(motions.clone(), "word-forward")));
    assert!(!is_inclusive_motion(&CommandId::new(motions.clone(), "word-backward")));
    assert!(!is_inclusive_motion(&CommandId::new(motions.clone(), "line-start")));
    assert!(!is_inclusive_motion(&CommandId::new(editor.clone(), "cursor-left")));
    assert!(!is_inclusive_motion(&CommandId::new(editor, "cursor-right")));
}

// ========================================================================
// is_word_forward_motion tests
// ========================================================================

#[test]
fn test_is_word_forward_motion() {
    let motions = ModuleId::new("motions");
    assert!(is_word_forward_motion(&CommandId::new(motions.clone(), "word-forward")));
    assert!(is_word_forward_motion(&CommandId::new(motions.clone(), "word-forward-big")));
    assert!(!is_word_forward_motion(&CommandId::new(motions.clone(), "word-backward")));
    assert!(!is_word_forward_motion(&CommandId::new(motions.clone(), "word-end")));
    assert!(!is_word_forward_motion(&CommandId::new(motions, "cursor-down")));
}

// ========================================================================
// is_line_operator_key additional tests
// ========================================================================

#[test]
fn test_is_line_operator_key_with_modifiers() {
    // Modified keys should NOT be line operator keys
    let key_ctrl_d = KeyEvent::with_modifiers(KeyCode::Char('d'), Modifiers::CTRL);
    assert!(!is_line_operator_key(&key_ctrl_d, OperatorType::Delete));
}

// ========================================================================
// build_operator_execute tests
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_build_operator_execute_delete() {
    let start = Position::new(0, 0);
    let end = Position::new(0, 5);
    let result = build_operator_execute(OperatorType::Delete, start, end, false, None, None);

    match result {
        PopResult::ExecuteCommand { command, args } => {
            assert_eq!(command.name(), "delete");
            assert_eq!(command.module().as_str(), "vim");
            // Check linewise flag
            assert!(args.contains_key("linewise"));
            // Check range positions
            assert!(args.contains_key("range_start"));
            assert!(args.contains_key("range_end"));
            // Check count defaults to 1
            assert!(args.contains_key("count"));
            // No register specified
            assert!(!args.contains_key("register"));
        }
        _ => panic!("Expected ExecuteCommand"),
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_build_operator_execute_with_count_and_register() {
    let start = Position::new(1, 0);
    let end = Position::new(3, 0);
    let result = build_operator_execute(OperatorType::Yank, start, end, true, Some(3), Some('a'));

    match result {
        PopResult::ExecuteCommand { command, args } => {
            assert_eq!(command.name(), "yank");
            // Check linewise is true
            if let Some(ArgValue::Bool(linewise)) = args.get("linewise") {
                assert!(linewise);
            } else {
                panic!("Expected linewise Bool");
            }
            // Check count
            if let Some(ArgValue::Count(count)) = args.get("count") {
                assert_eq!(*count, 3);
            } else {
                panic!("Expected count");
            }
            // Check register
            if let Some(ArgValue::Register(reg)) = args.get("register") {
                assert_eq!(*reg, 'a');
            } else {
                panic!("Expected register");
            }
        }
        _ => panic!("Expected ExecuteCommand"),
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_build_operator_execute_change() {
    let start = Position::new(0, 5);
    let end = Position::new(0, 10);
    let result = build_operator_execute(OperatorType::Change, start, end, false, Some(1), None);

    match result {
        PopResult::ExecuteCommand { command, .. } => {
            assert_eq!(command.name(), "change");
            assert_eq!(command.module().as_str(), "vim");
        }
        _ => panic!("Expected ExecuteCommand"),
    }
}

// ========================================================================
// build_cancelled tests
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_build_cancelled() {
    let transition = build_cancelled();
    match transition {
        ModeTransition::Pop { result } => {
            assert!(result.is_some());
            if let Some(PopResult::Cancelled) = result {
                // OK
            } else {
                panic!("Expected PopResult::Cancelled");
            }
        }
        _ => panic!("Expected ModeTransition::Pop"),
    }
}

// ========================================================================
// apply_keymap_policy tests
// ========================================================================

#[test]
fn test_apply_keymap_policy_exact_only() {
    let cmd = CommandId::new(ModuleId::new("test"), "cursor-down");
    let lookup = KeyLookupState::ExactOnly(cmd);
    let action = apply_keymap_policy(&lookup);
    assert!(matches!(action, KeymapAction::Execute(_)));
}

#[test]
fn test_apply_keymap_policy_exact_with_longer() {
    let cmd = CommandId::new(ModuleId::new("test"), "delete");
    let lookup = KeyLookupState::ExactWithLonger { exact: cmd };
    let action = apply_keymap_policy(&lookup);
    // ExactWithLonger should also Execute (not wait)
    assert!(matches!(action, KeymapAction::Execute(_)));
}

#[test]
fn test_apply_keymap_policy_prefix_only() {
    let lookup = KeyLookupState::PrefixOnly;
    let action = apply_keymap_policy(&lookup);
    assert!(matches!(action, KeymapAction::Pending));
}

#[test]
fn test_apply_keymap_policy_not_found() {
    let lookup = KeyLookupState::NotFound;
    let action = apply_keymap_policy(&lookup);
    assert!(matches!(action, KeymapAction::Cancel));
}

// ========================================================================
// OperatorState additional tests
// ========================================================================

#[test]
fn test_operator_state_set_start_position() {
    let mut state = OperatorState::new(OperatorType::Delete);
    assert!(state.start_position.is_none());

    state.set_start_position(Position::new(5, 10));
    assert_eq!(state.start_position, Some(Position::new(5, 10)));
}

#[test]
fn test_operator_state_has_motion_count() {
    let mut state = OperatorState::new(OperatorType::Yank);
    assert!(!state.has_motion_count());

    state.motion_count = Some(3);
    assert!(state.has_motion_count());
}

#[test]
fn test_operator_state_accumulate_motion_count() {
    let mut state = OperatorState::new(OperatorType::Delete);
    state.accumulate_motion_count(&key('3'));
    assert_eq!(state.motion_count, Some(3));

    state.accumulate_motion_count(&key('5'));
    assert_eq!(state.motion_count, Some(35));

    state.accumulate_motion_count(&key('0'));
    assert_eq!(state.motion_count, Some(350));
}

#[test]
fn test_operator_state_take_motion_count() {
    let mut state = OperatorState::new(OperatorType::Delete);
    state.motion_count = Some(5);
    assert_eq!(state.take_motion_count(), Some(5));
    assert!(state.motion_count.is_none());
}

#[test]
fn test_operator_state_push_and_keys() {
    let mut state = OperatorState::new(OperatorType::Delete);
    assert!(state.keys().is_empty());

    state.push_key(key('d'));
    state.push_key(key('w'));
    assert_eq!(state.keys().len(), 2);
}

#[test]
fn test_operator_state_clear_keys() {
    let mut state = OperatorState::new(OperatorType::Delete);
    state.push_key(key('x'));
    assert!(!state.keys().is_empty());

    state.clear_keys();
    assert!(state.keys().is_empty());
}

#[test]
fn test_operator_state_effective_count_motion_only() {
    let mut state = OperatorState::new(OperatorType::Delete);
    state.motion_count = Some(5);
    assert_eq!(state.effective_count(), 5);
}

#[test]
fn test_operator_state_effective_count_both() {
    let mut state = OperatorState::new(OperatorType::Delete);
    state.operator_count = Some(3);
    state.motion_count = Some(4);
    assert_eq!(state.effective_count(), 12);
}

// ========================================================================
// accumulate_count edge cases
// ========================================================================

#[test]
fn test_accumulate_count_non_digit() {
    // Non-digit key should not change count
    let result = accumulate_count(&key('a'), Some(5));
    assert_eq!(result, Some(5));
}

#[test]
fn test_accumulate_count_from_none() {
    let result = accumulate_count(&key('7'), None);
    assert_eq!(result, Some(7));
}

// ========================================================================
// KeymapAction debug
// ========================================================================

#[test]
fn test_keymap_action_debug() {
    let action = KeymapAction::Pending;
    let debug = format!("{action:?}");
    assert!(debug.contains("Pending"));

    let action = KeymapAction::Cancel;
    let debug = format!("{action:?}");
    assert!(debug.contains("Cancel"));
}
