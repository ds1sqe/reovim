//! Input event types for RPC protocol.
//!
//! These types mirror the input driver's types but with serde derives
//! for wire transmission. They enable structured input events in addition
//! to vim notation strings.
//!
//! # Input Enum
//!
//! The main [`Input`] enum represents all possible input events:
//! - [`Input::Key`] - Keyboard events
//! - [`Input::Click`] - Mouse click/drag events
//! - [`Input::Scroll`] - Mouse scroll events
//! - [`Input::Resize`] - Terminal resize events
//! - [`Input::Focus`] - Terminal focus events
//! - [`Input::Paste`] - Bracketed paste events
//! - [`Input::Attach`] - Session attach events
//! - [`Input::Detach`] - Session detach events
//! - [`Input::Ping`] - Connection health ping
//! - [`Input::Pong`] - Connection health pong

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

// =============================================================================
// Mouse Types
// =============================================================================

/// Mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    /// Left mouse button.
    Left,
    /// Right mouse button.
    Right,
    /// Middle mouse button (wheel click).
    Middle,
}

/// Click event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClickKind {
    /// Button pressed down.
    Down,
    /// Button released.
    Up,
    /// Mouse dragged with button held.
    Drag,
    /// Mouse moved (without button).
    Moved,
}

/// Mouse click event (serializable).
///
/// Represents mouse button interactions including clicks, drags, and movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClickEvent {
    /// The button involved (None for Moved events).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub button: Option<MouseButton>,
    /// Event kind (down, up, drag, moved).
    pub kind: ClickKind,
    /// Column position (0-indexed).
    pub column: u16,
    /// Row position (0-indexed).
    pub row: u16,
    /// Active modifiers.
    #[serde(default, skip_serializing_if = "Modifiers::is_empty")]
    pub modifiers: Modifiers,
}

impl ClickEvent {
    /// Create a new click event.
    #[must_use]
    pub const fn new(button: MouseButton, kind: ClickKind, column: u16, row: u16) -> Self {
        Self {
            button: Some(button),
            kind,
            column,
            row,
            modifiers: Modifiers::NONE,
        }
    }

    /// Create a moved event (no button).
    #[must_use]
    pub const fn moved(column: u16, row: u16) -> Self {
        Self {
            button: None,
            kind: ClickKind::Moved,
            column,
            row,
            modifiers: Modifiers::NONE,
        }
    }

    /// Create a drag event.
    #[must_use]
    pub const fn drag(button: MouseButton, column: u16, row: u16) -> Self {
        Self {
            button: Some(button),
            kind: ClickKind::Drag,
            column,
            row,
            modifiers: Modifiers::NONE,
        }
    }

    /// Check if this is a down event.
    #[must_use]
    pub const fn is_down(&self) -> bool {
        matches!(self.kind, ClickKind::Down)
    }

    /// Check if this is an up event.
    #[must_use]
    pub const fn is_up(&self) -> bool {
        matches!(self.kind, ClickKind::Up)
    }

    /// Check if this is a drag event.
    #[must_use]
    pub const fn is_drag(&self) -> bool {
        matches!(self.kind, ClickKind::Drag)
    }

    /// Check if this is a moved event.
    #[must_use]
    pub const fn is_moved(&self) -> bool {
        matches!(self.kind, ClickKind::Moved)
    }
}

/// Scroll direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollDirection {
    /// Scroll up.
    Up,
    /// Scroll down.
    Down,
    /// Scroll left (horizontal).
    Left,
    /// Scroll right (horizontal).
    Right,
}

/// Mouse scroll event (serializable).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScrollEvent {
    /// Scroll direction.
    pub direction: ScrollDirection,
    /// Column position (0-indexed).
    pub column: u16,
    /// Row position (0-indexed).
    pub row: u16,
    /// Active modifiers.
    #[serde(default, skip_serializing_if = "Modifiers::is_empty")]
    pub modifiers: Modifiers,
}

impl ScrollEvent {
    /// Create a new scroll event.
    #[must_use]
    pub const fn new(direction: ScrollDirection, column: u16, row: u16) -> Self {
        Self {
            direction,
            column,
            row,
            modifiers: Modifiers::NONE,
        }
    }
}

// =============================================================================
// Terminal Events
// =============================================================================

