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
mod tests {
    use super::*;

    #[test]
    fn test_bounds_new() {
        let bounds = Bounds::new(10, 20, 30, 40);
        assert_eq!(bounds.x, 10);
        assert_eq!(bounds.y, 20);
        assert_eq!(bounds.width, 30);
        assert_eq!(bounds.height, 40);
    }

    #[test]
    fn test_bounds_full_screen() {
        let bounds = Bounds::full_screen(80, 24);
        assert_eq!(bounds.x, 0);
        assert_eq!(bounds.y, 0);
        assert_eq!(bounds.width, 80);
        assert_eq!(bounds.height, 24);
    }

    #[test]
    fn test_bounds_contains_corners() {
        let bounds = Bounds::new(10, 20, 30, 40);
        // Top-left (inclusive)
        assert!(bounds.contains(10, 20));
        // Just inside bottom-right
        assert!(bounds.contains(39, 59));
        // Bottom-right (exclusive)
        assert!(!bounds.contains(40, 60));
    }

    #[test]
    fn test_bounds_contains_edges() {
        let bounds = Bounds::new(10, 20, 30, 40);
        // Left edge (just outside)
        assert!(!bounds.contains(9, 30));
        // Right edge (just outside)
        assert!(!bounds.contains(40, 30));
        // Top edge (just outside)
        assert!(!bounds.contains(20, 19));
        // Bottom edge (just outside)
        assert!(!bounds.contains(20, 60));
    }

    #[test]
    fn test_bounds_overlaps() {
        let a = Bounds::new(0, 0, 50, 50);
        let b = Bounds::new(25, 25, 50, 50);
        let c = Bounds::new(100, 100, 10, 10);
        assert!(a.overlaps(&b));
        assert!(b.overlaps(&a)); // Symmetric
        assert!(!a.overlaps(&c));
        assert!(!c.overlaps(&a));
    }

    #[test]
    fn test_bounds_overlaps_adjacent() {
        // Adjacent bounds should NOT overlap
        let a = Bounds::new(0, 0, 10, 10);
        let b = Bounds::new(10, 0, 10, 10); // Right adjacent
        assert!(!a.overlaps(&b));
    }

    #[test]
    fn test_bounds_overlaps_vertical_non_overlap() {
        // Horizontally overlapping but vertically non-overlapping (lines 68, 69 false)
        let a = Bounds::new(0, 0, 10, 5); // y: 0..5
        let b = Bounds::new(0, 10, 10, 5); // y: 10..15
        // x ranges overlap (both 0..10), but y ranges don't
        assert!(!a.overlaps(&b));
        assert!(!b.overlaps(&a));

        // Vertically adjacent (a bottom == b top)
        let c = Bounds::new(0, 0, 10, 10); // y: 0..10
        let d = Bounds::new(0, 10, 10, 10); // y: 10..20
        assert!(!c.overlaps(&d));
    }

    #[test]
    fn test_bounds_is_empty() {
        assert!(Bounds::new(0, 0, 0, 10).is_empty());
        assert!(Bounds::new(0, 0, 10, 0).is_empty());
        assert!(!Bounds::new(0, 0, 10, 10).is_empty());
    }

    #[test]
    fn test_bounds_right_bottom() {
        let bounds = Bounds::new(10, 20, 30, 40);
        assert_eq!(bounds.right(), 40);
        assert_eq!(bounds.bottom(), 60);
    }
}
