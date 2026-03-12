use crate::{KeyCode, KeyEvent, KeyEventKind, KeymapResult, Modifiers};

#[test]
fn test_modifiers_bitflags() {
    let ctrl_shift = Modifiers::CTRL | Modifiers::SHIFT;
    assert!(ctrl_shift.contains(Modifiers::CTRL));
    assert!(ctrl_shift.contains(Modifiers::SHIFT));
    assert!(!ctrl_shift.contains(Modifiers::ALT));
    assert!(!ctrl_shift.is_empty());
    assert!(Modifiers::NONE.is_empty());
}

#[test]
fn test_modifiers_all_variants() {
    // Test all modifier flags can be combined
    let all = Modifiers::SHIFT
        | Modifiers::CTRL
        | Modifiers::ALT
        | Modifiers::SUPER
        | Modifiers::HYPER
        | Modifiers::META;
    assert!(all.contains(Modifiers::SHIFT));
    assert!(all.contains(Modifiers::CTRL));
    assert!(all.contains(Modifiers::ALT));
    assert!(all.contains(Modifiers::SUPER));
    assert!(all.contains(Modifiers::HYPER));
    assert!(all.contains(Modifiers::META));
}

#[test]
fn test_key_event_creation() {
    let key = KeyEvent::new(KeyCode::Char('a'));
    assert_eq!(key.code, KeyCode::Char('a'));
    assert_eq!(key.modifiers, Modifiers::NONE);
    assert!(key.is_press());
    assert!(!key.is_release());
    assert!(!key.is_repeat());

    let ctrl_a = KeyEvent::with_modifiers(KeyCode::Char('a'), Modifiers::CTRL);
    assert!(ctrl_a.modifiers.contains(Modifiers::CTRL));
    assert!(ctrl_a.is_press());

    let release = KeyEvent::full(KeyCode::Escape, Modifiers::NONE, KeyEventKind::Release);
    assert!(release.is_release());
}

#[test]
fn test_keymap_result_is_match() {
    let match_result: KeymapResult<i32> = KeymapResult::Match(42);
    assert!(match_result.is_match());
    assert!(!match_result.is_prefix());
    assert!(!match_result.is_none());
}

#[test]
fn test_keymap_result_is_prefix() {
    let prefix: KeymapResult<i32> = KeymapResult::Prefix;
    assert!(!prefix.is_match());
    assert!(prefix.is_prefix());
    assert!(!prefix.is_none());
}

#[test]
fn test_keymap_result_is_none() {
    let none: KeymapResult<i32> = KeymapResult::None;
    assert!(!none.is_match());
    assert!(!none.is_prefix());
    assert!(none.is_none());
}

#[test]
fn test_keymap_result_into_option() {
    let match_result: KeymapResult<i32> = KeymapResult::Match(42);
    assert_eq!(match_result.into_option(), Some(42));

    let prefix: KeymapResult<i32> = KeymapResult::Prefix;
    assert_eq!(prefix.into_option(), None);

    let none: KeymapResult<i32> = KeymapResult::None;
    assert_eq!(none.into_option(), None);
}

#[test]
fn test_keymap_result_map() {
    let result: KeymapResult<i32> = KeymapResult::Match(21);
    let mapped = result.map(|x| x * 2);
    assert_eq!(mapped, KeymapResult::Match(42));

    let prefix: KeymapResult<i32> = KeymapResult::Prefix;
    let mapped_prefix: KeymapResult<String> = prefix.map(|x| x.to_string());
    assert!(mapped_prefix.is_prefix());

    let none: KeymapResult<i32> = KeymapResult::None;
    let mapped_none: KeymapResult<String> = none.map(|x| x.to_string());
    assert!(mapped_none.is_none());
}

#[test]
fn test_keymap_result_unwrap() {
    let result: KeymapResult<i32> = KeymapResult::Match(42);
    assert_eq!(result.unwrap(), 42);
}

#[test]
#[should_panic(expected = "Prefix")]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_keymap_result_unwrap_prefix_panics() {
    let prefix: KeymapResult<i32> = KeymapResult::Prefix;
    let _ = prefix.unwrap();
}

#[test]
#[should_panic(expected = "None")]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_keymap_result_unwrap_none_panics() {
    let none: KeymapResult<i32> = KeymapResult::None;
    let _ = none.unwrap();
}
