#![allow(clippy::field_reassign_with_default)]

use reovim_kernel::api::v1::Position;

use crate::session_state::*;

#[test]
fn test_vim_session_state_default() {
    let state = VimSessionState::default();
    assert!(state.pending_char.is_none());
    assert!(state.pending_textobj_range.is_none());
    assert!(state.pending_count.is_none());
    assert!(state.pending_register.is_none());
    assert!(!state.is_pending());
}

#[test]
fn test_textobj_range_characterwise() {
    let range = TextObjRange::characterwise(Position::new(0, 0), Position::new(0, 5));
    assert_eq!(range.start, Position::new(0, 0));
    assert_eq!(range.end, Position::new(0, 5));
    assert!(!range.is_linewise);
}

#[test]
fn test_textobj_range_linewise() {
    let range = TextObjRange::linewise(Position::new(1, 0), Position::new(3, 0));
    assert!(range.is_linewise);
}

#[test]
fn test_textobj_range_cleared_on_clear_pending() {
    let mut state = VimSessionState {
        pending_textobj_range: Some(TextObjRange::characterwise(
            Position::new(0, 0),
            Position::new(0, 5),
        )),
        ..Default::default()
    };
    assert!(state.pending_textobj_range.is_some());

    state.clear_pending();
    assert!(state.pending_textobj_range.is_none());
}

#[test]
fn test_vim_session_state_is_pending() {
    let mut state = VimSessionState::default();
    assert!(!state.is_pending());

    state.pending_count = Some(5);
    assert!(state.is_pending());

    state.clear_pending();
    assert!(!state.is_pending());
}

#[test]
fn test_vim_session_state_take_count() {
    let mut state = VimSessionState::default();
    assert_eq!(state.take_count(), 1); // Default

    state.pending_count = Some(5);
    assert_eq!(state.take_count(), 5);
    assert!(state.pending_count.is_none()); // Cleared
}

#[test]
fn test_pending_char_op_variants() {
    assert!(PendingCharOp::FindForward.is_find());
    assert!(PendingCharOp::FindBackward.is_find());
    assert!(!PendingCharOp::TillForward.is_find());

    assert!(PendingCharOp::TillForward.is_till());
    assert!(PendingCharOp::TillBackward.is_till());
    assert!(!PendingCharOp::FindForward.is_till());

    assert!(PendingCharOp::FindForward.is_forward());
    assert!(!PendingCharOp::FindBackward.is_forward());

    assert!(PendingCharOp::FindForward.is_motion());
    assert!(!PendingCharOp::Replace.is_motion());
}

// ========================================================================
// Macro Recording Tests (Epic #465 Phase 8D)
// ========================================================================

use reovim_driver_input::KeyCode;

fn key(c: char) -> reovim_driver_input::KeyEvent {
    reovim_driver_input::KeyEvent::new(KeyCode::Char(c))
}

#[test]
fn test_macro_default_state() {
    let state = VimSessionState::default();
    assert!(!state.is_recording());
    assert!(state.recording_register.is_none());
    assert!(state.recording_keys.is_empty());
    assert!(state.last_macro_register.is_none());
    assert_eq!(state.macro_playback_depth, 0);
}

#[test]
fn test_start_recording() {
    let mut state = VimSessionState::default();

    // Valid register (a-z)
    assert!(state.start_recording('a'));
    assert!(state.is_recording());
    assert_eq!(state.recording_register, Some('a'));
    assert!(state.recording_keys.is_empty());
}

#[test]
fn test_start_recording_invalid_register() {
    let mut state = VimSessionState::default();

    // Invalid registers
    assert!(!state.start_recording('A')); // uppercase
    assert!(!state.start_recording('1')); // digit
    assert!(!state.start_recording('+')); // special
    assert!(!state.is_recording());
}

