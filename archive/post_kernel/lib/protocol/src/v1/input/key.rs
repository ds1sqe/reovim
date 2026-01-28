//! Key event types for RPC protocol.
//!
//! Keyboard input types with serde derives for wire transmission.

use serde::{Deserialize, Serialize};

/// Key modifiers (serializable).
///
/// Sent as an array of modifier names for JSON clarity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)] // Modifiers naturally has 6 boolean flags
pub struct Modifiers {
    /// Shift key.
    #[serde(default, skip_serializing_if = "is_false")]
    pub shift: bool,
    /// Control key.
    #[serde(default, skip_serializing_if = "is_false")]
    pub ctrl: bool,
    /// Alt/Option key.
    #[serde(default, skip_serializing_if = "is_false")]
    pub alt: bool,
    /// Super/Meta/Command key.
    #[serde(default, skip_serializing_if = "is_false")]
    pub super_key: bool,
    /// Hyper key (rare).
    #[serde(default, skip_serializing_if = "is_false")]
    pub hyper: bool,
    /// Meta key (distinct from Super on some systems).
    #[serde(default, skip_serializing_if = "is_false")]
    pub meta: bool,
}

/// Helper for serde's `skip_serializing_if`.
#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_false(b: &bool) -> bool {
    !*b
}

impl Modifiers {
    /// No modifiers.
    pub const NONE: Self = Self {
        shift: false,
        ctrl: false,
        alt: false,
        super_key: false,
        hyper: false,
        meta: false,
    };

    /// Check if any modifier is active.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        !self.shift && !self.ctrl && !self.alt && !self.super_key && !self.hyper && !self.meta
    }
}

/// Platform-agnostic key codes (serializable).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum KeyCode {
    /// A character key (covers all printable ASCII and Unicode).
    Char(char),
    /// Function key (F1-F24, stored as 1-24).
    F(u8),
    /// Up arrow.
    Up,
    /// Down arrow.
    Down,
    /// Left arrow.
    Left,
    /// Right arrow.
    Right,
    /// Home key.
    Home,
    /// End key.
    End,
    /// Page up.
    PageUp,
    /// Page down.
    PageDown,
    /// Backspace key.
    Backspace,
    /// Delete key.
    Delete,
    /// Insert key.
    Insert,
    /// Tab key.
    Tab,
    /// Shift+Tab (backtab).
    BackTab,
    /// Enter/Return key.
    Enter,
    /// Escape key.
    Escape,
    /// Null character (Ctrl+@).
    Null,
    /// Caps lock.
    CapsLock,
    /// Scroll lock.
    ScrollLock,
    /// Num lock.
    NumLock,
    /// Print screen.
    PrintScreen,
    /// Pause key.
    Pause,
    /// Menu/Application key.
    Menu,
    /// Keypad begin (center key on keypad).
    KeypadBegin,
    /// Media play.
    MediaPlay,
    /// Media pause.
    MediaPause,
    /// Media play/pause toggle.
    MediaPlayPause,
    /// Media stop.
    MediaStop,
    /// Media reverse.
    MediaReverse,
    /// Media fast forward.
    MediaFastForward,
    /// Media rewind.
    MediaRewind,
    /// Media next track.
    MediaNext,
    /// Media previous track.
    MediaPrevious,
    /// Media record.
    MediaRecord,
    /// Media lower volume.
    MediaLowerVolume,
    /// Media raise volume.
    MediaRaiseVolume,
    /// Media mute volume.
    MediaMuteVolume,
    /// Left shift key.
    LeftShift,
    /// Right shift key.
    RightShift,
    /// Left control key.
    LeftCtrl,
    /// Right control key.
    RightCtrl,
    /// Left alt key.
    LeftAlt,
    /// Right alt key.
    RightAlt,
    /// Left super/meta key.
    LeftSuper,
    /// Right super/meta key.
    RightSuper,
    /// Left hyper key.
    LeftHyper,
    /// Right hyper key.
    RightHyper,
    /// Left meta key.
    LeftMeta,
    /// Right meta key.
    RightMeta,
    /// ISO Level 3 Shift (`AltGr` on some keyboards).
    IsoLevel3Shift,
    /// ISO Level 5 Shift.
    IsoLevel5Shift,
}

/// Key event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyEventKind {
    /// Key was pressed.
    #[default]
    Press,
    /// Key is being held (repeat).
    Repeat,
    /// Key was released.
    Release,
}

/// Complete key event (serializable).
///
/// Represents a keyboard event with key code, modifiers, and event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyEvent {
    /// The key code.
    pub code: KeyCode,
    /// Active modifiers.
    #[serde(default, skip_serializing_if = "Modifiers::is_empty")]
    pub modifiers: Modifiers,
    /// Event kind (press, repeat, release).
    #[serde(default, skip_serializing_if = "is_press")]
    pub kind: KeyEventKind,
}

/// Helper for skipping default press kind.
/// Note: serde requires `&T` signature, hence the reference.
#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_press(kind: &KeyEventKind) -> bool {
    matches!(kind, KeyEventKind::Press)
}

impl KeyEvent {
    /// Create a new key event with just a key code (press, no modifiers).
    #[must_use]
    pub const fn new(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: Modifiers::NONE,
            kind: KeyEventKind::Press,
        }
    }

    /// Create a key event with modifiers (press).
    #[must_use]
    pub const fn with_modifiers(code: KeyCode, modifiers: Modifiers) -> Self {
        Self {
            code,
            modifiers,
            kind: KeyEventKind::Press,
        }
    }

    /// Check if this is a press event.
    #[must_use]
    pub const fn is_press(&self) -> bool {
        matches!(self.kind, KeyEventKind::Press)
    }

    /// Check if this is a release event.
    #[must_use]
    pub const fn is_release(&self) -> bool {
        matches!(self.kind, KeyEventKind::Release)
    }

    /// Check if this is a repeat event.
    #[must_use]
    pub const fn is_repeat(&self) -> bool {
        matches!(self.kind, KeyEventKind::Repeat)
    }
}

#[cfg(test)]
mod tests {
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
}
