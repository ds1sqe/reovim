//! View manager trait - owns per-window state.
//!
//! The `ViewManager` manages the relationship between windows and their
//! content (buffers). Each window has a View that tracks cursor position,
//! scroll offset, and which buffer is displayed.
//!
//! # Separation of Concerns
//!
//! - **Compositor**: Knows window geometry (WHERE windows are)
//! - **`ViewManager`**: Knows window content (WHAT windows show)
//!
//! This separation allows the compositor to handle layout without knowing
//! about cursors, buffers, or scroll positions.
//!
//! # Location
//!
//! - **Trait definition**: `lib/drivers/display/src/layout/view.rs` (mechanism)
//! - **Implementation**: `modules/editor/src/view_manager.rs` (policy)
//!
//! # Strong Typing
//!
//! This module uses [`LineIndex`] and [`ColIndex`] newtypes to prevent
//! accidentally mixing up line numbers with column numbers or other indices.

use crate::WindowId;
use reovim_kernel::api::v1::BufferId;

// ============================================================================
// Index Newtypes (Strong Typing)
// ============================================================================

/// Line index within a buffer (0-indexed).
///
/// Using a newtype prevents accidentally mixing line indices with column
/// indices or other numeric values.
///
/// # Example
///
/// ```
/// use reovim_display::layout::LineIndex;
///
/// let line = LineIndex::new(5);
/// assert_eq!(line.as_usize(), 5);
///
/// // Arithmetic operations
/// let next = line + 1;
/// assert_eq!(next.as_usize(), 6);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct LineIndex(usize);

impl LineIndex {
    /// Create a new line index.
    #[must_use]
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Get the raw numeric value.
    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.0
    }

    /// Line index zero (first line).
    pub const ZERO: Self = Self(0);
}

impl std::ops::Add<usize> for LineIndex {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl std::ops::Sub<usize> for LineIndex {
    type Output = Self;

    fn sub(self, rhs: usize) -> Self::Output {
        Self(self.0.saturating_sub(rhs))
    }
}

impl std::fmt::Display for LineIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Display as 1-indexed for user-facing output
        write!(f, "{}", self.0 + 1)
    }
}

/// Column index within a line (0-indexed, counting chars).
///
/// Using a newtype prevents accidentally mixing column indices with line
/// indices or other numeric values.
///
/// # Example
///
/// ```
/// use reovim_display::layout::ColIndex;
///
/// let col = ColIndex::new(10);
/// assert_eq!(col.as_usize(), 10);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ColIndex(usize);

impl ColIndex {
    /// Create a new column index.
    #[must_use]
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Get the raw numeric value.
    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.0
    }

    /// Column index zero (first column).
    pub const ZERO: Self = Self(0);
}

impl std::ops::Add<usize> for ColIndex {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl std::ops::Sub<usize> for ColIndex {
    type Output = Self;

    fn sub(self, rhs: usize) -> Self::Output {
        Self(self.0.saturating_sub(rhs))
    }
}

impl std::fmt::Display for ColIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Display as 1-indexed for user-facing output
        write!(f, "{}", self.0 + 1)
    }
}

// ============================================================================
// Position
// ============================================================================

/// Position in a buffer (line and column).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Position {
    /// Line number (0-indexed).
    pub line: LineIndex,
    /// Column number (0-indexed).
    pub col: ColIndex,
}

impl Position {
    /// Create a new position.
    #[must_use]
    pub const fn new(line: LineIndex, col: ColIndex) -> Self {
        Self { line, col }
    }

    /// Create a position from raw usize values.
    ///
    /// Convenience constructor for cases where you have raw indices.
    #[must_use]
    pub const fn from_raw(line: usize, col: usize) -> Self {
        Self {
            line: LineIndex::new(line),
            col: ColIndex::new(col),
        }
    }

    /// Position at the start of a buffer.
    #[must_use]
    pub const fn origin() -> Self {
        Self {
            line: LineIndex::ZERO,
            col: ColIndex::ZERO,
        }
    }
}

/// A View is a window's perspective into a buffer.
///
/// Multiple windows can have views into the same buffer,
/// each with independent cursor and scroll positions.
#[derive(Debug, Clone)]
pub struct View {
    /// Buffer being displayed.
    pub buffer_id: BufferId,
    /// Cursor position within the buffer.
    pub cursor: Position,
    /// First visible line (scroll offset).
    pub scroll_top: LineIndex,
    /// First visible column (horizontal scroll).
    pub scroll_left: ColIndex,
}

impl View {
    /// Create a new view for a buffer.
    #[must_use]
    pub const fn new(buffer_id: BufferId) -> Self {
        Self {
            buffer_id,
            cursor: Position::origin(),
            scroll_top: LineIndex::ZERO,
            scroll_left: ColIndex::ZERO,
        }
    }

    /// Create a view with specific cursor position.
    #[must_use]
    pub const fn with_cursor(mut self, cursor: Position) -> Self {
        self.cursor = cursor;
        self
    }

