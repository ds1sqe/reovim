//! Geometry primitives for client-side rendering.
//!
//! These types represent screen coordinates and dimensions,
//! independent of any specific platform (terminal, DOM, native UI).

use serde::{Deserialize, Serialize};

/// Screen position (column, row) in screen coordinates.
///
/// Origin (0, 0) is typically the top-left corner of the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ScreenPosition {
    /// Column (x-coordinate), 0-indexed from left.
    pub x: u16,
    /// Row (y-coordinate), 0-indexed from top.
    pub y: u16,
}

impl ScreenPosition {
    /// Create a new screen position.
    #[must_use]
    pub const fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }

    /// Add an offset to this position.
    ///
    /// Negative results are clamped to 0. Values larger than `i16::MAX`
    /// will wrap around (intentional for screen coordinates).
    #[must_use]
    #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
    pub const fn offset(self, dx: i16, dy: i16) -> Self {
        let new_x = self.x as i16 + dx;
        let new_y = self.y as i16 + dy;
        Self {
            // Safe: we check for negative before cast
            x: if new_x < 0 { 0 } else { new_x as u16 },
            y: if new_y < 0 { 0 } else { new_y as u16 },
        }
    }
}

/// Size in screen units (width and height).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Size {
    /// Width in columns.
    pub width: u16,
    /// Height in rows.
    pub height: u16,
}

impl Size {
    /// Create a new size.
    #[must_use]
    pub const fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }

    /// Calculate the area (width * height).
    #[must_use]
    pub const fn area(self) -> u32 {
        self.width as u32 * self.height as u32
    }

    /// Check if this size is empty (zero area).
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// Rectangle with position and size.
///
/// Represents a rectangular region on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Rect {
    /// X-coordinate of top-left corner.
    pub x: u16,
    /// Y-coordinate of top-left corner.
    pub y: u16,
    /// Width of the rectangle.
    pub width: u16,
    /// Height of the rectangle.
    pub height: u16,
}

impl Rect {
    /// Create a new rectangle.
    #[must_use]
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Create a rectangle from position and size.
    #[must_use]
    pub const fn from_position_size(pos: ScreenPosition, size: Size) -> Self {
        Self {
            x: pos.x,
            y: pos.y,
            width: size.width,
            height: size.height,
        }
    }

    /// Get the top-left position.
    #[must_use]
    pub const fn position(&self) -> ScreenPosition {
        ScreenPosition {
            x: self.x,
            y: self.y,
        }
    }

    /// Get the size.
    #[must_use]
    pub const fn size(&self) -> Size {
        Size {
            width: self.width,
            height: self.height,
        }
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

    /// Check if this rectangle contains a point.
    #[must_use]
    pub const fn contains(&self, pos: ScreenPosition) -> bool {
        pos.x >= self.x && pos.x < self.right() && pos.y >= self.y && pos.y < self.bottom()
    }

    /// Check if this rectangle contains a point at the given coordinates.
    ///
    /// This is a convenience method equivalent to `contains(ScreenPosition::new(x, y))`.
    #[must_use]
    pub const fn contains_xy(&self, x: u16, y: u16) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    /// Check if this rectangle intersects with another.
    #[must_use]
    pub const fn intersects(&self, other: &Self) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }

    /// Calculate the intersection of two rectangles.
    ///
    /// Returns `None` if the rectangles don't intersect.
    #[must_use]
    pub fn intersection(&self, other: &Self) -> Option<Self> {
        if !self.intersects(other) {
            return None;
        }

        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());

