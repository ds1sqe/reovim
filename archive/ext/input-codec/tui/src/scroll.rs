//! TUI scroll event type.
//!
//! `ScrollEvent` captures axis-based scroll deltas with optional cell-grid
//! position and modifiers.  It is separate from `MouseEventKind::ScrollUp/Down`
//! to allow high-resolution scrolling (e.g., trackpads) and horizontal scroll.

use crate::Modifiers;

/// Scroll event with directional deltas and optional position.
///
/// `dx` / `dy` are signed; positive dx = right, positive dy = down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollEvent {
    /// Horizontal scroll delta (positive = right).
    pub dx: i16,
    /// Vertical scroll delta (positive = down).
    pub dy: i16,
    /// Column of the pointer when scrolling (0-based, may be 0 when unknown).
    pub x: u16,
    /// Row of the pointer when scrolling (0-based, may be 0 when unknown).
    pub y: u16,
    /// Active modifiers at scroll time.
    pub modifiers: Modifiers,
}

impl ScrollEvent {
    /// Create a vertical scroll event with no position.
    #[must_use]
    pub const fn vertical(dy: i16) -> Self {
        Self {
            dx: 0,
            dy,
            x: 0,
            y: 0,
            modifiers: Modifiers::NONE,
        }
    }

    /// Create a scroll event with full fields.
    #[must_use]
    pub const fn new(dx: i16, dy: i16, x: u16, y: u16, modifiers: Modifiers) -> Self {
        Self {
            dx,
            dy,
            x,
            y,
            modifiers,
        }
    }
}
