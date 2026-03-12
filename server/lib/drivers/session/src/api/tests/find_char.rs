
use super::*;


#[test]
fn test_find_char_record_new_and_accessors() {
    let record = FindCharRecord::new('x', true, true);
    assert_eq!(record.char(), 'x');
    assert!(record.forward());
    assert!(record.inclusive());
}

#[test]
fn test_find_char_record_backward_till() {
    let record = FindCharRecord::new('a', false, false);
    assert_eq!(record.char(), 'a');
    assert!(!record.forward());
    assert!(!record.inclusive());
}

#[test]
fn test_find_char_record_reversed() {
    let record = FindCharRecord::new('x', true, true);
    let rev = record.reversed();
    assert_eq!(rev.char(), 'x');
    assert!(!rev.forward());
    assert!(rev.inclusive());
}

#[test]
fn test_find_char_record_reversed_preserves_inclusive() {
    let record = FindCharRecord::new('t', true, false);
    let rev = record.reversed();
    assert_eq!(rev.char(), 't');
    assert!(!rev.forward());
    assert!(!rev.inclusive());
}

#[test]
fn test_find_char_record_double_reversed() {
    let original = FindCharRecord::new('z', true, false);
    let double_rev = original.reversed().reversed();
    assert_eq!(double_rev, original);
}

#[test]
fn test_find_char_record_unicode() {
    let record = FindCharRecord::new('\u{1F600}', true, true);
    assert_eq!(record.char(), '\u{1F600}');
}

#[test]
fn test_find_char_record_partial_eq() {
    let a = FindCharRecord::new('x', true, true);
    let b = FindCharRecord::new('x', true, true);
    let c = FindCharRecord::new('y', true, true);
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn test_find_char_record_clone_copy() {
    let record = FindCharRecord::new('a', true, false);
    let cloned = record;
    let copied = record;
    assert_eq!(record, cloned);
    assert_eq!(record, copied);
}

#[test]
fn test_find_char_record_debug() {
    let record = FindCharRecord::new('x', true, true);
    let debug = format!("{record:?}");
    assert!(debug.contains("FindCharRecord"));
}

#[test]
fn test_find_char_state_default() {
    let state = FindCharState::default();
    assert!(state.last().is_none());
}

#[test]
fn test_find_char_state_record_and_last() {
    let mut state = FindCharState::default();
    state.record('x', true, true);
    let record = state.last().unwrap();
    assert_eq!(record.char(), 'x');
    assert!(record.forward());
    assert!(record.inclusive());
}

#[test]
fn test_find_char_state_record_overwrites() {
    let mut state = FindCharState::default();
    state.record('a', true, true);
    state.record('b', false, false);
    let record = state.last().unwrap();
    assert_eq!(record.char(), 'b');
    assert!(!record.forward());
    assert!(!record.inclusive());
}

#[test]
fn test_find_char_state_last_returns_ref() {
    let mut state = FindCharState::default();
    state.record('x', true, true);
    let copied = state.last().copied();
    assert_eq!(copied.unwrap().char(), 'x');
}

#[test]
fn test_session_extension_create() {
    let state = FindCharState::create();
    assert!(state.last().is_none());
}

#[test]
fn test_find_char_state_debug() {
    let state = FindCharState::default();
    let debug = format!("{state:?}");
    assert!(debug.contains("FindCharState"));
}