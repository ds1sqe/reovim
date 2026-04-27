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
#[path = "mod_tests.rs"]
mod tests;
