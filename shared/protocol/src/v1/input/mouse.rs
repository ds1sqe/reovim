//! Mouse event types for RPC protocol.
//!
//! Mouse input types with serde derives for wire transmission.

use serde::{Deserialize, Serialize};

use super::key::Modifiers;

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

#[cfg(test)]
#[path = "mouse_tests.rs"]
mod tests;