#[test]
fn test_record_key() {
    let mut state = VimSessionState::default();
    state.start_recording('a');

    state.record_key(key('d'));
    state.record_key(key('w'));

    assert_eq!(state.recording_keys.len(), 2);
    assert_eq!(state.recording_keys[0].code, KeyCode::Char('d'));
    assert_eq!(state.recording_keys[1].code, KeyCode::Char('w'));
}

#[test]
fn test_record_key_not_recording() {
    let mut state = VimSessionState::default();

    // Should be a no-op when not recording
    state.record_key(key('d'));
    assert!(state.recording_keys.is_empty());
}

#[test]
fn test_stop_recording() {
    let mut state = VimSessionState::default();
    state.start_recording('a');
    state.record_key(key('d'));
    state.record_key(key('w'));

    let result = state.stop_recording();
    assert!(result.is_some());

    let (register, keys) = result.unwrap();
    assert_eq!(register, 'a');
    assert_eq!(keys.len(), 2);

    // State should be cleared
    assert!(!state.is_recording());
    assert!(state.recording_keys.is_empty());
}

#[test]
fn test_stop_recording_not_recording() {
    let mut state = VimSessionState::default();
    let result = state.stop_recording();
    assert!(result.is_none());
}

#[test]
fn test_start_recording_clears_previous() {
    let mut state = VimSessionState::default();
    state.start_recording('a');
    state.record_key(key('x'));

    // Starting new recording should clear previous keys
    state.start_recording('b');
    assert!(state.recording_keys.is_empty());
    assert_eq!(state.recording_register, Some('b'));
}

#[test]
fn test_macro_playback_depth() {
    let mut state = VimSessionState::default();

    // Can enter playback
    assert!(state.enter_macro_playback());
    assert_eq!(state.macro_playback_depth, 1);

    // Can enter multiple levels
    for i in 2..=MAX_MACRO_DEPTH {
        assert!(state.enter_macro_playback());
        assert_eq!(state.macro_playback_depth, i);
    }

    // Cannot exceed depth limit
    assert!(state.is_macro_depth_exceeded());
    assert!(!state.enter_macro_playback());
    assert_eq!(state.macro_playback_depth, MAX_MACRO_DEPTH);
}

#[test]
fn test_exit_macro_playback() {
    let mut state = VimSessionState::default();
    state.enter_macro_playback();
    state.enter_macro_playback();
    assert_eq!(state.macro_playback_depth, 2);

    state.exit_macro_playback();
    assert_eq!(state.macro_playback_depth, 1);

    state.exit_macro_playback();
    assert_eq!(state.macro_playback_depth, 0);

    // Saturating sub - doesn't go negative
    state.exit_macro_playback();
    assert_eq!(state.macro_playback_depth, 0);
}

// ========================================================================
// PendingMotion tests
// ========================================================================

#[test]
fn test_pending_motion_new() {
    let motion = PendingMotion::new(true, false, true);
    assert!(motion.linewise);
    assert!(!motion.inclusive);
    assert!(motion.word_forward);
}

#[test]
fn test_pending_motion_characterwise() {
    let motion = PendingMotion::characterwise();
    assert!(!motion.linewise);
    assert!(!motion.inclusive);
    assert!(!motion.word_forward);
}

#[test]
fn test_pending_motion_characterwise_inclusive() {
    let motion = PendingMotion::characterwise_inclusive();
    assert!(!motion.linewise);
    assert!(motion.inclusive);
    assert!(!motion.word_forward);
}

#[test]
fn test_pending_motion_linewise() {
    let motion = PendingMotion::linewise();
    assert!(motion.linewise);
    assert!(!motion.inclusive);
    assert!(!motion.word_forward);
}

#[test]
fn test_pending_motion_word_forward() {
    let motion = PendingMotion::word_forward();
    assert!(!motion.linewise);
    assert!(!motion.inclusive);
    assert!(motion.word_forward);
}

// ========================================================================
// PendingCharOp additional tests
// ========================================================================

