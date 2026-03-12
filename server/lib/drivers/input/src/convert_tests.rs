use crate::{KeyCode, KeyEvent, KeyEventKind, Modifiers, MouseButton, MouseEvent, MouseEventKind};

// ========================================================================
// Modifiers conversion tests
// ========================================================================

#[test]
fn test_modifiers_roundtrip() {
    let driver_mods = Modifiers::CTRL | Modifiers::SHIFT;
    let arch_mods: reovim_arch::Modifiers = driver_mods.into();
    let back: Modifiers = arch_mods.into();
    assert_eq!(driver_mods, back);
}

#[test]
fn test_modifiers_all_flags_roundtrip() {
    let driver_mods = Modifiers::SHIFT
        | Modifiers::CTRL
        | Modifiers::ALT
        | Modifiers::SUPER
        | Modifiers::HYPER
        | Modifiers::META;
    let arch_mods: reovim_arch::Modifiers = driver_mods.into();
    let back: Modifiers = arch_mods.into();
    assert_eq!(driver_mods, back);
}

#[test]
fn test_modifiers_empty_roundtrip() {
    let driver_mods = Modifiers::NONE;
    let arch_mods: reovim_arch::Modifiers = driver_mods.into();
    let back: Modifiers = arch_mods.into();
    assert_eq!(driver_mods, back);
    assert!(back.is_empty());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_modifiers_individual_flags_to_arch() {
    // Test each individual flag converts correctly
    let pairs = [
        (Modifiers::SHIFT, reovim_arch::Modifiers::SHIFT),
        (Modifiers::CTRL, reovim_arch::Modifiers::CTRL),
        (Modifiers::ALT, reovim_arch::Modifiers::ALT),
        (Modifiers::SUPER, reovim_arch::Modifiers::SUPER),
        (Modifiers::HYPER, reovim_arch::Modifiers::HYPER),
        (Modifiers::META, reovim_arch::Modifiers::META),
    ];

    for (driver, expected_arch) in pairs {
        let arch: reovim_arch::Modifiers = driver.into();
        assert!(
            arch.contains(expected_arch),
            "Driver {driver:?} did not convert to arch {expected_arch:?}"
        );
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_modifiers_individual_flags_from_arch() {
    // Test each individual flag converts correctly from arch
    let pairs = [
        (reovim_arch::Modifiers::SHIFT, Modifiers::SHIFT),
        (reovim_arch::Modifiers::CTRL, Modifiers::CTRL),
        (reovim_arch::Modifiers::ALT, Modifiers::ALT),
        (reovim_arch::Modifiers::SUPER, Modifiers::SUPER),
        (reovim_arch::Modifiers::HYPER, Modifiers::HYPER),
        (reovim_arch::Modifiers::META, Modifiers::META),
    ];

    for (arch, expected_driver) in pairs {
        let driver: Modifiers = arch.into();
        assert!(
            driver.contains(expected_driver),
            "Arch {arch:?} did not convert to driver {expected_driver:?}"
        );
    }
}

// ========================================================================
// KeyCode conversion tests
// ========================================================================

#[test]
fn test_key_code_roundtrip() {
    let codes = [
        KeyCode::Char('a'),
        KeyCode::F(12),
        KeyCode::Enter,
        KeyCode::Escape,
        KeyCode::Up,
        KeyCode::Home,
        KeyCode::MediaPlay,
        KeyCode::LeftCtrl,
    ];

    for code in codes {
        let arch_code: reovim_arch::KeyCode = code.into();
        let back: KeyCode = arch_code.into();
        assert_eq!(code, back);
    }
}

#[test]
fn test_key_code_all_navigation_roundtrip() {
    let codes = [
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::Home,
        KeyCode::End,
        KeyCode::PageUp,
        KeyCode::PageDown,
    ];

    for code in codes {
        let arch_code: reovim_arch::KeyCode = code.into();
        let back: KeyCode = arch_code.into();
        assert_eq!(code, back);
    }
}

#[test]
fn test_key_code_all_editing_roundtrip() {
    let codes = [
        KeyCode::Backspace,
        KeyCode::Delete,
        KeyCode::Insert,
        KeyCode::Tab,
        KeyCode::BackTab,
        KeyCode::Enter,
        KeyCode::Escape,
    ];

    for code in codes {
        let arch_code: reovim_arch::KeyCode = code.into();
        let back: KeyCode = arch_code.into();
        assert_eq!(code, back);
    }
}

#[test]
fn test_key_code_all_special_roundtrip() {
    let codes = [
        KeyCode::Null,
        KeyCode::CapsLock,
        KeyCode::ScrollLock,
        KeyCode::NumLock,
        KeyCode::PrintScreen,
        KeyCode::Pause,
        KeyCode::Menu,
        KeyCode::KeypadBegin,
    ];

    for code in codes {
        let arch_code: reovim_arch::KeyCode = code.into();
        let back: KeyCode = arch_code.into();
        assert_eq!(code, back);
    }
}

#[test]
fn test_key_code_all_media_roundtrip() {
    let codes = [
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

    for code in codes {
        let arch_code: reovim_arch::KeyCode = code.into();
        let back: KeyCode = arch_code.into();
        assert_eq!(code, back);
    }
}

#[test]
fn test_key_code_all_modifier_keys_roundtrip() {
    let codes = [
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

    for code in codes {
        let arch_code: reovim_arch::KeyCode = code.into();
        let back: KeyCode = arch_code.into();
        assert_eq!(code, back);
    }
}

#[test]
fn test_key_code_function_keys_roundtrip() {
    for n in 1..=24 {
        let code = KeyCode::F(n);
        let arch_code: reovim_arch::KeyCode = code.into();
        let back: KeyCode = arch_code.into();
        assert_eq!(code, back);
    }
}

#[test]
fn test_key_code_char_unicode_roundtrip() {
    let chars = ['a', 'Z', '0', ' ', '\n', '\t', '\u{00e9}', '\u{1f600}'];
    for c in chars {
        let code = KeyCode::Char(c);
        let arch_code: reovim_arch::KeyCode = code.into();
        let back: KeyCode = arch_code.into();
        assert_eq!(code, back);
    }
}

// ========================================================================
// KeyEvent conversion tests
// ========================================================================

#[test]
fn test_key_event_roundtrip() {
    let driver_event = KeyEvent::with_modifiers(KeyCode::Char('a'), Modifiers::CTRL);
    let arch_event: reovim_arch::KeyEvent = driver_event.into();
    let back: KeyEvent = arch_event.into();
    assert_eq!(driver_event.code, back.code);
    assert_eq!(driver_event.modifiers, back.modifiers);
    assert_eq!(driver_event.kind, back.kind);
}

#[test]
fn test_key_event_from_arch_preserves_kind() {
    let arch_event = reovim_arch::KeyEvent {
        code: reovim_arch::KeyCode::Char('x'),
        modifiers: reovim_arch::Modifiers::NONE,
        kind: reovim_arch::KeyEventKind::Repeat,
        state: reovim_arch::KeyEventState::NONE,
    };
    let driver_event: KeyEvent = arch_event.into();
    assert_eq!(driver_event.kind, KeyEventKind::Repeat);
}

#[test]
fn test_key_event_to_arch_sets_state_none() {
    let driver_event = KeyEvent::new(KeyCode::Char('a'));
    let arch_event: reovim_arch::KeyEvent = driver_event.into();
    assert_eq!(arch_event.state, reovim_arch::KeyEventState::NONE);
}

// ========================================================================
// KeyEventKind conversion tests
// ========================================================================

#[test]
fn test_key_event_kind_roundtrip() {
    let kinds = [
        KeyEventKind::Press,
        KeyEventKind::Repeat,
        KeyEventKind::Release,
    ];

    for kind in kinds {
        let arch_kind: reovim_arch::KeyEventKind = kind.into();
        let back: KeyEventKind = arch_kind.into();
        assert_eq!(kind, back);
    }
}

// ========================================================================
// MouseButton conversion tests
// ========================================================================

#[test]
fn test_mouse_button_roundtrip() {
    let buttons = [MouseButton::Left, MouseButton::Right, MouseButton::Middle];

    for button in buttons {
        let arch_button: reovim_arch::MouseButton = button.into();
        let back: MouseButton = arch_button.into();
        assert_eq!(button, back);
    }
}

// ========================================================================
// MouseEvent conversion tests
// ========================================================================

#[test]
fn test_mouse_event_roundtrip() {
    let driver_event = MouseEvent::new(MouseEventKind::Down(MouseButton::Left), 10, 20);
    let arch_event: reovim_arch::MouseEvent = driver_event.into();
    let back: MouseEvent = arch_event.into();
    assert_eq!(driver_event, back);
}

#[test]
fn test_mouse_event_with_modifiers_roundtrip() {
    let driver_event = MouseEvent::with_modifiers(
        MouseEventKind::Down(MouseButton::Right),
        5,
        15,
        Modifiers::CTRL | Modifiers::SHIFT,
    );
    let arch_event: reovim_arch::MouseEvent = driver_event.into();
    let back: MouseEvent = arch_event.into();
    assert_eq!(driver_event, back);
}

#[test]
fn test_mouse_event_preserves_coordinates() {
    let driver_event = MouseEvent::new(MouseEventKind::Moved, 100, 200);
    let arch_event: reovim_arch::MouseEvent = driver_event.into();
    assert_eq!(arch_event.column, 100);
    assert_eq!(arch_event.row, 200);

    let back: MouseEvent = arch_event.into();
    assert_eq!(back.column, 100);
    assert_eq!(back.row, 200);
}

// ========================================================================
// MouseEventKind conversion tests
// ========================================================================

#[test]
fn test_mouse_event_kind_roundtrip() {
    let kinds = [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Right),
        MouseEventKind::Drag(MouseButton::Middle),
        MouseEventKind::Moved,
        MouseEventKind::ScrollUp,
        MouseEventKind::ScrollDown,
        MouseEventKind::ScrollLeft,
        MouseEventKind::ScrollRight,
    ];

    for kind in kinds {
        let arch_kind: reovim_arch::MouseEventKind = kind.into();
        let back: MouseEventKind = arch_kind.into();
        assert_eq!(kind, back);
    }
}

#[test]
fn test_mouse_event_kind_all_buttons_for_down() {
    for button in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
        let kind = MouseEventKind::Down(button);
        let arch_kind: reovim_arch::MouseEventKind = kind.into();
        let back: MouseEventKind = arch_kind.into();
        assert_eq!(kind, back);
    }
}

#[test]
fn test_mouse_event_kind_all_buttons_for_up() {
    for button in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
        let kind = MouseEventKind::Up(button);
        let arch_kind: reovim_arch::MouseEventKind = kind.into();
        let back: MouseEventKind = arch_kind.into();
        assert_eq!(kind, back);
    }
}

#[test]
fn test_mouse_event_kind_all_buttons_for_drag() {
    for button in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
        let kind = MouseEventKind::Drag(button);
        let arch_kind: reovim_arch::MouseEventKind = kind.into();
        let back: MouseEventKind = arch_kind.into();
        assert_eq!(kind, back);
    }
}
