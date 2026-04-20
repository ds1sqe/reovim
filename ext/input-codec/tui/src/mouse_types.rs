//! TUI mouse input vocabulary.
//!
//! Provides `MouseButton`, `MouseEvent`, and `MouseEventKind` for terminal
//! cell-grid coordinate mouse events.

use crate::Modifiers;

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

/// Mouse event with cell-grid position and modifiers.
///
/// `column` and `row` are 0-based cell indices in the terminal grid.
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
    /// Create a new mouse event with no modifiers.
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

/// Encode a `MouseEventKind` to a `u8` tag + `u8` button value.
///
/// Encoding:
/// ```text
/// tag  button  meaning
///   0    0     Moved
///   1    b     Down(b)
///   2    b     Up(b)
///   3    b     Drag(b)
///   4    0     ScrollUp
///   5    0     ScrollDown
///   6    0     ScrollLeft
///   7    0     ScrollRight
/// ```
/// Button encoding: Left=0, Right=1, Middle=2.
#[must_use]
pub fn mouse_kind_to_bytes(kind: &MouseEventKind) -> (u8, u8) {
    match kind {
        MouseEventKind::Moved => (0, 0),
        MouseEventKind::Down(b) => (1, button_to_u8(b)),
        MouseEventKind::Up(b) => (2, button_to_u8(b)),
        MouseEventKind::Drag(b) => (3, button_to_u8(b)),
        MouseEventKind::ScrollUp => (4, 0),
        MouseEventKind::ScrollDown => (5, 0),
        MouseEventKind::ScrollLeft => (6, 0),
        MouseEventKind::ScrollRight => (7, 0),
    }
}

/// Decode `(tag, button)` back to `MouseEventKind`.
///
/// Returns `None` for unknown tag values.
#[must_use]
pub fn bytes_to_mouse_kind(tag: u8, button: u8) -> Option<MouseEventKind> {
    match tag {
        0 => Some(MouseEventKind::Moved),
        1 => Some(MouseEventKind::Down(u8_to_button(button)?)),
        2 => Some(MouseEventKind::Up(u8_to_button(button)?)),
        3 => Some(MouseEventKind::Drag(u8_to_button(button)?)),
        4 => Some(MouseEventKind::ScrollUp),
        5 => Some(MouseEventKind::ScrollDown),
        6 => Some(MouseEventKind::ScrollLeft),
        7 => Some(MouseEventKind::ScrollRight),
        _ => None,
    }
}

fn button_to_u8(button: &MouseButton) -> u8 {
    match button {
        MouseButton::Left => 0,
        MouseButton::Right => 1,
        MouseButton::Middle => 2,
    }
}

fn u8_to_button(value: u8) -> Option<MouseButton> {
    match value {
        0 => Some(MouseButton::Left),
        1 => Some(MouseButton::Right),
        2 => Some(MouseButton::Middle),
        _ => None,
    }
}