#[test]
fn test_pending_char_op_is_find() {
    assert!(PendingCharOp::FindForward.is_find());
    assert!(PendingCharOp::FindBackward.is_find());
    assert!(!PendingCharOp::TillForward.is_find());
    assert!(!PendingCharOp::TillBackward.is_find());
    assert!(!PendingCharOp::Replace.is_find());
}

#[test]
fn test_pending_char_op_is_till() {
    assert!(!PendingCharOp::FindForward.is_till());
    assert!(!PendingCharOp::FindBackward.is_till());
    assert!(PendingCharOp::TillForward.is_till());
    assert!(PendingCharOp::TillBackward.is_till());
    assert!(!PendingCharOp::Replace.is_till());
}

#[test]
fn test_pending_char_op_is_motion() {
    assert!(PendingCharOp::FindForward.is_motion());
    assert!(PendingCharOp::FindBackward.is_motion());
    assert!(PendingCharOp::TillForward.is_motion());
    assert!(PendingCharOp::TillBackward.is_motion());
    assert!(!PendingCharOp::Replace.is_motion());
}

#[test]
fn test_pending_char_op_is_forward() {
    assert!(PendingCharOp::FindForward.is_forward());
    assert!(PendingCharOp::TillForward.is_forward());
    assert!(!PendingCharOp::FindBackward.is_forward());
    assert!(!PendingCharOp::TillBackward.is_forward());
    assert!(!PendingCharOp::Replace.is_forward());
}

#[test]
fn test_pending_char_op_equality() {
    assert_eq!(PendingCharOp::FindForward, PendingCharOp::FindForward);
    assert_ne!(PendingCharOp::FindForward, PendingCharOp::FindBackward);
}

// ========================================================================
// ChangeType and LastChange tests (Epic #465)
// ========================================================================

#[test]
fn test_change_type_operator_motion() {
    let ct = ChangeType::OperatorMotion {
        operator: OperatorType::Delete,
        linewise: false,
    };
    assert!(matches!(ct, ChangeType::OperatorMotion { .. }));
}

#[test]
fn test_change_type_operator_text_object() {
    let ct = ChangeType::OperatorTextObject {
        operator: OperatorType::Change,
        linewise: true,
    };
    assert!(matches!(ct, ChangeType::OperatorTextObject { .. }));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_change_type_insert() {
    let ct = ChangeType::Insert {
        text: "hello".to_string(),
    };
    match ct {
        ChangeType::Insert { text } => assert_eq!(text, "hello"),
        _ => panic!("Expected Insert variant"),
    }
}

#[test]
fn test_operator_type_variants() {
    assert_eq!(OperatorType::Delete, OperatorType::Delete);
    assert_ne!(OperatorType::Delete, OperatorType::Yank);
    assert_ne!(OperatorType::Yank, OperatorType::Change);
}

#[test]
fn test_last_change_with_all_fields() {
    let lc = LastChange {
        change_type: ChangeType::OperatorMotion {
            operator: OperatorType::Delete,
            linewise: true,
        },
        count: Some(5),
        register: Some('a'),
        keys: Vec::new(),
    };
    assert_eq!(lc.count, Some(5));
    assert_eq!(lc.register, Some('a'));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::redundant_clone)]
fn test_last_change_clone() {
    let lc = LastChange {
        change_type: ChangeType::Insert {
            text: "test".to_string(),
        },
        count: Some(2),
        register: None,
        keys: Vec::new(),
    };
    let cloned = lc.clone();
    assert_eq!(cloned.count, Some(2));
    match cloned.change_type {
        ChangeType::Insert { text } => assert_eq!(text, "test"),
        _ => panic!("Expected Insert"),
    }
}

// ========================================================================
// VimSessionState additional tests
// ========================================================================

#[test]
fn test_vim_session_state_is_pending_with_pending_char() {
    let mut state = VimSessionState::default();
    state.pending_char = Some(PendingCharOp::FindForward);
    assert!(state.is_pending());
}

#[test]
fn test_vim_session_state_is_pending_with_register() {
    let mut state = VimSessionState::default();
    state.pending_register = Some('a');
    assert!(state.is_pending());
}