/// Terminal resize event (serializable).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResizeEvent {
    /// New width in columns.
    pub width: u16,
    /// New height in rows.
    pub height: u16,
}

impl ResizeEvent {
    /// Create a new resize event.
    #[must_use]
    pub const fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }
}

/// Focus event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FocusKind {
    /// Terminal gained focus.
    Gained,
    /// Terminal lost focus.
    Lost,
}

/// Terminal focus event (serializable).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FocusEvent {
    /// Focus kind (gained or lost).
    pub kind: FocusKind,
}

impl FocusEvent {
    /// Create a focus gained event.
    #[must_use]
    pub const fn gained() -> Self {
        Self {
            kind: FocusKind::Gained,
        }
    }

    /// Create a focus lost event.
    #[must_use]
    pub const fn lost() -> Self {
        Self {
            kind: FocusKind::Lost,
        }
    }
}

/// Paste event (serializable).
///
/// Represents bracketed paste data from the terminal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasteEvent {
    /// The pasted text content.
    pub content: String,
}

impl PasteEvent {
    /// Create a new paste event.
    #[must_use]
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
        }
    }
}

// =============================================================================
// Session Events
// =============================================================================

/// Session attach event (serializable).
///
/// Sent when a client attaches to a session. This enables multi-client
/// scenarios where multiple terminals can connect to the same editor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachEvent {
    /// Session ID to attach to (optional, server may assign).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Client identifier for tracking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    /// Requested terminal width.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u16>,
    /// Requested terminal height.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u16>,
}

impl AttachEvent {
    /// Create a new attach event.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            session_id: None,
            client_id: None,
            width: None,
            height: None,
        }
    }

    /// Create an attach event with session ID.
    #[must_use]
    pub fn with_session(session_id: impl Into<String>) -> Self {
        Self {
            session_id: Some(session_id.into()),
            client_id: None,
            width: None,
            height: None,
        }
    }

    /// Create an attach event with size.
    #[must_use]
    pub const fn with_size(width: u16, height: u16) -> Self {
        Self {
            session_id: None,
            client_id: None,
            width: Some(width),
            height: Some(height),
        }
    }
}

impl Default for AttachEvent {
    fn default() -> Self {
        Self::new()
    }
}

/// Detach reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetachReason {
    /// Normal detach (user requested).
    #[default]
    Normal,
    /// Client disconnected unexpectedly.
    Disconnected,
    /// Session is being closed.
    SessionClosed,
    /// Client was kicked by another client.
    Kicked,
}

/// Session detach event (serializable).
///
/// Sent when a client detaches from a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetachEvent {
    /// Reason for detach.
    #[serde(default)]
    pub reason: DetachReason,
    /// Optional message explaining the detach.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl DetachEvent {
    /// Create a normal detach event.
    #[must_use]
    pub const fn normal() -> Self {
        Self {
            reason: DetachReason::Normal,
            message: None,
        }
    }

    /// Create a detach event with reason.
    #[must_use]
    pub const fn with_reason(reason: DetachReason) -> Self {
        Self {
            reason,
            message: None,
        }
    }

    /// Create a detach event with message.
    #[must_use]
    pub fn with_message(reason: DetachReason, message: impl Into<String>) -> Self {
        Self {
            reason,
            message: Some(message.into()),
        }
    }
}

impl Default for DetachEvent {
    fn default() -> Self {
        Self::normal()
    }
}

// =============================================================================
// Connection Health Events
// =============================================================================

/// Ping event (serializable).
///
/// Sent by either client or server to check connection health.
/// The receiver should respond with a matching [`PongEvent`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PingEvent {
    /// Optional sequence number for matching ping/pong pairs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seq: Option<u64>,
    /// Optional timestamp (milliseconds since epoch).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<u64>,
}

impl PingEvent {
    /// Create a new ping event without sequence.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            seq: None,
            timestamp: None,
        }
    }

    /// Create a ping event with sequence number.
    #[must_use]
    pub const fn with_seq(seq: u64) -> Self {
        Self {
            seq: Some(seq),
            timestamp: None,
        }
    }

    /// Create a ping event with timestamp.
    #[must_use]
    pub const fn with_timestamp(timestamp: u64) -> Self {
        Self {
            seq: None,
            timestamp: Some(timestamp),
        }
    }
}

impl Default for PingEvent {
    fn default() -> Self {
        Self::new()
    }
}

