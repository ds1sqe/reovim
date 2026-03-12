use super::*;

#[test]
fn test_key_code_char_serialization() {
    let key = KeyCode::Char('a');
    let json = serde_json::to_string(&key).unwrap();
    assert_eq!(json, r#"{"type":"Char","value":"a"}"#);

    let decoded: KeyCode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, key);
}

#[test]
fn test_key_code_special_serialization() {
    let key = KeyCode::Escape;
    let json = serde_json::to_string(&key).unwrap();
    assert_eq!(json, r#"{"type":"Escape"}"#);

    let decoded: KeyCode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, key);
}

#[test]
fn test_key_code_function_serialization() {
    let key = KeyCode::F(12);
    let json = serde_json::to_string(&key).unwrap();
    assert_eq!(json, r#"{"type":"F","value":12}"#);

    let decoded: KeyCode = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, key);
}

#[test]
fn test_modifiers_empty_serialization() {
    let mods = Modifiers::NONE;
    let json = serde_json::to_string(&mods).unwrap();
    // All false fields should be skipped
    assert_eq!(json, "{}");
}

#[test]
fn test_modifiers_with_values() {
    let mods = Modifiers {
        ctrl: true,
        shift: true,
        ..Modifiers::NONE
    };
    let json = serde_json::to_string(&mods).unwrap();
    assert!(json.contains("\"ctrl\":true"));
    assert!(json.contains("\"shift\":true"));
    assert!(!json.contains("\"alt\""));

    let decoded: Modifiers = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, mods);
}

#[test]
fn test_key_event_minimal_serialization() {
    let event = KeyEvent::new(KeyCode::Char('j'));
    let json = serde_json::to_string(&event).unwrap();
    // Should only have code, no modifiers or kind (defaults skipped)
    assert!(json.contains("\"code\""));
    assert!(!json.contains("\"modifiers\""));
    assert!(!json.contains("\"kind\""));
}

#[test]
fn test_key_event_with_modifiers() {
    let event = KeyEvent::with_modifiers(
        KeyCode::Char('w'),
        Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        },
    );
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"ctrl\":true"));
}

#[test]
fn test_key_event_roundtrip() {
    let event = KeyEvent {
        code: KeyCode::F(5),
        modifiers: Modifiers {
            alt: true,
            shift: true,
            ..Modifiers::NONE
        },
        kind: KeyEventKind::Release,
    };
    let json = serde_json::to_string(&event).unwrap();
    let decoded: KeyEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.code, event.code);
    assert_eq!(decoded.modifiers, event.modifiers);
    assert_eq!(decoded.kind, event.kind);
}

#[test]
fn test_key_event_kind_serialization() {
    assert_eq!(serde_json::to_string(&KeyEventKind::Press).unwrap(), "\"press\"");
    assert_eq!(serde_json::to_string(&KeyEventKind::Release).unwrap(), "\"release\"");
    assert_eq!(serde_json::to_string(&KeyEventKind::Repeat).unwrap(), "\"repeat\"");
}

#[test]
fn test_modifiers_is_empty() {
    assert!(Modifiers::NONE.is_empty());
    assert!(Modifiers::default().is_empty());

    let with_ctrl = Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    };
    assert!(!with_ctrl.is_empty());

    let with_alt = Modifiers {
        alt: true,
        ..Modifiers::NONE
    };
    assert!(!with_alt.is_empty());

    let with_super = Modifiers {
        super_key: true,
        ..Modifiers::NONE
    };
    assert!(!with_super.is_empty());

    let with_hyper = Modifiers {
        hyper: true,
        ..Modifiers::NONE
    };
    assert!(!with_hyper.is_empty());

    let with_meta = Modifiers {
        meta: true,
        ..Modifiers::NONE
    };
    assert!(!with_meta.is_empty());
}

#[test]
fn test_key_event_is_press() {
    let press = KeyEvent::new(KeyCode::Char('a'));
    assert!(press.is_press());
    assert!(!press.is_release());
    assert!(!press.is_repeat());
}

