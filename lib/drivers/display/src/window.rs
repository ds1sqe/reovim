//! Window management types.

/// Unique window identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct WindowId(pub usize);

impl WindowId {
    /// Create a new window ID.
    #[must_use]
    pub const fn new(id: usize) -> Self {
        Self(id)
    }

    /// Get the raw ID value.
    #[must_use]
    pub const fn raw(&self) -> usize {
        self.0
    }
}

/// Split direction for window layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    /// Top/bottom split.
    Horizontal,
    /// Left/right split.
    Vertical,
}

/// Navigation direction between windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigateDirection {
    /// Navigate left.
    Left,
    /// Navigate down.
    Down,
    /// Navigate up.
    Up,
    /// Navigate right.
    Right,
}

/// Terminal size in columns and rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TerminalSize {
    /// Width in columns.
    pub width: u16,
    /// Height in rows.
    pub height: u16,
}

impl TerminalSize {
    /// Create a new terminal size.
    #[must_use]
    pub const fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }

    /// Check if the size is valid (non-zero dimensions).
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.width > 0 && self.height > 0
    }
}

/// Rectangle for window bounds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rect {
    /// X coordinate (column).
    pub x: u16,
    /// Y coordinate (row).
    pub y: u16,
    /// Width in columns.
    pub width: u16,
    /// Height in rows.
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

    /// Check if a point is within this rectangle.
    #[must_use]
    pub const fn contains(&self, px: u16, py: u16) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_id() {
        let id = WindowId::new(42);
        assert_eq!(id.raw(), 42);

        let default_id = WindowId::default();
        assert_eq!(default_id.raw(), 0);
    }

    #[test]
    fn test_terminal_size_is_valid() {
        assert!(TerminalSize::new(80, 24).is_valid());
        assert!(TerminalSize::new(1, 1).is_valid());

        assert!(!TerminalSize::new(0, 24).is_valid());
        assert!(!TerminalSize::new(80, 0).is_valid());
        assert!(!TerminalSize::new(0, 0).is_valid());
    }

    #[test]
    fn test_rect_contains() {
        let rect = Rect::new(10, 20, 30, 40);

        // Inside
        assert!(rect.contains(10, 20)); // Top-left corner
        assert!(rect.contains(25, 40)); // Middle
        assert!(rect.contains(39, 59)); // Bottom-right corner (exclusive bounds)

        // Outside
        assert!(!rect.contains(9, 20)); // Left of rect
        assert!(!rect.contains(10, 19)); // Above rect
        assert!(!rect.contains(40, 20)); // Right of rect (at boundary)
        assert!(!rect.contains(10, 60)); // Below rect (at boundary)
    }
}
