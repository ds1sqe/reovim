//! Tiling window layout implementation.
//!
//! This module provides `TilingLayout`, which implements the `LayoutPolicy` trait
//! from the display driver. It uses a split tree to manage window positions.
//!
//! # Architecture
//!
//! Following the mechanism vs policy principle:
//! - **Mechanism** (display driver): `LayoutPolicy` trait, rendering
//! - **Policy** (this module): `TilingLayout` decides WHERE windows go
//!
//! # Example
//!
//! ```ignore
//! use reovim_module_layout::TilingLayout;
//! use reovim_driver_display::{LayoutPolicy, WindowId, SplitDirection};
//!
//! let mut layout = TilingLayout::new();
//! let w1 = layout.add_first_window();
//! let w2 = layout.split_vertical(w1).unwrap();
//!
//! // Arrange returns positioned windows
//! let views = layout.arrange((80, 24), &[w1, w2]);
//! ```

use reovim_driver_display::{LayoutPolicy, Rect, SplitDirection, WindowId, WindowView};

use crate::split::SplitTree;

/// Tiling window layout manager.
///
/// Implements vim-style window splits using a binary tree structure.
/// Each window gets a portion of the screen based on the split tree.
#[derive(Debug, Clone, Default)]
pub struct TilingLayout {
    /// The split tree managing window positions.
    tree: SplitTree,
    /// Gap between windows in cells (configurable).
    gap: u16,
}

impl TilingLayout {
    /// Create a new empty tiling layout.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            tree: SplitTree::new(),
            gap: 0,
        }
    }

    /// Create a new tiling layout with gaps between windows.
    #[must_use]
    pub const fn with_gap(gap: u16) -> Self {
        Self {
            tree: SplitTree::new(),
            gap,
        }
    }

    /// Set the gap between windows.
    pub fn set_gap(&mut self, gap: u16) {
        self.gap = gap;
    }

    /// Get the current gap setting.
    #[must_use]
    pub const fn gap(&self) -> u16 {
        self.gap
    }

    /// Check if there are no windows.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }

    /// Get the number of windows.
    #[must_use]
    pub fn window_count(&self) -> usize {
        self.tree.window_count()
    }

    /// Get all window IDs.
    #[must_use]
    pub fn windows(&self) -> Vec<WindowId> {
        self.tree.windows()
    }

    /// Add the first window to an empty layout.
    ///
    /// Returns the new window ID.
    pub fn add_first_window(&mut self) -> WindowId {
        self.tree.add_first_window()
    }

    /// Check if a window exists in the layout.
    #[must_use]
    pub fn contains(&self, id: WindowId) -> bool {
        self.tree.contains(id)
    }

    /// Split a window horizontally (top/bottom).
    ///
    /// Creates a new window below the target. Returns the new window ID.
    pub fn split_horizontal(&mut self, target: WindowId) -> Option<WindowId> {
        self.tree.split_window(target, SplitDirection::Horizontal)
    }

    /// Split a window vertically (left/right).
    ///
    /// Creates a new window to the right of the target. Returns the new window ID.
    pub fn split_vertical(&mut self, target: WindowId) -> Option<WindowId> {
        self.tree.split_window(target, SplitDirection::Vertical)
    }

    /// Close a window.
    ///
    /// Returns true if the window was found and removed.
    pub fn close_window(&mut self, target: WindowId) -> bool {
        self.tree.remove_window(target)
    }

    /// Close all windows except the specified one.
    pub fn close_others(&mut self, keep: WindowId) {
        let windows = self.tree.windows();
        for window in windows {
            if window != keep {
                self.tree.remove_window(window);
            }
        }
    }

    /// Increase the size of a window's split.
    pub fn resize_increase(&mut self, target: WindowId) {
        self.tree.adjust_ratio(target, 0.05);
    }

    /// Decrease the size of a window's split.
    pub fn resize_decrease(&mut self, target: WindowId) {
        self.tree.adjust_ratio(target, -0.05);
    }

    /// Reset all splits to equal ratios.
    ///
    /// Note: This is a simplified implementation that equalizes the immediate
    /// parent split only. A full implementation would recursively equalize.
    pub fn equalize(&mut self, _target: WindowId) {
        // TODO: Implement full tree equalization
        // For now, this is a no-op placeholder
    }

    /// Get the first window in the layout.
    #[must_use]
    pub fn first_window(&self) -> Option<WindowId> {
        self.tree.first_window()
    }

    /// Apply gaps to a rectangle, shrinking it appropriately.
    fn apply_gaps(&self, bounds: Rect, is_first: bool, total: usize) -> Rect {
        if self.gap == 0 || total <= 1 {
            return bounds;
        }

        // Apply gap by shrinking the bounds
        let gap = self.gap;
        let half_gap = gap / 2;

        // For simplicity, apply half gap on each side
        Rect::new(
            if is_first {
                bounds.x
            } else {
                bounds.x.saturating_add(half_gap)
            },
            bounds.y,
            bounds
                .width
                .saturating_sub(if is_first { half_gap } else { gap }),
            bounds.height,
        )
    }

    /// Add an existing window ID (for external ID management).
    pub fn add_existing_window(&mut self, id: WindowId) {
        self.tree.add_existing_window(id);
    }

    /// Split with an existing window ID (for external ID management).
    pub fn split_with_existing(
        &mut self,
        target: WindowId,
        new_id: WindowId,
        direction: SplitDirection,
    ) -> bool {
        self.tree.split_with_existing(target, new_id, direction)
    }
}

