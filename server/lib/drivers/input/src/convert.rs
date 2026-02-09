//! Conversions between arch types and canonical driver types.
//!
//! This module bridges the existing arch layer types with the new
//! canonical driver types. Platform code can use either, but the
//! driver types are the source of truth.

use crate::{
    key::{KeyCode, KeyEvent, KeyEventKind, Modifiers},
    mouse::{MouseButton, MouseEvent, MouseEventKind},
};

// =============================================================================
// Modifiers conversion
// =============================================================================

impl From<reovim_arch::Modifiers> for Modifiers {
    fn from(m: reovim_arch::Modifiers) -> Self {
        let mut result = Self::empty();
        if m.contains(reovim_arch::Modifiers::SHIFT) {
            result |= Self::SHIFT;
        }
        if m.contains(reovim_arch::Modifiers::CTRL) {
            result |= Self::CTRL;
        }
        if m.contains(reovim_arch::Modifiers::ALT) {
            result |= Self::ALT;
        }
        if m.contains(reovim_arch::Modifiers::SUPER) {
            result |= Self::SUPER;
        }
        if m.contains(reovim_arch::Modifiers::HYPER) {
            result |= Self::HYPER;
        }
        if m.contains(reovim_arch::Modifiers::META) {
            result |= Self::META;
        }
        result
    }
}

impl From<Modifiers> for reovim_arch::Modifiers {
    fn from(m: Modifiers) -> Self {
        let mut result = Self::NONE;
        if m.contains(Modifiers::SHIFT) {
            result = result.union(Self::SHIFT);
        }
        if m.contains(Modifiers::CTRL) {
            result = result.union(Self::CTRL);
        }
        if m.contains(Modifiers::ALT) {
            result = result.union(Self::ALT);
        }
        if m.contains(Modifiers::SUPER) {
            result = result.union(Self::SUPER);
        }
        if m.contains(Modifiers::HYPER) {
            result = result.union(Self::HYPER);
        }
        if m.contains(Modifiers::META) {
            result = result.union(Self::META);
        }
        result
    }
}

// =============================================================================
// KeyCode conversion
// =============================================================================

impl From<reovim_arch::KeyCode> for KeyCode {
    fn from(k: reovim_arch::KeyCode) -> Self {
        use reovim_arch::KeyCode as AK;
        match k {
            AK::Char(c) => Self::Char(c),
            AK::F(n) => Self::F(n),
            AK::Backspace => Self::Backspace,
            AK::Enter => Self::Enter,
            AK::Tab => Self::Tab,
            AK::BackTab => Self::BackTab,
            AK::Escape => Self::Escape,
            AK::Up => Self::Up,
            AK::Down => Self::Down,
            AK::Left => Self::Left,
            AK::Right => Self::Right,
            AK::Home => Self::Home,
            AK::End => Self::End,
            AK::PageUp => Self::PageUp,
            AK::PageDown => Self::PageDown,
            AK::Insert => Self::Insert,
            AK::Delete => Self::Delete,
            AK::Null => Self::Null,
            AK::CapsLock => Self::CapsLock,
            AK::ScrollLock => Self::ScrollLock,
            AK::NumLock => Self::NumLock,
            AK::PrintScreen => Self::PrintScreen,
            AK::Pause => Self::Pause,
            AK::Menu => Self::Menu,
            AK::KeypadBegin => Self::KeypadBegin,
            AK::MediaPlay => Self::MediaPlay,
            AK::MediaPause => Self::MediaPause,
            AK::MediaPlayPause => Self::MediaPlayPause,
            AK::MediaStop => Self::MediaStop,
            AK::MediaReverse => Self::MediaReverse,
            AK::MediaFastForward => Self::MediaFastForward,
            AK::MediaRewind => Self::MediaRewind,
            AK::MediaNext => Self::MediaNext,
            AK::MediaPrevious => Self::MediaPrevious,
            AK::MediaRecord => Self::MediaRecord,
            AK::MediaLowerVolume => Self::MediaLowerVolume,
            AK::MediaRaiseVolume => Self::MediaRaiseVolume,
            AK::MediaMuteVolume => Self::MediaMuteVolume,
            AK::LeftShift => Self::LeftShift,
            AK::RightShift => Self::RightShift,
            AK::LeftCtrl => Self::LeftCtrl,
            AK::RightCtrl => Self::RightCtrl,
            AK::LeftAlt => Self::LeftAlt,
            AK::RightAlt => Self::RightAlt,
            AK::LeftSuper => Self::LeftSuper,
            AK::RightSuper => Self::RightSuper,
            AK::LeftHyper => Self::LeftHyper,
            AK::RightHyper => Self::RightHyper,
            AK::LeftMeta => Self::LeftMeta,
            AK::RightMeta => Self::RightMeta,
            AK::IsoLevel3Shift => Self::IsoLevel3Shift,
            AK::IsoLevel5Shift => Self::IsoLevel5Shift,
        }
    }
}

