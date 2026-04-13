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