    /// Create a view with specific scroll position.
    #[must_use]
    pub const fn with_scroll(mut self, top: LineIndex, left: ColIndex) -> Self {
        self.scroll_top = top;
        self.scroll_left = left;
        self
    }

    /// Create a view with scroll position from raw values.
    #[must_use]
    pub const fn with_scroll_raw(mut self, top: usize, left: usize) -> Self {
        self.scroll_top = LineIndex::new(top);
        self.scroll_left = ColIndex::new(left);
        self
    }
}

/// `ViewManager` owns all per-window view state.
///
/// This trait is implemented by the editor module to manage the
/// relationship between windows and buffers.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow the `ViewManager`
/// to be shared across async tasks.
///
/// # Lifecycle
///
/// 1. When a window is created: `create(window_id, buffer_id)`
/// 2. During editing: Views are accessed via `get`/`get_mut`
/// 3. When focus changes: `on_focus_changed(old, new)`
/// 4. When a window closes: `remove(window_id)`
pub trait ViewManager: Send + Sync {
    /// Create a view for a new window.
    ///
    /// Called when a window is created and needs to display a buffer.
    /// Returns a reference to the created view.
    fn create(&mut self, window: WindowId, buffer: BufferId) -> &View;

    /// Get view for a window.
    fn get(&self, window: WindowId) -> Option<&View>;

    /// Get mutable view.
    fn get_mut(&mut self, window: WindowId) -> Option<&mut View>;

    /// Remove view when window closes.
    fn remove(&mut self, window: WindowId);

    /// Called when focus changes between windows.
    ///
    /// Allows the `ViewManager` to update state (e.g., save cursor
    /// position to jumplist, update alternate file).
    fn on_focus_changed(&mut self, from: WindowId, to: WindowId);

    /// Get all views for rendering.
    ///
    /// Returns window IDs paired with their views.
    fn all_views(&self) -> Vec<(WindowId, &View)>;

    /// Get the buffer displayed in a window.
    fn buffer_of(&self, window: WindowId) -> Option<BufferId> {
        self.get(window).map(|v| v.buffer_id)
    }

    /// Check if a window exists.
    fn contains(&self, window: WindowId) -> bool {
        self.get(window).is_some()
    }

    /// Count of managed views.
    fn len(&self) -> usize {
        self.all_views().len()
    }

    /// Check if no views exist.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_index() {
        let line = LineIndex::new(10);
        assert_eq!(line.as_usize(), 10);

        // Arithmetic
        assert_eq!((line + 5).as_usize(), 15);
        assert_eq!((line - 3).as_usize(), 7);

        // Saturating subtraction
        let zero = LineIndex::ZERO;
        assert_eq!((zero - 5).as_usize(), 0);

        // Display (1-indexed)
        assert_eq!(format!("{line}"), "11");
    }

    #[test]
    fn test_col_index() {
        let col = ColIndex::new(5);
        assert_eq!(col.as_usize(), 5);

        // Arithmetic
        assert_eq!((col + 3).as_usize(), 8);
        assert_eq!((col - 2).as_usize(), 3);

        // Display (1-indexed)
        assert_eq!(format!("{col}"), "6");
    }

    #[test]
    fn test_position_new() {
        let pos = Position::new(LineIndex::new(10), ColIndex::new(5));
        assert_eq!(pos.line.as_usize(), 10);
        assert_eq!(pos.col.as_usize(), 5);
    }

    #[test]
    fn test_position_from_raw() {
        let pos = Position::from_raw(10, 5);
        assert_eq!(pos.line.as_usize(), 10);
        assert_eq!(pos.col.as_usize(), 5);
    }

    #[test]
    fn test_position_origin() {
        let pos = Position::origin();
        assert_eq!(pos.line.as_usize(), 0);
        assert_eq!(pos.col.as_usize(), 0);
    }

    #[test]
    fn test_view_new() {
        let buffer_id = BufferId::from_raw(1);
        let view = View::new(buffer_id);
        assert_eq!(view.buffer_id, buffer_id);
        assert_eq!(view.cursor, Position::origin());
        assert_eq!(view.scroll_top.as_usize(), 0);
        assert_eq!(view.scroll_left.as_usize(), 0);
    }

    #[test]
    fn test_view_builder() {
        let buffer_id = BufferId::from_raw(1);
        let view = View::new(buffer_id)
            .with_cursor(Position::from_raw(10, 5))
            .with_scroll(LineIndex::new(100), ColIndex::new(20));

        assert_eq!(view.cursor.line.as_usize(), 10);
        assert_eq!(view.cursor.col.as_usize(), 5);
        assert_eq!(view.scroll_top.as_usize(), 100);
        assert_eq!(view.scroll_left.as_usize(), 20);
    }

    #[test]
    fn test_view_builder_raw() {
        let buffer_id = BufferId::from_raw(1);
        let view = View::new(buffer_id)
            .with_cursor(Position::from_raw(10, 5))
            .with_scroll_raw(100, 20);

        assert_eq!(view.scroll_top.as_usize(), 100);
        assert_eq!(view.scroll_left.as_usize(), 20);
    }
}