impl From<KeyCode> for reovim_arch::KeyCode {
    fn from(k: KeyCode) -> Self {
        match k {
            KeyCode::Char(c) => Self::Char(c),
            KeyCode::F(n) => Self::F(n),
            KeyCode::Backspace => Self::Backspace,
            KeyCode::Enter => Self::Enter,
            KeyCode::Tab => Self::Tab,
            KeyCode::BackTab => Self::BackTab,
            KeyCode::Escape => Self::Escape,
            KeyCode::Up => Self::Up,
            KeyCode::Down => Self::Down,
            KeyCode::Left => Self::Left,
            KeyCode::Right => Self::Right,
            KeyCode::Home => Self::Home,
            KeyCode::End => Self::End,
            KeyCode::PageUp => Self::PageUp,
            KeyCode::PageDown => Self::PageDown,
            KeyCode::Insert => Self::Insert,
            KeyCode::Delete => Self::Delete,
            KeyCode::Null => Self::Null,
            KeyCode::CapsLock => Self::CapsLock,
            KeyCode::ScrollLock => Self::ScrollLock,
            KeyCode::NumLock => Self::NumLock,
            KeyCode::PrintScreen => Self::PrintScreen,
            KeyCode::Pause => Self::Pause,
            KeyCode::Menu => Self::Menu,
            KeyCode::KeypadBegin => Self::KeypadBegin,
            KeyCode::MediaPlay => Self::MediaPlay,
            KeyCode::MediaPause => Self::MediaPause,
            KeyCode::MediaPlayPause => Self::MediaPlayPause,
            KeyCode::MediaStop => Self::MediaStop,
            KeyCode::MediaReverse => Self::MediaReverse,
            KeyCode::MediaFastForward => Self::MediaFastForward,
            KeyCode::MediaRewind => Self::MediaRewind,
            KeyCode::MediaNext => Self::MediaNext,
            KeyCode::MediaPrevious => Self::MediaPrevious,
            KeyCode::MediaRecord => Self::MediaRecord,
            KeyCode::MediaLowerVolume => Self::MediaLowerVolume,
            KeyCode::MediaRaiseVolume => Self::MediaRaiseVolume,
            KeyCode::MediaMuteVolume => Self::MediaMuteVolume,
            KeyCode::LeftShift => Self::LeftShift,
            KeyCode::RightShift => Self::RightShift,
            KeyCode::LeftCtrl => Self::LeftCtrl,
            KeyCode::RightCtrl => Self::RightCtrl,
            KeyCode::LeftAlt => Self::LeftAlt,
            KeyCode::RightAlt => Self::RightAlt,
            KeyCode::LeftSuper => Self::LeftSuper,
            KeyCode::RightSuper => Self::RightSuper,
            KeyCode::LeftHyper => Self::LeftHyper,
            KeyCode::RightHyper => Self::RightHyper,
            KeyCode::LeftMeta => Self::LeftMeta,
            KeyCode::RightMeta => Self::RightMeta,
            KeyCode::IsoLevel3Shift => Self::IsoLevel3Shift,
            KeyCode::IsoLevel5Shift => Self::IsoLevel5Shift,
        }
    }
}

// =============================================================================
// KeyEventKind conversion
// =============================================================================

