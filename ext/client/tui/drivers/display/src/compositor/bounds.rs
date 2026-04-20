//! Bounds struct for composable element boundaries.

/// Bounds of a composable element.
///
/// Represents a rectangular region with position and dimensions.
/// Uses half-open interval semantics for containment checks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Bounds {
    /// X position (left edge)
    pub x: u16,
    /// Y position (top edge)
    pub y: u16,
    /// Width in columns
    pub width: u16,
    /// Height in rows
    pub height: u16,
}

impl Bounds {
    /// Create new bounds with the given position and dimensions.
    #[must_use]
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Create bounds covering the full screen (starting at 0,0).
    #[must_use]
    pub const fn full_screen(width: u16, height: u16) -> Self {
        Self {
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    /// Check if a point is inside the bounds.
    ///
    /// Uses half-open interval semantics:
    /// - x is in `[self.x, self.x + self.width)` (inclusive start, exclusive end)
    /// - y is in `[self.y, self.y + self.height)` (inclusive start, exclusive end)
    #[must_use]
    pub const fn contains(&self, px: u16, py: u16) -> bool {
        px >= self.x
            && px < self.x.saturating_add(self.width)
            && py >= self.y
            && py < self.y.saturating_add(self.height)
    }

    /// Check if this bounds overlaps with another bounds.
    ///
    /// Two bounds overlap if they share at least one point.
    /// Adjacent bounds (sharing only an edge) do NOT overlap.
    #[must_use]
    pub const fn overlaps(&self, other: &Self) -> bool {
        let self_right = self.x.saturating_add(self.width);
        let self_bottom = self.y.saturating_add(self.height);
        let other_right = other.x.saturating_add(other.width);
        let other_bottom = other.y.saturating_add(other.height);

        self.x < other_right
            && self_right > other.x
            && self.y < other_bottom
            && self_bottom > other.y
    }

    /// Check if bounds are empty (zero width or height).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Get the right edge (exclusive).
    #[must_use]
    pub const fn right(&self) -> u16 {
        self.x.saturating_add(self.width)
    }

    /// Get the bottom edge (exclusive).
    #[must_use]
    pub const fn bottom(&self) -> u16 {
        self.y.saturating_add(self.height)
    }
}

#[cfg(test)]
#[path = "bounds_tests.rs"]
mod tests;