/// Pong event (serializable).
///
/// Response to a [`PingEvent`]. Should echo the same sequence number
/// if one was provided in the ping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PongEvent {
    /// Sequence number echoed from the ping.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seq: Option<u64>,
    /// Original timestamp from the ping.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<u64>,
}

impl PongEvent {
    /// Create a new pong event without sequence.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            seq: None,
            timestamp: None,
        }
    }

    /// Create a pong event with sequence number.
    #[must_use]
    pub const fn with_seq(seq: u64) -> Self {
        Self {
            seq: Some(seq),
            timestamp: None,
        }
    }

    /// Create a pong from a ping (echoes seq and timestamp).
    #[must_use]
    pub const fn from_ping(ping: &PingEvent) -> Self {
        Self {
            seq: ping.seq,
            timestamp: ping.timestamp,
        }
    }
}

impl Default for PongEvent {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Input Enum
// =============================================================================

/// Input event (serializable).
///
/// Represents all possible input events that can be sent to the editor.
/// This is the primary type for the `input/event` RPC method.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Input {
    /// Keyboard event.
    Key(KeyEvent),
    /// Mouse click/drag event.
    Click(ClickEvent),
    /// Mouse scroll event.
    Scroll(ScrollEvent),
    /// Terminal resize event.
    Resize(ResizeEvent),
    /// Terminal focus event.
    Focus(FocusEvent),
    /// Bracketed paste event.
    Paste(PasteEvent),
    /// Session attach event.
    Attach(AttachEvent),
    /// Session detach event.
    Detach(DetachEvent),
    /// Connection health ping.
    Ping(PingEvent),
    /// Connection health pong.
    Pong(PongEvent),
}

impl Input {
    /// Create a key input event.
    #[must_use]
    pub const fn key(event: KeyEvent) -> Self {
        Self::Key(event)
    }

    /// Create a click input event.
    #[must_use]
    pub const fn click(event: ClickEvent) -> Self {
        Self::Click(event)
    }

    /// Create a scroll input event.
    #[must_use]
    pub const fn scroll(event: ScrollEvent) -> Self {
        Self::Scroll(event)
    }

    /// Create a resize input event.
    #[must_use]
    pub const fn resize(event: ResizeEvent) -> Self {
        Self::Resize(event)
    }

    /// Create a focus input event.
    #[must_use]
    pub const fn focus(event: FocusEvent) -> Self {
        Self::Focus(event)
    }

    /// Create a paste input event.
    #[must_use]
    pub const fn paste(event: PasteEvent) -> Self {
        Self::Paste(event)
    }

    /// Create an attach input event.
    #[must_use]
    pub const fn attach(event: AttachEvent) -> Self {
        Self::Attach(event)
    }

    /// Create a detach input event.
    #[must_use]
    pub const fn detach(event: DetachEvent) -> Self {
        Self::Detach(event)
    }

    /// Create a ping input event.
    #[must_use]
    pub const fn ping(event: PingEvent) -> Self {
        Self::Ping(event)
    }

    /// Create a pong input event.
    #[must_use]
    pub const fn pong(event: PongEvent) -> Self {
        Self::Pong(event)
    }

    /// Check if this is a key event.
    #[must_use]
    pub const fn is_key(&self) -> bool {
        matches!(self, Self::Key(_))
    }

    /// Check if this is a click event.
    #[must_use]
    pub const fn is_click(&self) -> bool {
        matches!(self, Self::Click(_))
    }

    /// Check if this is a scroll event.
    #[must_use]
    pub const fn is_scroll(&self) -> bool {
        matches!(self, Self::Scroll(_))
    }

    /// Check if this is a resize event.
    #[must_use]
    pub const fn is_resize(&self) -> bool {
        matches!(self, Self::Resize(_))
    }

    /// Check if this is a focus event.
    #[must_use]
    pub const fn is_focus(&self) -> bool {
        matches!(self, Self::Focus(_))
    }

    /// Check if this is a paste event.
    #[must_use]
    pub const fn is_paste(&self) -> bool {
        matches!(self, Self::Paste(_))
    }

    /// Check if this is an attach event.
    #[must_use]
    pub const fn is_attach(&self) -> bool {
        matches!(self, Self::Attach(_))
    }

    /// Check if this is a detach event.
    #[must_use]
    pub const fn is_detach(&self) -> bool {
        matches!(self, Self::Detach(_))
    }