impl From<reovim_arch::KeyEventKind> for KeyEventKind {
    fn from(k: reovim_arch::KeyEventKind) -> Self {
        match k {
            reovim_arch::KeyEventKind::Press => Self::Press,
            reovim_arch::KeyEventKind::Repeat => Self::Repeat,
            reovim_arch::KeyEventKind::Release => Self::Release,
        }
    }
}

impl From<KeyEventKind> for reovim_arch::KeyEventKind {
    fn from(k: KeyEventKind) -> Self {
        match k {
            KeyEventKind::Press => Self::Press,
            KeyEventKind::Repeat => Self::Repeat,
            KeyEventKind::Release => Self::Release,
        }
    }
}

// =============================================================================
// KeyEvent conversion
// =============================================================================

impl From<reovim_arch::KeyEvent> for KeyEvent {
    fn from(e: reovim_arch::KeyEvent) -> Self {
        Self {
            code: e.code.into(),
            modifiers: e.modifiers.into(),
            kind: e.kind.into(),
        }
    }
}

impl From<KeyEvent> for reovim_arch::KeyEvent {
    fn from(e: KeyEvent) -> Self {
        Self {
            code: e.code.into(),
            modifiers: e.modifiers.into(),
            kind: e.kind.into(),
            state: reovim_arch::KeyEventState::NONE,
        }
    }
}

// =============================================================================
// MouseButton conversion
// =============================================================================

impl From<reovim_arch::MouseButton> for MouseButton {
    fn from(b: reovim_arch::MouseButton) -> Self {
        match b {
            reovim_arch::MouseButton::Left => Self::Left,
            reovim_arch::MouseButton::Right => Self::Right,
            reovim_arch::MouseButton::Middle => Self::Middle,
        }
    }
}

impl From<MouseButton> for reovim_arch::MouseButton {
    fn from(b: MouseButton) -> Self {
        match b {
            MouseButton::Left => Self::Left,
            MouseButton::Right => Self::Right,
            MouseButton::Middle => Self::Middle,
        }
    }
}

// =============================================================================
// MouseEventKind conversion
// =============================================================================

impl From<reovim_arch::MouseEventKind> for MouseEventKind {
    fn from(k: reovim_arch::MouseEventKind) -> Self {
        use reovim_arch::MouseEventKind as AMK;
        match k {
            AMK::Down(b) => Self::Down(b.into()),
            AMK::Up(b) => Self::Up(b.into()),
            AMK::Drag(b) => Self::Drag(b.into()),
            AMK::Moved => Self::Moved,
            AMK::ScrollUp => Self::ScrollUp,
            AMK::ScrollDown => Self::ScrollDown,
            AMK::ScrollLeft => Self::ScrollLeft,
            AMK::ScrollRight => Self::ScrollRight,
        }
    }
}

impl From<MouseEventKind> for reovim_arch::MouseEventKind {
    fn from(k: MouseEventKind) -> Self {
        match k {
            MouseEventKind::Down(b) => Self::Down(b.into()),
            MouseEventKind::Up(b) => Self::Up(b.into()),
            MouseEventKind::Drag(b) => Self::Drag(b.into()),
            MouseEventKind::Moved => Self::Moved,
            MouseEventKind::ScrollUp => Self::ScrollUp,
            MouseEventKind::ScrollDown => Self::ScrollDown,
            MouseEventKind::ScrollLeft => Self::ScrollLeft,
            MouseEventKind::ScrollRight => Self::ScrollRight,
        }
    }
}

// =============================================================================
// MouseEvent conversion
// =============================================================================

impl From<reovim_arch::MouseEvent> for MouseEvent {
    fn from(e: reovim_arch::MouseEvent) -> Self {
        Self {
            kind: e.kind.into(),
            column: e.column,
            row: e.row,
            modifiers: e.modifiers.into(),
        }
    }
}

impl From<MouseEvent> for reovim_arch::MouseEvent {
    fn from(e: MouseEvent) -> Self {
        Self {
            kind: e.kind.into(),
            column: e.column,
            row: e.row,
            modifiers: e.modifiers.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
