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
mod tests {
    use super::*;

    // Direction tests
    #[test]
    fn test_direction_opposite() {
        assert_eq!(Direction::Up.opposite(), Direction::Down);
        assert_eq!(Direction::Down.opposite(), Direction::Up);
        assert_eq!(Direction::Left.opposite(), Direction::Right);
        assert_eq!(Direction::Right.opposite(), Direction::Left);
    }

    #[test]
    fn test_direction_is_horizontal() {
        assert!(Direction::Left.is_horizontal());
        assert!(Direction::Right.is_horizontal());
        assert!(!Direction::Up.is_horizontal());
        assert!(!Direction::Down.is_horizontal());
    }

    #[test]
    fn test_direction_is_vertical() {
        assert!(Direction::Up.is_vertical());
        assert!(Direction::Down.is_vertical());
        assert!(!Direction::Left.is_vertical());
        assert!(!Direction::Right.is_vertical());
    }

    #[test]
    fn test_direction_as_delta() {
        assert_eq!(Direction::Up.as_delta(), (0, -1));
        assert_eq!(Direction::Down.as_delta(), (0, 1));
        assert_eq!(Direction::Left.as_delta(), (-1, 0));
        assert_eq!(Direction::Right.as_delta(), (1, 0));
    }

    // SplitDirection tests
    #[test]
    fn test_split_direction_opposite() {
        assert_eq!(SplitDirection::Horizontal.opposite(), SplitDirection::Vertical);
        assert_eq!(SplitDirection::Vertical.opposite(), SplitDirection::Horizontal);
    }

    #[test]
    fn test_split_direction_is_along() {
        // Horizontal split (stacked vertically) - Up/Down navigate between children
        assert!(SplitDirection::Horizontal.is_along(Direction::Up));
        assert!(SplitDirection::Horizontal.is_along(Direction::Down));
        assert!(!SplitDirection::Horizontal.is_along(Direction::Left));
        assert!(!SplitDirection::Horizontal.is_along(Direction::Right));

        // Vertical split (side by side) - Left/Right navigate between children
        assert!(SplitDirection::Vertical.is_along(Direction::Left));
        assert!(SplitDirection::Vertical.is_along(Direction::Right));
        assert!(!SplitDirection::Vertical.is_along(Direction::Up));
        assert!(!SplitDirection::Vertical.is_along(Direction::Down));
    }
}
