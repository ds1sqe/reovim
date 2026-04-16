use crate::{
    KeyCode, KeyEvent, KeyEventKind, KeymapResult, Modifiers, MouseButton, MouseEvent,
    MouseEventKind,
};

fn all_key_codes() -> Vec<KeyCode> {
    vec![
        KeyCode::Char('a'),
        KeyCode::Char('한'),
        KeyCode::Char('\u{1f600}'),
        KeyCode::F(1),
        KeyCode::F(12),
        KeyCode::F(24),
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
    ]
}

fn all_key_kinds() -> [KeyEventKind; 3] {
    [
        KeyEventKind::Press,
        KeyEventKind::Repeat,
        KeyEventKind::Release,
    ]
}

fn all_mouse_buttons() -> [MouseButton; 3] {
    [MouseButton::Left, MouseButton::Right, MouseButton::Middle]
}

fn all_mouse_kinds() -> Vec<MouseEventKind> {
    let mut kinds = Vec::new();
    for button in all_mouse_buttons() {
        kinds.push(MouseEventKind::Down(button));
        kinds.push(MouseEventKind::Up(button));
        kinds.push(MouseEventKind::Drag(button));
    }
    kinds.extend([
        MouseEventKind::Moved,
        MouseEventKind::ScrollUp,
        MouseEventKind::ScrollDown,
        MouseEventKind::ScrollLeft,
        MouseEventKind::ScrollRight,
    ]);
    kinds
}

#[test]
fn legacy_modifier_roundtrip_is_exhaustive() {
    for bits in 0u8..(1u8 << 6) {
        let local = Modifiers::from_bits_retain(bits);
        let legacy: reovim_subsys_input::Modifiers = local.into();
        assert_eq!(legacy.bits(), bits);

        let back: Modifiers = legacy.into();
        assert_eq!(back, local);
    }
}

#[test]
fn legacy_key_code_roundtrip_is_exhaustive() {
    for local in all_key_codes() {
        let legacy: reovim_subsys_input::KeyCode = local.into();
        let back: KeyCode = legacy.into();
        assert_eq!(back, local);
    }
}

#[test]
fn legacy_key_event_kind_roundtrip_is_exhaustive() {
    for local in all_key_kinds() {
        let legacy: reovim_subsys_input::KeyEventKind = local.into();
        let back: KeyEventKind = legacy.into();
        assert_eq!(back, local);
    }
}

#[test]
fn legacy_key_event_roundtrip_is_exhaustive() {
    for code in all_key_codes() {
        for bits in 0u8..(1u8 << 6) {
            let modifiers = Modifiers::from_bits_retain(bits);
            for kind in all_key_kinds() {
                let local = KeyEvent::full(code, modifiers, kind);
                let legacy: reovim_subsys_input::KeyEvent = local.into();
                let back: KeyEvent = legacy.into();
                assert_eq!(back, local);
            }
        }
    }
}

#[test]
fn legacy_keymap_result_roundtrip_is_exhaustive() {
    for local in [
        KeymapResult::Match(String::from("action")),
        KeymapResult::Prefix,
        KeymapResult::None,
    ] {
        let legacy: reovim_subsys_input::KeymapResult<String> = local.clone().into();
        let back: KeymapResult<String> = legacy.into();
        assert_eq!(back, local);
    }
}

#[test]
fn legacy_mouse_button_roundtrip_is_exhaustive() {
    for local in all_mouse_buttons() {
        let legacy: reovim_subsys_input::MouseButton = local.into();
        let back: MouseButton = legacy.into();
        assert_eq!(back, local);
    }
}

#[test]
fn legacy_mouse_event_kind_roundtrip_is_exhaustive() {
    for local in all_mouse_kinds() {
        let legacy: reovim_subsys_input::MouseEventKind = local.into();
        let back: MouseEventKind = legacy.into();
        assert_eq!(back, local);
    }
}

#[test]
fn legacy_mouse_event_roundtrip_is_exhaustive() {
    for kind in all_mouse_kinds() {
        for &(column, row) in &[(0, 0), (1, 2), (u16::MAX, u16::MAX - 1)] {
            for bits in 0u8..(1u8 << 6) {
                let local = MouseEvent::with_modifiers(
                    kind,
                    column,
                    row,
                    Modifiers::from_bits_retain(bits),
                );
                let legacy: reovim_subsys_input::MouseEvent = local.into();
                let back: MouseEvent = legacy.into();
                assert_eq!(back, local);
            }
        }
    }
}
