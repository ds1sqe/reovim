//! Canonical mouse input types.
//!
//! These types are the source of truth for mouse input across all platforms.
//! Platform-specific code converts native events to these canonical types.

use crate::key::Modifiers;

/// Mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    /// Left mouse button.
    Left,
    /// Right mouse button.
    Right,
    /// Middle mouse button (wheel click).
    Middle,
}

/// Mouse event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseEventKind {
    /// Button pressed down.
    Down(MouseButton),
    /// Button released.
    Up(MouseButton),
    /// Mouse dragged with button held.
    Drag(MouseButton),
    /// Mouse moved (without button).
    Moved,
    /// Scroll up.
    ScrollUp,
    /// Scroll down.
    ScrollDown,
    /// Scroll left (horizontal).
    ScrollLeft,
    /// Scroll right (horizontal).
    ScrollRight,
}

/// Mouse event with position and modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseEvent {
    /// The event kind.
    pub kind: MouseEventKind,
    /// Column (0-based).
    pub column: u16,
    /// Row (0-based).
    pub row: u16,
    /// Active modifiers.
    pub modifiers: Modifiers,
}

impl MouseEvent {
    /// Create a new mouse event.
    #[must_use]
    pub const fn new(kind: MouseEventKind, column: u16, row: u16) -> Self {
        Self {
            kind,
            column,
            row,
            modifiers: Modifiers::NONE,
        }
    }

    /// Create a mouse event with modifiers.
    #[must_use]
    pub const fn with_modifiers(
        kind: MouseEventKind,
        column: u16,
        row: u16,
        modifiers: Modifiers,
    ) -> Self {
        Self {
            kind,
            column,
            row,
            modifiers,
        }
    }

    /// Check if this is a click event (down).
    #[must_use]
    pub const fn is_down(&self) -> bool {
        matches!(self.kind, MouseEventKind::Down(_))
    }

    /// Check if this is a release event (up).
    #[must_use]
    pub const fn is_up(&self) -> bool {
        matches!(self.kind, MouseEventKind::Up(_))
    }

    /// Check if this is a scroll event.
    #[must_use]
    pub const fn is_scroll(&self) -> bool {
        matches!(
            self.kind,
            MouseEventKind::ScrollUp
                | MouseEventKind::ScrollDown
                | MouseEventKind::ScrollLeft
                | MouseEventKind::ScrollRight
        )
    }

    /// Check if this is a drag event.
    #[must_use]
    pub const fn is_drag(&self) -> bool {
        matches!(self.kind, MouseEventKind::Drag(_))
    }

    /// Check if this is a move event.
    #[must_use]
    pub const fn is_moved(&self) -> bool {
        matches!(self.kind, MouseEventKind::Moved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mouse_event_creation() {
        let event = MouseEvent::new(MouseEventKind::Down(MouseButton::Left), 10, 20);
        assert_eq!(event.column, 10);
        assert_eq!(event.row, 20);
        assert!(event.is_down());
        assert!(!event.is_up());
        assert!(!event.is_scroll());
        assert!(!event.is_drag());
        assert!(!event.is_moved());
    }

    #[test]
    fn test_mouse_event_with_modifiers() {
        let event = MouseEvent::with_modifiers(
            MouseEventKind::Down(MouseButton::Left),
            5,
            10,
            Modifiers::CTRL | Modifiers::SHIFT,
        );
        assert!(event.modifiers.contains(Modifiers::CTRL));
        assert!(event.modifiers.contains(Modifiers::SHIFT));
    }

    #[test]
    fn test_mouse_scroll() {
        let scroll_up = MouseEvent::new(MouseEventKind::ScrollUp, 0, 0);
        assert!(scroll_up.is_scroll());
        assert!(!scroll_up.is_down());

        let scroll_down = MouseEvent::new(MouseEventKind::ScrollDown, 0, 0);
        assert!(scroll_down.is_scroll());

        let scroll_left = MouseEvent::new(MouseEventKind::ScrollLeft, 0, 0);
        assert!(scroll_left.is_scroll());

        let scroll_right = MouseEvent::new(MouseEventKind::ScrollRight, 0, 0);
        assert!(scroll_right.is_scroll());
    }

    #[test]
    fn test_mouse_up() {
        let event = MouseEvent::new(MouseEventKind::Up(MouseButton::Right), 0, 0);
        assert!(event.is_up());
        assert!(!event.is_down());
    }

    #[test]
    fn test_mouse_drag() {
        let event = MouseEvent::new(MouseEventKind::Drag(MouseButton::Left), 15, 25);
        assert!(event.is_drag());
        assert!(!event.is_down());
        assert!(!event.is_up());
    }

    #[test]
    fn test_mouse_moved() {
        let event = MouseEvent::new(MouseEventKind::Moved, 30, 40);
        assert!(event.is_moved());
        assert!(!event.is_down());
        assert!(!event.is_drag());
    }

    #[test]
    fn test_mouse_button_variants() {
        let left = MouseEvent::new(MouseEventKind::Down(MouseButton::Left), 0, 0);
        let right = MouseEvent::new(MouseEventKind::Down(MouseButton::Right), 0, 0);
        let middle = MouseEvent::new(MouseEventKind::Down(MouseButton::Middle), 0, 0);

        assert!(left.is_down());
        assert!(right.is_down());
        assert!(middle.is_down());
    }
}