    /// Check if this is a ping event.
    #[must_use]
    pub const fn is_ping(&self) -> bool {
        matches!(self, Self::Ping(_))
    }

    /// Check if this is a pong event.
    #[must_use]
    pub const fn is_pong(&self) -> bool {
        matches!(self, Self::Pong(_))
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

    // =========================================================================
    // Mouse Type Tests
    // =========================================================================

    #[test]
    fn test_mouse_button_serialization() {
        assert_eq!(serde_json::to_string(&MouseButton::Left).unwrap(), "\"left\"");
        assert_eq!(serde_json::to_string(&MouseButton::Right).unwrap(), "\"right\"");
        assert_eq!(serde_json::to_string(&MouseButton::Middle).unwrap(), "\"middle\"");
    }

    #[test]
    fn test_click_kind_serialization() {
        assert_eq!(serde_json::to_string(&ClickKind::Down).unwrap(), "\"down\"");
        assert_eq!(serde_json::to_string(&ClickKind::Up).unwrap(), "\"up\"");
        assert_eq!(serde_json::to_string(&ClickKind::Drag).unwrap(), "\"drag\"");
        assert_eq!(serde_json::to_string(&ClickKind::Moved).unwrap(), "\"moved\"");
    }

    #[test]
    fn test_click_event_minimal() {
        let click = ClickEvent::new(MouseButton::Left, ClickKind::Down, 10, 20);
        let json = serde_json::to_string(&click).unwrap();
        assert!(json.contains("\"button\":\"left\""));
        assert!(json.contains("\"kind\":\"down\""));
        assert!(json.contains("\"column\":10"));
        assert!(json.contains("\"row\":20"));
        // No modifiers should be serialized (skipped)
        assert!(!json.contains("\"modifiers\""));
    }

    #[test]
    fn test_click_event_moved() {
        let moved = ClickEvent::moved(5, 15);
        let json = serde_json::to_string(&moved).unwrap();
        assert!(json.contains("\"kind\":\"moved\""));
        // button should be skipped (None)
        assert!(!json.contains("\"button\""));
    }

    #[test]
    fn test_click_event_helpers() {
        let down = ClickEvent::new(MouseButton::Left, ClickKind::Down, 0, 0);
        assert!(down.is_down());
        assert!(!down.is_up());

        let up = ClickEvent::new(MouseButton::Left, ClickKind::Up, 0, 0);
        assert!(up.is_up());

        let drag = ClickEvent::drag(MouseButton::Left, 10, 20);
        assert!(drag.is_drag());
        assert_eq!(drag.button, Some(MouseButton::Left));
        assert_eq!(drag.column, 10);
        assert_eq!(drag.row, 20);

        let moved = ClickEvent::moved(0, 0);
        assert!(moved.is_moved());
    }

    #[test]
    fn test_scroll_direction_serialization() {
        assert_eq!(serde_json::to_string(&ScrollDirection::Up).unwrap(), "\"up\"");
        assert_eq!(serde_json::to_string(&ScrollDirection::Down).unwrap(), "\"down\"");
        assert_eq!(serde_json::to_string(&ScrollDirection::Left).unwrap(), "\"left\"");
        assert_eq!(serde_json::to_string(&ScrollDirection::Right).unwrap(), "\"right\"");
    }

    #[test]
    fn test_scroll_event_serialization() {
        let scroll = ScrollEvent::new(ScrollDirection::Down, 15, 25);
        let json = serde_json::to_string(&scroll).unwrap();
        assert!(json.contains("\"direction\":\"down\""));
        assert!(json.contains("\"column\":15"));
        assert!(json.contains("\"row\":25"));
        assert!(!json.contains("\"modifiers\""));
    }

    // =========================================================================
    // Terminal Event Tests
    // =========================================================================

    #[test]
    fn test_resize_event_serialization() {
        let resize = ResizeEvent::new(80, 24);
        let json = serde_json::to_string(&resize).unwrap();
        assert_eq!(json, r#"{"width":80,"height":24}"#);

        let decoded: ResizeEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.width, 80);
        assert_eq!(decoded.height, 24);
    }

    #[test]
    fn test_focus_event_serialization() {
        let gained = FocusEvent::gained();
        let json = serde_json::to_string(&gained).unwrap();
        assert_eq!(json, r#"{"kind":"gained"}"#);

        let lost = FocusEvent::lost();
        let json = serde_json::to_string(&lost).unwrap();
        assert_eq!(json, r#"{"kind":"lost"}"#);
    }

    #[test]
    fn test_paste_event_serialization() {
        let paste = PasteEvent::new("Hello, World!");
        let json = serde_json::to_string(&paste).unwrap();
        assert!(json.contains("\"content\":\"Hello, World!\""));

        let decoded: PasteEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.content, "Hello, World!");
    }

    // =========================================================================
    // Input Enum Tests
    // =========================================================================

    #[test]
    fn test_input_key_serialization() {
        let input = Input::key(KeyEvent::new(KeyCode::Char('a')));
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"type\":\"key\""));
        assert!(input.is_key());
        assert!(!input.is_click());
    }

