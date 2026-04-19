//! Direction types for navigation and layout.

/// Navigation direction for focus movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Move up.
    Up,
    /// Move down.
    Down,
    /// Move left.
    Left,
    /// Move right.
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

    /// Check if this is a horizontal direction.
    #[must_use]
    pub const fn is_horizontal(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }

    /// Check if this is a vertical direction.
    #[must_use]
    pub const fn is_vertical(self) -> bool {
        matches!(self, Self::Up | Self::Down)
    }

    /// Convert to a delta `(dx, dy)` for a single step.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SplitDirection {
    /// Split horizontally (windows stacked vertically).
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