#[test]
fn test_key_event_is_release() {
    let release = KeyEvent {
        code: KeyCode::Char('a'),
        modifiers: Modifiers::NONE,
        kind: KeyEventKind::Release,
    };
    assert!(release.is_release());
    assert!(!release.is_press());
}

#[test]
fn test_key_event_is_repeat() {
    let repeat = KeyEvent {
        code: KeyCode::Char('a'),
        modifiers: Modifiers::NONE,
        kind: KeyEventKind::Repeat,
    };
    assert!(repeat.is_repeat());
    assert!(!repeat.is_press());
}

#[test]
fn test_key_event_kind_default() {
    let kind = KeyEventKind::default();
    assert!(matches!(kind, KeyEventKind::Press));
}

#[test]
fn test_key_code_all_special_keys_serialization() {
    // Test a variety of special keys round-trip correctly
    let keys = [
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::Home,
        KeyCode::End,
        KeyCode::PageUp,
        KeyCode::PageDown,
        KeyCode::Backspace,
        KeyCode::Delete,
        KeyCode::Insert,
        KeyCode::Tab,
        KeyCode::BackTab,
        KeyCode::Enter,
        KeyCode::Escape,
        KeyCode::Null,
        KeyCode::CapsLock,
        KeyCode::ScrollLock,
        KeyCode::NumLock,
        KeyCode::PrintScreen,
        KeyCode::Pause,
        KeyCode::Menu,
        KeyCode::KeypadBegin,
    ];
    for key in keys {
        let json = serde_json::to_string(&key).unwrap();
        let decoded: KeyCode = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, key);
    }
}

#[test]
fn test_key_code_media_keys_serialization() {
    let keys = [
        KeyCode::MediaPlay,
        KeyCode::MediaPause,
        KeyCode::MediaPlayPause,
        KeyCode::MediaStop,
        KeyCode::MediaReverse,
        KeyCode::MediaFastForward,
        KeyCode::MediaRewind,
        KeyCode::MediaNext,
        KeyCode::MediaPrevious,
        KeyCode::MediaRecord,
        KeyCode::MediaLowerVolume,
        KeyCode::MediaRaiseVolume,
        KeyCode::MediaMuteVolume,
    ];
    for key in keys {
        let json = serde_json::to_string(&key).unwrap();
        let decoded: KeyCode = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, key);
    }
}

#[test]
fn test_key_code_modifier_keys_serialization() {
    let keys = [
        KeyCode::LeftShift,
        KeyCode::RightShift,
        KeyCode::LeftCtrl,
        KeyCode::RightCtrl,
        KeyCode::LeftAlt,
        KeyCode::RightAlt,
        KeyCode::LeftSuper,
        KeyCode::RightSuper,
        KeyCode::LeftHyper,
        KeyCode::RightHyper,
        KeyCode::LeftMeta,
        KeyCode::RightMeta,
        KeyCode::IsoLevel3Shift,
        KeyCode::IsoLevel5Shift,
    ];
    for key in keys {
        let json = serde_json::to_string(&key).unwrap();
        let decoded: KeyCode = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, key);
    }
}

#[test]
fn test_modifiers_all_flags() {
    let mods = Modifiers {
        shift: true,
        ctrl: true,
        alt: true,
        super_key: true,
        hyper: true,
        meta: true,
    };
    assert!(!mods.is_empty());
    let json = serde_json::to_string(&mods).unwrap();
    assert!(json.contains("\"shift\":true"));
    assert!(json.contains("\"ctrl\":true"));
    assert!(json.contains("\"alt\":true"));
    assert!(json.contains("\"super_key\":true"));
    assert!(json.contains("\"hyper\":true"));
    assert!(json.contains("\"meta\":true"));
}

#[test]
fn test_key_event_with_repeat_kind_serialization() {
    let event = KeyEvent {
        code: KeyCode::Char('x'),
        modifiers: Modifiers::NONE,
        kind: KeyEventKind::Repeat,
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"kind\":\"repeat\""));
}