    #[test]
    fn test_input_click_serialization() {
        let input = Input::click(ClickEvent::new(MouseButton::Left, ClickKind::Down, 10, 20));
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"type\":\"click\""));
        assert!(input.is_click());
    }

    #[test]
    fn test_input_scroll_serialization() {
        let input = Input::scroll(ScrollEvent::new(ScrollDirection::Up, 5, 10));
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"type\":\"scroll\""));
        assert!(input.is_scroll());
    }

    #[test]
    fn test_input_resize_serialization() {
        let input = Input::resize(ResizeEvent::new(120, 40));
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"type\":\"resize\""));
        assert!(json.contains("\"width\":120"));
        assert!(input.is_resize());
    }

    #[test]
    fn test_input_focus_serialization() {
        let input = Input::focus(FocusEvent::gained());
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"type\":\"focus\""));
        assert!(json.contains("\"kind\":\"gained\""));
        assert!(input.is_focus());
    }

    #[test]
    fn test_input_paste_serialization() {
        let input = Input::paste(PasteEvent::new("pasted text"));
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"type\":\"paste\""));
        assert!(json.contains("\"content\":\"pasted text\""));
        assert!(input.is_paste());
    }

    #[test]
    fn test_input_roundtrip() {
        // Key event
        let key_input = Input::key(KeyEvent::with_modifiers(
            KeyCode::Char('s'),
            Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        ));
        let json = serde_json::to_string(&key_input).unwrap();
        let decoded: Input = serde_json::from_str(&json).unwrap();
        assert!(decoded.is_key());

        // Click event
        let click_input = Input::click(ClickEvent::new(MouseButton::Right, ClickKind::Up, 5, 15));
        let json = serde_json::to_string(&click_input).unwrap();
        let decoded: Input = serde_json::from_str(&json).unwrap();
        assert!(decoded.is_click());

        // Resize event
        let resize_input = Input::resize(ResizeEvent::new(100, 50));
        let json = serde_json::to_string(&resize_input).unwrap();
        let decoded: Input = serde_json::from_str(&json).unwrap();
        assert!(decoded.is_resize());
    }

    // =========================================================================
    // Session Event Tests
    // =========================================================================

    #[test]
    fn test_attach_event_minimal() {
        let attach = AttachEvent::new();
        let json = serde_json::to_string(&attach).unwrap();
        // All None fields should be skipped
        assert_eq!(json, "{}");
    }

    #[test]
    fn test_attach_event_with_session() {
        let attach = AttachEvent::with_session("session-123");
        let json = serde_json::to_string(&attach).unwrap();
        assert!(json.contains("\"session_id\":\"session-123\""));
        assert!(!json.contains("\"client_id\""));
    }

    #[test]
    fn test_attach_event_with_size() {
        let attach = AttachEvent::with_size(80, 24);
        let json = serde_json::to_string(&attach).unwrap();
        assert!(json.contains("\"width\":80"));
        assert!(json.contains("\"height\":24"));
    }

    #[test]
    fn test_detach_event_normal() {
        let detach = DetachEvent::normal();
        let json = serde_json::to_string(&detach).unwrap();
        assert!(json.contains("\"reason\":\"normal\""));
        assert!(!json.contains("\"message\""));
    }

    #[test]
    fn test_detach_event_with_reason() {
        let detach = DetachEvent::with_reason(DetachReason::Kicked);
        let json = serde_json::to_string(&detach).unwrap();
        assert!(json.contains("\"reason\":\"kicked\""));
    }

    #[test]
    fn test_detach_event_with_message() {
        let detach = DetachEvent::with_message(DetachReason::SessionClosed, "Server shutting down");
        let json = serde_json::to_string(&detach).unwrap();
        assert!(json.contains("\"reason\":\"session_closed\""));
        assert!(json.contains("\"message\":\"Server shutting down\""));
    }

    #[test]
    fn test_detach_reason_serialization() {
        assert_eq!(serde_json::to_string(&DetachReason::Normal).unwrap(), "\"normal\"");
        assert_eq!(serde_json::to_string(&DetachReason::Disconnected).unwrap(), "\"disconnected\"");
        assert_eq!(
            serde_json::to_string(&DetachReason::SessionClosed).unwrap(),
            "\"session_closed\""
        );
        assert_eq!(serde_json::to_string(&DetachReason::Kicked).unwrap(), "\"kicked\"");
    }

    #[test]
    fn test_input_attach_serialization() {
        let input = Input::attach(AttachEvent::with_session("my-session"));
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"type\":\"attach\""));
        assert!(json.contains("\"session_id\":\"my-session\""));
        assert!(input.is_attach());
        assert!(!input.is_detach());
    }

    #[test]
    fn test_input_detach_serialization() {
        let input = Input::detach(DetachEvent::normal());
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"type\":\"detach\""));
        assert!(json.contains("\"reason\":\"normal\""));
        assert!(input.is_detach());
        assert!(!input.is_attach());
    }

    #[test]
    fn test_session_events_roundtrip() {
        // Attach event
        let attach_input = Input::attach(AttachEvent::with_size(120, 40));
        let json = serde_json::to_string(&attach_input).unwrap();
        let decoded: Input = serde_json::from_str(&json).unwrap();
        assert!(decoded.is_attach());

        // Detach event
        let detach_input = Input::detach(DetachEvent::with_reason(DetachReason::Disconnected));
        let json = serde_json::to_string(&detach_input).unwrap();
        let decoded: Input = serde_json::from_str(&json).unwrap();
        assert!(decoded.is_detach());
    }

    // =========================================================================
    // Ping/Pong Event Tests
    // =========================================================================

    #[test]
    fn test_ping_event_minimal() {
        let ping = PingEvent::new();
        let json = serde_json::to_string(&ping).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn test_ping_event_with_seq() {
        let ping = PingEvent::with_seq(42);
        let json = serde_json::to_string(&ping).unwrap();
        assert!(json.contains("\"seq\":42"));
        assert!(!json.contains("\"timestamp\""));
    }

    #[test]
    fn test_ping_event_with_timestamp() {
        let ping = PingEvent::with_timestamp(1_705_000_000_000);
        let json = serde_json::to_string(&ping).unwrap();
        assert!(json.contains("\"timestamp\":1705000000000"));
        assert!(!json.contains("\"seq\""));
    }

    #[test]
    fn test_pong_event_minimal() {
        let pong = PongEvent::new();
        let json = serde_json::to_string(&pong).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn test_pong_from_ping() {
        let source = PingEvent {
            seq: Some(123),
            timestamp: Some(1_705_000_000_000),
        };
        let response = PongEvent::from_ping(&source);
        assert_eq!(response.seq, Some(123));
        assert_eq!(response.timestamp, Some(1_705_000_000_000));
    }

    #[test]
    fn test_input_ping_serialization() {
        let input = Input::ping(PingEvent::with_seq(1));
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"type\":\"ping\""));
        assert!(json.contains("\"seq\":1"));
        assert!(input.is_ping());
        assert!(!input.is_pong());
    }

    #[test]
    fn test_input_pong_serialization() {
        let input = Input::pong(PongEvent::with_seq(1));
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"type\":\"pong\""));
        assert!(json.contains("\"seq\":1"));
        assert!(input.is_pong());
        assert!(!input.is_ping());
    }

    #[test]
    fn test_ping_pong_roundtrip() {
        // Ping event
        let health_check = Input::ping(PingEvent::with_seq(999));
        let json = serde_json::to_string(&health_check).unwrap();
        let decoded: Input = serde_json::from_str(&json).unwrap();
        assert!(decoded.is_ping());

        // Pong event
        let health_response = Input::pong(PongEvent::with_seq(999));
        let json2 = serde_json::to_string(&health_response).unwrap();
        let decoded2: Input = serde_json::from_str(&json2).unwrap();
        assert!(decoded2.is_pong());
    }
}