impl LayoutPolicy for TilingLayout {
    fn arrange(&self, screen_size: (u16, u16), windows: &[WindowId]) -> Vec<WindowView> {
        // If no windows or empty tree, return empty
        if windows.is_empty() || self.tree.is_empty() {
            return Vec::new();
        }

        // Calculate bounds from tree
        let bounds_list = self.tree.calculate_bounds(screen_size);
        let total = bounds_list.len();

        // Convert to WindowViews, applying gaps and filtering to requested windows
        bounds_list
            .into_iter()
            .enumerate()
            .filter_map(|(i, (id, bounds))| {
                // Only include windows that were requested
                if windows.contains(&id) {
                    let adjusted = self.apply_gaps(bounds, i == 0, total);
                    Some(WindowView::new(id, adjusted))
                } else {
                    None
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tiling_layout_new() {
        let layout = TilingLayout::new();
        assert!(layout.is_empty());
        assert_eq!(layout.window_count(), 0);
        assert_eq!(layout.gap(), 0);
    }

    #[test]
    fn test_tiling_layout_with_gap() {
        let layout = TilingLayout::with_gap(2);
        assert_eq!(layout.gap(), 2);
    }

    #[test]
    fn test_tiling_layout_add_first() {
        let mut layout = TilingLayout::new();
        let id = layout.add_first_window();

        assert!(!layout.is_empty());
        assert_eq!(layout.window_count(), 1);
        assert!(layout.contains(id));
    }

    #[test]
    fn test_tiling_layout_split_vertical() {
        let mut layout = TilingLayout::new();
        let first = layout.add_first_window();
        let second = layout.split_vertical(first);

        assert!(second.is_some());
        assert_eq!(layout.window_count(), 2);
    }

    #[test]
    fn test_tiling_layout_split_horizontal() {
        let mut layout = TilingLayout::new();
        let first = layout.add_first_window();
        let second = layout.split_horizontal(first);

        assert!(second.is_some());
        assert_eq!(layout.window_count(), 2);
    }

    #[test]
    fn test_tiling_layout_close_window() {
        let mut layout = TilingLayout::new();
        let first = layout.add_first_window();
        let second = layout.split_vertical(first).unwrap();

        assert!(layout.close_window(first));
        assert_eq!(layout.window_count(), 1);
        assert!(!layout.contains(first));
        assert!(layout.contains(second));
    }

    #[test]
    fn test_tiling_layout_close_others() {
        let mut layout = TilingLayout::new();
        let first = layout.add_first_window();
        let second = layout.split_vertical(first).unwrap();
        let third = layout.split_horizontal(second).unwrap();

        layout.close_others(second);

        assert_eq!(layout.window_count(), 1);
        assert!(layout.contains(second));
        assert!(!layout.contains(first));
        assert!(!layout.contains(third));
    }

    #[test]
    fn test_tiling_layout_arrange_single() {
        let mut layout = TilingLayout::new();
        let id = layout.add_first_window();

        let views = layout.arrange((80, 24), &[id]);

        assert_eq!(views.len(), 1);
        assert_eq!(views[0].window_id, id);
        assert_eq!(views[0].bounds, Rect::new(0, 0, 80, 24));
    }

    #[test]
    fn test_tiling_layout_arrange_vertical_split() {
        let mut layout = TilingLayout::new();
        let first = layout.add_first_window();
        let second = layout.split_vertical(first).unwrap();

        let views = layout.arrange((80, 24), &[first, second]);

        assert_eq!(views.len(), 2);

        // Find the views by ID
        let first_view = views.iter().find(|v| v.window_id == first).unwrap();
        let second_view = views.iter().find(|v| v.window_id == second).unwrap();

        // First should be on left, second on right
        assert_eq!(first_view.bounds.x, 0);
        assert_eq!(first_view.bounds.width, 40);
        assert_eq!(second_view.bounds.x, 40);
        assert_eq!(second_view.bounds.width, 40);
    }

    #[test]
    fn test_tiling_layout_arrange_horizontal_split() {
        let mut layout = TilingLayout::new();
        let first = layout.add_first_window();
        let second = layout.split_horizontal(first).unwrap();

        let views = layout.arrange((80, 24), &[first, second]);

        assert_eq!(views.len(), 2);

        let first_view = views.iter().find(|v| v.window_id == first).unwrap();
        let second_view = views.iter().find(|v| v.window_id == second).unwrap();

        // First should be on top, second on bottom
        assert_eq!(first_view.bounds.y, 0);
        assert_eq!(first_view.bounds.height, 12);
        assert_eq!(second_view.bounds.y, 12);
        assert_eq!(second_view.bounds.height, 12);
    }

    #[test]
    fn test_tiling_layout_arrange_empty() {
        let layout = TilingLayout::new();
        let views = layout.arrange((80, 24), &[]);
        assert!(views.is_empty());
    }

    #[test]
    fn test_tiling_layout_arrange_filters_windows() {
        let mut layout = TilingLayout::new();
        let first = layout.add_first_window();
        let _second = layout.split_vertical(first).unwrap();

        // Only request first window
        let views = layout.arrange((80, 24), &[first]);

        assert_eq!(views.len(), 1);
        assert_eq!(views[0].window_id, first);
    }

    #[test]
    fn test_tiling_layout_windows() {
        let mut layout = TilingLayout::new();
        let first = layout.add_first_window();
        let second = layout.split_vertical(first).unwrap();

        let windows = layout.windows();
        assert_eq!(windows.len(), 2);
        assert!(windows.contains(&first));
        assert!(windows.contains(&second));
    }

    #[test]
    fn test_tiling_layout_first_window() {
        let mut layout = TilingLayout::new();
        assert!(layout.first_window().is_none());

        let first = layout.add_first_window();
        assert_eq!(layout.first_window(), Some(first));
    }

    #[test]
    fn test_tiling_layout_set_gap() {
        let mut layout = TilingLayout::new();
        assert_eq!(layout.gap(), 0);

        layout.set_gap(2);
        assert_eq!(layout.gap(), 2);
    }
}