#[test]
fn test_vim_session_state_take_register() {
    let mut state = VimSessionState::default();
    assert!(state.take_register().is_none());

    state.pending_register = Some('x');
    assert_eq!(state.take_register(), Some('x'));
    assert!(state.pending_register.is_none());
}

#[test]
fn test_vim_session_state_clear_pending_preserves_last_change() {
    let mut state = VimSessionState::default();
    state.last_change = Some(LastChange {
        change_type: ChangeType::Insert {
            text: "test".to_string(),
        },
        count: None,
        register: None,
        keys: Vec::new(),
    });
    state.pending_count = Some(5);
    state.pending_register = Some('a');
    state.pending_char = Some(PendingCharOp::FindForward);
    state.pending_motion = Some(PendingMotion::characterwise());
    state.pending_textobj_range =
        Some(TextObjRange::characterwise(Position::new(0, 0), Position::new(0, 5)));

    state.clear_pending();

    // These should be cleared
    assert!(state.pending_count.is_none());
    assert!(state.pending_register.is_none());
    assert!(state.pending_char.is_none());
    assert!(state.pending_motion.is_none());
    assert!(state.pending_textobj_range.is_none());
    // But last_change should be preserved
    assert!(state.last_change.is_some());
}

#[test]
fn test_vim_session_state_insert_buffer() {
    let mut state = VimSessionState::default();
    assert!(state.insert_buffer.is_empty());

    state.insert_buffer.push('h');
    state.insert_buffer.push('i');
    assert_eq!(state.insert_buffer, "hi");
}

#[test]
fn test_vim_session_state_max_macro_depth() {
    assert_eq!(MAX_MACRO_DEPTH, 16);
}

#[test]
fn test_vim_session_state_is_macro_depth_exceeded_at_zero() {
    let state = VimSessionState::default();
    assert!(!state.is_macro_depth_exceeded());
}

#[test]
fn test_vim_session_state_is_macro_depth_exceeded_at_max() {
    let mut state = VimSessionState::default();
    state.macro_playback_depth = MAX_MACRO_DEPTH;
    assert!(state.is_macro_depth_exceeded());
}

#[test]
fn test_vim_session_state_last_macro_register() {
    let mut state = VimSessionState::default();
    assert!(state.last_macro_register.is_none());

    state.last_macro_register = Some('q');
    assert_eq!(state.last_macro_register, Some('q'));
}

#[test]
fn test_pending_motion_debug() {
    let motion = PendingMotion::characterwise();
    let debug = format!("{motion:?}");
    assert!(debug.contains("PendingMotion"));
}

#[test]
fn test_pending_char_op_debug() {
    let op = PendingCharOp::FindForward;
    let debug = format!("{op:?}");
    assert!(debug.contains("FindForward"));
}

#[test]
fn test_change_type_debug() {
    let ct = ChangeType::Insert {
        text: "test".to_string(),
    };
    let debug = format!("{ct:?}");
    assert!(debug.contains("Insert"));
}

#[test]
fn test_vim_session_state_debug() {
    let state = VimSessionState::default();
    let debug = format!("{state:?}");
    assert!(debug.contains("VimSessionState"));
}

#[test]
fn test_session_extension_create() {
    let state = <VimSessionState as reovim_driver_session::SessionExtension>::create();
    assert!(!state.is_pending());
    assert!(!state.is_recording());
    assert!(!state.is_macro_depth_exceeded());
}

// ========================================================================
// Additional VimSessionState tests
// ========================================================================

#[test]
fn test_vim_session_state_clear_pending_does_not_clear_insert_buffer() {
    let mut state = VimSessionState::default();
    state.insert_buffer.push_str("hello");
    state.pending_count = Some(5);

    state.clear_pending();

    assert!(state.pending_count.is_none());
    // insert_buffer is NOT cleared by clear_pending
    assert_eq!(state.insert_buffer, "hello");
}

