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
mod tests {
    use super::*;

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
}
