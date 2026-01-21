//! Tiled layer trait for vim-style window splits.
//!
//! The tiled layer manages a binary split tree of windows, providing
//! vim-compatible navigation, splitting, and resizing operations.
//!
//! # Layout Model
//!
//! Windows are organized as a binary tree where each internal node
//! represents a split (horizontal or vertical) and each leaf is a window.
//!
//! ```text
//!       Split(V)          Layout:
//!       /     \           ┌────┬────┐
//!    Win A  Split(H)      │ A  │ B  │
//!            /   \        │    ├────┤
//!         Win B  Win C    │    │ C  │
//!                         └────┴────┘
//! ```
//!
//! # Winnr Ordering
//!
//! Windows are numbered top-to-bottom, left-to-right (vim compatibility):
//!
//! ```text
//! ┌────┬────┐
//! │ 1  │ 2  │  winnr assignment
//! ├────┼────┤
//! │ 3  │ 4  │
//! └────┴────┘
//! ```

use super::layer::WindowPlacement;
use crate::{NavigateDirection, Rect, SplitDirection, WindowId};

/// Tiled layer manages vim-style split windows.
///
/// This trait defines the contract for implementing a tiled window manager.
/// Implementations should maintain a binary split tree and provide
/// vim-compatible navigation.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow the tiled layer to be
/// shared across async tasks.
///
/// # Winnr Algorithm
///
/// Windows must be numbered according to vim's winnr ordering:
///
/// ```text
/// fn assign_winnr(placements: &[WindowPlacement]) -> HashMap<WindowId, usize> {
///     let mut sorted = placements.to_vec();
///     sorted.sort_by_key(|p| (p.bounds.y, p.bounds.x));  // top-to-bottom, left-to-right
///     sorted.iter()
///         .enumerate()
///         .map(|(i, p)| (p.window_id, i + 1))  // 1-indexed
///         .collect()
/// }
/// ```
pub trait TiledLayer: Send + Sync {
    /// Arrange tiled windows within bounds.
    ///
    /// Returns window placements with computed geometry based on the
    /// current split tree structure.
    fn arrange(&self, bounds: Rect) -> Vec<WindowPlacement>;

    /// Add first window to empty layout.
    ///
    /// Called when creating the initial window in an empty tiled zone.
    /// Returns the ID of the new window.
    fn add_first(&mut self) -> WindowId;

    /// Split a window.
    ///
    /// Creates a new window adjacent to `target` by splitting in the
    /// specified direction:
    /// - `Vertical`: New window appears to the right
    /// - `Horizontal`: New window appears below
    ///
    /// # Arguments
    ///
    /// * `target` - The window to split
    /// * `direction` - Direction of the split
    ///
    /// # Returns
    ///
    /// The ID of the new window, or `None` if the split is not possible
    /// (e.g., window too small).
    fn split(&mut self, target: WindowId, direction: SplitDirection) -> Option<WindowId>;

    /// Close a window, returns neighbor to focus.
    ///
    /// Removes the window from the split tree and returns the ID of
    /// a neighboring window that should receive focus.
    ///
    /// # Returns
    ///
    /// - `Some(WindowId)` - The window to focus next
    /// - `None` - This was the last window (caller should handle E444)
    fn close(&mut self, window: WindowId) -> Option<WindowId>;

    /// Navigate from window in direction.
    ///
    /// Finds the nearest window in the specified direction based on
    /// computed geometry (not tree structure).
    ///
    /// # Arguments
    ///
    /// * `from` - Starting window
    /// * `direction` - Direction to navigate
    /// * `views` - Current window placements (needed for geometry-based navigation)
    ///
    /// # Returns
    ///
    /// The window in that direction, or `None` if at boundary.
    fn navigate(
        &self,
        from: WindowId,
        direction: NavigateDirection,
        views: &[WindowPlacement],
    ) -> Option<WindowId>;

    /// Resize window edge.
    ///
    /// Moves the edge of the window in the specified direction.
    /// Positive delta expands the window, negative contracts.
    ///
    /// # Resize Semantics
    ///
    /// ```text
    /// resize(A, Right, +10):
    ///   Before: A(40) | B(40)
    ///   After:  A(50) | B(30)
    ///
    /// The resize propagates through the split tree:
    /// 1. Find the split node containing window in direction
    /// 2. Adjust split ratio at that node
    /// 3. Children recalculate geometry
    /// ```
    ///
    /// # Arguments
    ///
    /// * `window` - Window to resize
    /// * `direction` - Edge to move
    /// * `delta` - Amount to move (positive = expand, negative = contract)
    fn resize(&mut self, window: WindowId, direction: NavigateDirection, delta: i16);

    /// Equalize all windows.
    ///
    /// Distributes space equally among siblings at each level of the
    /// split tree.
    fn equalize(&mut self);

    /// Get all tiled window IDs.
    fn windows(&self) -> Vec<WindowId>;

    /// Check if empty.
    fn is_empty(&self) -> bool;

    /// Cycle to next/previous window.
    ///
    /// Returns the next (forward=true) or previous (forward=false) window
    /// in winnr order, wrapping at boundaries.
    ///
    /// # Arguments
    ///
    /// * `from` - Current window
    /// * `forward` - Direction to cycle (true = next, false = previous)
    /// * `views` - Current window placements (for winnr ordering)
    ///
    /// # Returns
    ///
    /// The target window, or `None` if only one window exists.
    fn cycle(&self, from: WindowId, forward: bool, views: &[WindowPlacement]) -> Option<WindowId>;
}

/// Minimum window dimensions in cells.
///
/// Windows cannot be resized below these dimensions.
pub const MIN_WINDOW_WIDTH: u16 = 10;
pub const MIN_WINDOW_HEIGHT: u16 = 3;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_min_dimensions() {
        assert!(MIN_WINDOW_WIDTH >= 1);
        assert!(MIN_WINDOW_HEIGHT >= 1);
    }
}