#[test]
fn test_vim_session_state_take_count_default_is_one() {
    let mut state = VimSessionState::default();
    // No pending count
    assert_eq!(state.take_count(), 1);
}

#[test]
fn test_vim_session_state_take_count_clears() {
    let mut state = VimSessionState::default();
    state.pending_count = Some(42);
    assert_eq!(state.take_count(), 42);
    assert!(state.pending_count.is_none());
    // Second call should return default
    assert_eq!(state.take_count(), 1);
}

#[test]
fn test_vim_session_state_take_register_clears() {
    let mut state = VimSessionState::default();
    state.pending_register = Some('z');
    assert_eq!(state.take_register(), Some('z'));
    assert!(state.pending_register.is_none());
    assert_eq!(state.take_register(), None);
}

#[test]
fn test_vim_session_state_is_pending_with_all_set() {
    let mut state = VimSessionState::default();
    state.pending_char = Some(PendingCharOp::TillForward);
    state.pending_count = Some(3);
    state.pending_register = Some('a');
    assert!(state.is_pending());
}

#[test]
fn test_vim_session_state_clear_pending_clears_all() {
    let mut state = VimSessionState::default();
    state.pending_char = Some(PendingCharOp::FindForward);
    state.pending_count = Some(5);
    state.pending_register = Some('x');
    state.pending_motion = Some(PendingMotion::linewise());
    state.pending_textobj_range =
        Some(TextObjRange::characterwise(Position::new(0, 0), Position::new(0, 5)));

    state.clear_pending();

    assert!(!state.is_pending());
    assert!(state.pending_char.is_none());
    assert!(state.pending_count.is_none());
    assert!(state.pending_register.is_none());
    assert!(state.pending_motion.is_none());
    assert!(state.pending_textobj_range.is_none());
}

#[test]
fn test_vim_session_state_recording_lifecycle() {
    let mut state = VimSessionState::default();

    // Start
    assert!(state.start_recording('a'));
    assert!(state.is_recording());

    // Record keys
    state.record_key(key('d'));
    state.record_key(key('w'));
    assert_eq!(state.recording_keys.len(), 2);

    // Stop
    let result = state.stop_recording();
    assert!(result.is_some());
    let (reg, keys) = result.unwrap();
    assert_eq!(reg, 'a');
    assert_eq!(keys.len(), 2);

    // Verify state is clean
    assert!(!state.is_recording());
    assert!(state.recording_keys.is_empty());
}

#[test]
fn test_vim_session_state_macro_playback_lifecycle() {
    let mut state = VimSessionState::default();

    // Enter
    assert!(state.enter_macro_playback());
    assert_eq!(state.macro_playback_depth, 1);

    // Enter again
    assert!(state.enter_macro_playback());
    assert_eq!(state.macro_playback_depth, 2);

    // Exit
    state.exit_macro_playback();
    assert_eq!(state.macro_playback_depth, 1);

    state.exit_macro_playback();
    assert_eq!(state.macro_playback_depth, 0);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_vim_session_state_start_recording_boundary_chars() {
    let mut state = VimSessionState::default();

    // 'a' is valid
    assert!(state.start_recording('a'));
    state.stop_recording();

    // 'z' is valid
    assert!(state.start_recording('z'));
    state.stop_recording();

    // Just before 'a' is invalid
    assert!(!state.start_recording('`'));

    // Just after 'z' is invalid
    assert!(!state.start_recording('{'));
}

#[test]
fn test_pending_motion_clone() {
    let motion = PendingMotion::characterwise_inclusive();
    let cloned = motion;
    assert_eq!(cloned.inclusive, motion.inclusive);
    assert_eq!(cloned.linewise, motion.linewise);
    assert_eq!(cloned.word_forward, motion.word_forward);
}

#[test]
fn test_pending_motion_all_flags_set() {
    let motion = PendingMotion::new(true, true, true);
    assert!(motion.linewise);
    assert!(motion.inclusive);
    assert!(motion.word_forward);
}

#[test]
fn test_pending_motion_all_flags_unset() {
    let motion = PendingMotion::new(false, false, false);
    assert!(!motion.linewise);
    assert!(!motion.inclusive);
    assert!(!motion.word_forward);
}

#[test]
fn test_pending_char_op_clone_copy() {
    let op = PendingCharOp::TillBackward;
    let copied = op;
    assert_eq!(copied, op);
}

#[test]
#[allow(clippy::redundant_clone)]
fn test_change_type_clone() {
    let ct = ChangeType::OperatorMotion {
        operator: OperatorType::Yank,
        linewise: true,
    };
    let cloned = ct.clone();
    assert!(matches!(
        cloned,
        ChangeType::OperatorMotion {
            operator: OperatorType::Yank,
            linewise: true,
        }
    ));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::redundant_clone)]
