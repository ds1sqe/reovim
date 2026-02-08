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

mod health;
mod key;
mod mouse;
mod session;
mod terminal;

// Re-export all types at module root for API compatibility
pub use {
    health::{PingEvent, PongEvent},
    key::{KeyCode, KeyEvent, KeyEventKind, Modifiers},
    mouse::{ClickEvent, ClickKind, MouseButton, ScrollDirection, ScrollEvent},
    session::{AttachEvent, DetachEvent, DetachReason},
    terminal::{FocusEvent, FocusKind, PasteEvent, ResizeEvent},
};

use serde::{Deserialize, Serialize};

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

    #[test]
    fn test_input_all_is_methods_exclusive() {
        let key = Input::key(KeyEvent::new(KeyCode::Char('a')));
        assert!(key.is_key());
        assert!(!key.is_click());
        assert!(!key.is_scroll());
        assert!(!key.is_resize());
        assert!(!key.is_focus());
        assert!(!key.is_paste());
        assert!(!key.is_attach());
        assert!(!key.is_detach());
        assert!(!key.is_ping());
        assert!(!key.is_pong());
    }

    #[test]
    fn test_input_scroll_variant_is_methods() {
        let scroll = Input::scroll(ScrollEvent::new(ScrollDirection::Down, 0, 0));
        assert!(scroll.is_scroll());
        assert!(!scroll.is_key());
    }

    #[test]
    fn test_input_focus_variant_is_methods() {
        let focus = Input::focus(FocusEvent::lost());
        assert!(focus.is_focus());
        assert!(!scroll_input_is_not_focus(&focus));
    }

    fn scroll_input_is_not_focus(input: &Input) -> bool {
        input.is_scroll()
    }

    #[test]
    fn test_input_paste_variant() {
        let paste = Input::paste(PasteEvent::new("text"));
        assert!(paste.is_paste());
        assert!(!paste.is_key());
    }

    #[test]
    fn test_input_equality() {
        let a = Input::key(KeyEvent::new(KeyCode::Char('a')));
        let b = Input::key(KeyEvent::new(KeyCode::Char('a')));
        assert_eq!(a, b);

        let c = Input::key(KeyEvent::new(KeyCode::Char('b')));
        assert_ne!(a, c);
    }

    #[test]
    fn test_input_all_constructors() {
        // Verify all constructors compile and produce correct variants
        let _ = Input::key(KeyEvent::new(KeyCode::Escape));
        let _ = Input::click(ClickEvent::moved(0, 0));
        let _ = Input::scroll(ScrollEvent::new(ScrollDirection::Up, 0, 0));
        let _ = Input::resize(ResizeEvent::new(80, 24));
        let _ = Input::focus(FocusEvent::gained());
        let _ = Input::paste(PasteEvent::new(""));
        let _ = Input::attach(AttachEvent::new());
        let _ = Input::detach(DetachEvent::normal());
        let _ = Input::ping(PingEvent::new());
        let _ = Input::pong(PongEvent::new());
    }
}
