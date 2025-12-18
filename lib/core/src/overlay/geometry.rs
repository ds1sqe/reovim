//! Overlay geometry traits and types

/// Common bounds for all overlays
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OverlayBounds {
    /// X position (left edge)
    pub x: u16,
    /// Y position (top edge)
    pub y: u16,
    /// Width in columns
    pub width: u16,
    /// Height in rows
    pub height: u16,
}

impl OverlayBounds {
    /// Create a new `OverlayBounds`
    #[must_use]
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Create bounds centered on screen
    #[must_use]
    pub const fn centered(screen_width: u16, screen_height: u16, width: u16, height: u16) -> Self {
        let x = screen_width.saturating_sub(width) / 2;
        let y = screen_height.saturating_sub(height) / 2;
        Self::new(x, y, width, height)
    }

    /// Check if a point is inside the bounds
    #[must_use]
    pub const fn contains(&self, x: u16, y: u16) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }

    /// Get the right edge (exclusive)
    #[must_use]
    pub const fn right(&self) -> u16 {
        self.x + self.width
    }

    /// Get the bottom edge (exclusive)
    #[must_use]
    pub const fn bottom(&self) -> u16 {
        self.y + self.height
    }
}

/// Trait for overlays that compute their own positioning and sizing
///
/// Each feature OWNS its sizing/positioning logic. This trait provides
/// a common interface while allowing feature-specific heuristics.
pub trait OverlayGeometry {
    /// Compute bounds for this overlay based on screen dimensions
    ///
    /// This is the main method for free-floating overlays that position
    /// themselves relative to the screen.
    fn compute_bounds(&self, screen_width: u16, screen_height: u16) -> OverlayBounds;

    /// Compute bounds for cursor-anchored overlays (e.g., completion popup)
    ///
    /// Default implementation ignores the anchor and falls back to `compute_bounds`.
    /// Override for overlays that should appear near the cursor.
    fn compute_bounds_anchored(
        &self,
        screen_width: u16,
        screen_height: u16,
        _anchor_x: u16,
        _anchor_y: u16,
    ) -> OverlayBounds {
        self.compute_bounds(screen_width, screen_height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlay_bounds_new() {
        let bounds = OverlayBounds::new(10, 5, 40, 20);
        assert_eq!(bounds.x, 10);
        assert_eq!(bounds.y, 5);
        assert_eq!(bounds.width, 40);
        assert_eq!(bounds.height, 20);
    }

    #[test]
    fn test_overlay_bounds_centered() {
        let bounds = OverlayBounds::centered(100, 50, 40, 20);
        assert_eq!(bounds.x, 30); // (100 - 40) / 2
        assert_eq!(bounds.y, 15); // (50 - 20) / 2
    }

    #[test]
    fn test_overlay_bounds_contains() {
        let bounds = OverlayBounds::new(10, 10, 20, 10);
        assert!(bounds.contains(10, 10)); // Top-left corner
        assert!(bounds.contains(15, 15)); // Inside
        assert!(bounds.contains(29, 19)); // Just inside
        assert!(!bounds.contains(30, 15)); // Just outside right
        assert!(!bounds.contains(15, 20)); // Just outside bottom
        assert!(!bounds.contains(9, 15)); // Just outside left
    }

    #[test]
    fn test_overlay_bounds_edges() {
        let bounds = OverlayBounds::new(10, 5, 40, 20);
        assert_eq!(bounds.right(), 50);
        assert_eq!(bounds.bottom(), 25);
    }
}