fn test_change_type_insert_clone() {
    let ct = ChangeType::Insert {
        text: "some text".to_string(),
    };
    let cloned = ct.clone();
    match cloned {
        ChangeType::Insert { text } => assert_eq!(text, "some text"),
        _ => panic!("Expected Insert"),
    }
}

#[test]
fn test_operator_type_debug() {
    let d = format!("{:?}", OperatorType::Delete);
    assert!(d.contains("Delete"));
    let y = format!("{:?}", OperatorType::Yank);
    assert!(y.contains("Yank"));
    let c = format!("{:?}", OperatorType::Change);
    assert!(c.contains("Change"));
}

#[test]
fn test_operator_type_clone_copy() {
    let op = OperatorType::Delete;
    let copied = op;
    assert_eq!(copied, OperatorType::Delete);
}

#[test]
fn test_last_change_no_count_no_register() {
    let lc = LastChange {
        change_type: ChangeType::Insert {
            text: "x".to_string(),
        },
        count: None,
        register: None,
        keys: Vec::new(),
    };
    assert!(lc.count.is_none());
    assert!(lc.register.is_none());
}

#[test]
fn test_last_change_debug() {
    let lc = LastChange {
        change_type: ChangeType::OperatorTextObject {
            operator: OperatorType::Change,
            linewise: false,
        },
        count: Some(1),
        register: Some('a'),
        keys: Vec::new(),
    };
    let debug = format!("{lc:?}");
    assert!(debug.contains("LastChange"));
    assert!(debug.contains("OperatorTextObject"));
}

#[test]
fn test_textobj_range_characterwise_fields() {
    let range = TextObjRange::characterwise(Position::new(1, 5), Position::new(3, 10));
    assert_eq!(range.start.line, 1);
    assert_eq!(range.start.column, 5);
    assert_eq!(range.end.line, 3);
    assert_eq!(range.end.column, 10);
    assert!(!range.is_linewise);
}

#[test]
fn test_textobj_range_linewise_fields() {
    let range = TextObjRange::linewise(Position::new(2, 0), Position::new(5, 0));
    assert_eq!(range.start.line, 2);
    assert_eq!(range.end.line, 5);
    assert!(range.is_linewise);
}

// ========================================================================
// Dot Repeat Recording Tests (#577)
// ========================================================================

#[test]
fn test_dot_repeat_default_state() {
    let state = VimSessionState::default();
    assert!(!state.recording_repeat);
    assert!(state.repeat_keys.is_empty());
}

#[test]
fn test_start_repeat_recording() {
    let mut state = VimSessionState::default();
    state.repeat_keys.push(key('x')); // leftover from previous

    state.start_repeat_recording();

    assert!(state.recording_repeat);
    assert!(state.repeat_keys.is_empty()); // cleared
}

#[test]
fn test_record_repeat_key_when_recording() {
    let mut state = VimSessionState::default();
    state.start_repeat_recording();

    state.record_repeat_key(key('c'));
    state.record_repeat_key(key('w'));

    assert_eq!(state.repeat_keys.len(), 2);
    assert_eq!(state.repeat_keys[0].code, KeyCode::Char('c'));
    assert_eq!(state.repeat_keys[1].code, KeyCode::Char('w'));
}

