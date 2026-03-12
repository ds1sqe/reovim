//! Direction types for navigation and layout.
//!
//! These types represent directions for cursor movement,
//! focus navigation, and window splitting.

use serde::{Deserialize, Serialize};

/// Navigation direction for focus movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum Direction {
    /// Move up (decrease y).
    Up,
    /// Move down (increase y).
    Down,
    /// Move left (decrease x).
    Left,
    /// Move right (increase x).
    Right,
}

impl Direction {
    /// Get the opposite direction.
    #[must_use]
    pub const fn opposite(self) -> Self {
        match self {
            Self::Up => Self::Down,
            Self::Down => Self::Up,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }

    /// Check if this is a horizontal direction (Left or Right).
    #[must_use]
    pub const fn is_horizontal(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }

    /// Check if this is a vertical direction (Up or Down).
    #[must_use]
    pub const fn is_vertical(self) -> bool {
        matches!(self, Self::Up | Self::Down)
    }

    /// Convert to a delta (dx, dy) for a single step.
    #[must_use]
    pub const fn as_delta(self) -> (i16, i16) {
        match self {
            Self::Up => (0, -1),
            Self::Down => (0, 1),
            Self::Left => (-1, 0),
            Self::Right => (1, 0),
        }
    }
}

/// Split direction for window layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum SplitDirection {
    /// Split horizontally (windows stacked vertically, one above the other).
    Horizontal,
    /// Split vertically (windows side by side).
    Vertical,
}

impl SplitDirection {
    /// Get the opposite split direction.
    #[must_use]
    pub const fn opposite(self) -> Self {
        match self {
            Self::Horizontal => Self::Vertical,
            Self::Vertical => Self::Horizontal,
        }
    }

    /// Check if navigation direction is along this split.
    ///
    /// For horizontal splits (stacked vertically), Up/Down navigate between children.
    /// For vertical splits (side by side), Left/Right navigate between children.
    #[must_use]
    pub const fn is_along(&self, direction: Direction) -> bool {
        match self {
            Self::Horizontal => direction.is_vertical(),
            Self::Vertical => direction.is_horizontal(),
        }
    }
}

#[cfg(test)]
#[path = "direction_tests.rs"]
mod tests;