        Some(Self {
            x,
            y,
            width: right - x,
            height: bottom - y,
        })
    }

    /// Check if this rectangle is empty (zero area).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Calculate the area of this rectangle.
    #[must_use]
    pub const fn area(&self) -> u32 {
        self.width as u32 * self.height as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ScreenPosition tests
    #[test]
    fn test_screen_position_new() {
        let pos = ScreenPosition::new(10, 20);
        assert_eq!(pos.x, 10);
        assert_eq!(pos.y, 20);
    }

    #[test]
    fn test_screen_position_default() {
        let pos = ScreenPosition::default();
        assert_eq!(pos, ScreenPosition::new(0, 0));
    }

    #[test]
    fn test_screen_position_offset_positive() {
        let pos = ScreenPosition::new(10, 20);
        let offset = pos.offset(5, 10);
        assert_eq!(offset, ScreenPosition::new(15, 30));
    }

    #[test]
    fn test_screen_position_offset_negative_clamped() {
        let pos = ScreenPosition::new(5, 5);
        let offset = pos.offset(-10, -10);
        assert_eq!(offset, ScreenPosition::new(0, 0));
    }

    // Size tests
    #[test]
    fn test_size_new() {
        let size = Size::new(80, 24);
        assert_eq!(size.width, 80);
        assert_eq!(size.height, 24);
    }

    #[test]
    fn test_size_area() {
        let size = Size::new(80, 24);
        assert_eq!(size.area(), 1920);
    }

    #[test]
    fn test_size_is_empty() {
        assert!(Size::new(0, 10).is_empty());
        assert!(Size::new(10, 0).is_empty());
        assert!(!Size::new(10, 10).is_empty());
    }

    #[test]
    fn test_size_default() {
        let size = Size::default();
        assert!(size.is_empty());
    }

    // Rect tests
    #[test]
    fn test_rect_new() {
        let rect = Rect::new(10, 20, 30, 40);
        assert_eq!(rect.x, 10);
        assert_eq!(rect.y, 20);
        assert_eq!(rect.width, 30);
        assert_eq!(rect.height, 40);
    }

    #[test]
    fn test_rect_from_position_size() {
        let pos = ScreenPosition::new(10, 20);
        let size = Size::new(30, 40);
        let rect = Rect::from_position_size(pos, size);
        assert_eq!(rect, Rect::new(10, 20, 30, 40));
    }

    #[test]
    fn test_rect_position_and_size() {
        let rect = Rect::new(10, 20, 30, 40);
        assert_eq!(rect.position(), ScreenPosition::new(10, 20));
        assert_eq!(rect.size(), Size::new(30, 40));
    }

    #[test]
    fn test_rect_right_and_bottom() {
        let rect = Rect::new(10, 20, 30, 40);
        assert_eq!(rect.right(), 40);
        assert_eq!(rect.bottom(), 60);
    }

    #[test]
    fn test_rect_contains_inside() {
        let rect = Rect::new(10, 10, 20, 20);
        assert!(rect.contains(ScreenPosition::new(15, 15)));
        assert!(rect.contains(ScreenPosition::new(10, 10))); // Top-left corner
    }

    #[test]
    fn test_rect_contains_outside() {
        let rect = Rect::new(10, 10, 20, 20);
        assert!(!rect.contains(ScreenPosition::new(5, 15))); // Left of rect
        assert!(!rect.contains(ScreenPosition::new(35, 15))); // Right of rect
        assert!(!rect.contains(ScreenPosition::new(15, 5))); // Above rect
        assert!(!rect.contains(ScreenPosition::new(15, 35))); // Below rect
    }

    #[test]
    fn test_rect_contains_on_boundary() {
        let rect = Rect::new(10, 10, 20, 20);
        // Right and bottom edges are exclusive
        assert!(!rect.contains(ScreenPosition::new(30, 15))); // Right edge
        assert!(!rect.contains(ScreenPosition::new(15, 30))); // Bottom edge
    }

    #[test]
    fn test_rect_contains_xy() {
        let rect = Rect::new(10, 10, 20, 20);
        // Inside
        assert!(rect.contains_xy(15, 15));
        assert!(rect.contains_xy(10, 10)); // Top-left corner
        // Outside
        assert!(!rect.contains_xy(5, 15)); // Left
        assert!(!rect.contains_xy(30, 15)); // Right edge (exclusive)
        assert!(!rect.contains_xy(15, 30)); // Bottom edge (exclusive)
    }

    #[test]
    fn test_rect_intersects_overlapping() {
        let rect1 = Rect::new(0, 0, 20, 20);
        let rect2 = Rect::new(10, 10, 20, 20);
        assert!(rect1.intersects(&rect2));
        assert!(rect2.intersects(&rect1));
    }

    #[test]
    fn test_rect_intersects_non_overlapping() {
        let rect1 = Rect::new(0, 0, 10, 10);
        let rect2 = Rect::new(20, 20, 10, 10);
        assert!(!rect1.intersects(&rect2));
        assert!(!rect2.intersects(&rect1));
    }

    #[test]
    fn test_rect_intersects_touching_edges() {
        // Touching edges don't count as intersection
        let rect1 = Rect::new(0, 0, 10, 10);
        let rect2 = Rect::new(10, 0, 10, 10);
        assert!(!rect1.intersects(&rect2));
    }

    #[test]
    fn test_rect_intersection_overlapping() {
        let rect1 = Rect::new(0, 0, 20, 20);
        let rect2 = Rect::new(10, 10, 20, 20);
        let intersection = rect1.intersection(&rect2);
        assert_eq!(intersection, Some(Rect::new(10, 10, 10, 10)));
    }

    #[test]
    fn test_rect_intersection_non_overlapping() {
        let rect1 = Rect::new(0, 0, 10, 10);
        let rect2 = Rect::new(20, 20, 10, 10);
        assert_eq!(rect1.intersection(&rect2), None);
    }

    #[test]
    fn test_rect_is_empty() {
        assert!(Rect::new(10, 10, 0, 10).is_empty());
        assert!(Rect::new(10, 10, 10, 0).is_empty());
        assert!(!Rect::new(10, 10, 10, 10).is_empty());
    }

    #[test]
    fn test_rect_area() {
        let rect = Rect::new(0, 0, 30, 40);
        assert_eq!(rect.area(), 1200);
    }
}