#[test]
fn test_record_repeat_key_when_not_recording() {
    let mut state = VimSessionState::default();
    // Not recording — should be a no-op
    state.record_repeat_key(key('d'));
    assert!(state.repeat_keys.is_empty());
}

#[test]
fn test_finish_repeat_recording_saves_to_last_change() {
    let mut state = VimSessionState::default();
    state.last_change = Some(LastChange {
        change_type: ChangeType::OperatorMotion {
            operator: OperatorType::Delete,
            linewise: false,
        },
        count: None,
        register: None,
        keys: Vec::new(),
    });

    state.start_repeat_recording();
    state.record_repeat_key(key('d'));
    state.record_repeat_key(key('w'));
    state.finish_repeat_recording();

    assert!(!state.recording_repeat);
    assert!(state.repeat_keys.is_empty());

    let lc = state.last_change.as_ref().unwrap();
    assert_eq!(lc.keys.len(), 2);
    assert_eq!(lc.keys[0].code, KeyCode::Char('d'));
    assert_eq!(lc.keys[1].code, KeyCode::Char('w'));
}

#[test]
fn test_finish_repeat_recording_without_last_change() {
    let mut state = VimSessionState::default();
    // No last_change set

    state.start_repeat_recording();
    state.record_repeat_key(key('x'));
    state.finish_repeat_recording();

    assert!(!state.recording_repeat);
    assert!(state.repeat_keys.is_empty());
    assert!(state.last_change.is_none()); // still none
}

#[test]
fn test_repeat_recording_full_lifecycle() {
    let mut state = VimSessionState::default();

    // Start recording for an operator
    state.start_repeat_recording();
    state.record_repeat_key(key('c'));
    state.record_repeat_key(key('w'));

    // Simulate entering insert mode and typing
    state.record_repeat_key(key('b'));
    state.record_repeat_key(key('a'));
    state.record_repeat_key(key('r'));

    // Simulate Esc
    let esc = reovim_driver_input::KeyEvent::new(KeyCode::Escape);
    state.record_repeat_key(esc);

    // Set last_change before finishing
    state.last_change = Some(LastChange {
        change_type: ChangeType::OperatorMotion {
            operator: OperatorType::Change,
            linewise: false,
        },
        count: None,
        register: None,
        keys: Vec::new(),
    });

    state.finish_repeat_recording();

    let lc = state.last_change.as_ref().unwrap();
    assert_eq!(lc.keys.len(), 6); // c, w, b, a, r, Esc
}

#[test]
fn test_repeat_and_macro_recording_independent() {
    let mut state = VimSessionState::default();

    // Start both recordings
    state.start_repeat_recording();
    state.start_recording('a');

    // Record keys to both
    state.record_repeat_key(key('d'));
    state.record_key(key('d'));
    state.record_repeat_key(key('w'));
    state.record_key(key('w'));

    // Both should have keys
    assert_eq!(state.repeat_keys.len(), 2);
    assert_eq!(state.recording_keys.len(), 2);

    // Finish repeat recording
    state.last_change = Some(LastChange {
        change_type: ChangeType::OperatorMotion {
            operator: OperatorType::Delete,
            linewise: false,
        },
        count: None,
        register: None,
        keys: Vec::new(),
    });
    state.finish_repeat_recording();

    // Repeat is done but macro still recording
    assert!(!state.recording_repeat);
    assert!(state.is_recording());
    assert_eq!(state.recording_keys.len(), 2);
}

#[test]
fn test_last_change_keys_field() {
    let lc = LastChange {
        change_type: ChangeType::OperatorMotion {
            operator: OperatorType::Delete,
            linewise: false,
        },
        count: None,
        register: None,
        keys: vec![key('d'), key('w')],
    };
    assert_eq!(lc.keys.len(), 2);
    assert_eq!(lc.keys[0].code, KeyCode::Char('d'));
}
