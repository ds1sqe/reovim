//! Tiling window layout implementation.
//!
//! This module provides `TilingLayout`, which implements both `LayoutPolicy`
//! (legacy) and `TiledLayer` (new nested compositor) traits from the display driver.
//!
//! # Architecture
//!
//! Following the mechanism vs policy principle:
//! - **Mechanism** (display driver): `TiledLayer` trait (new), `LayoutPolicy` trait (legacy)
//! - **Policy** (this module): `TilingLayout` decides WHERE windows go
//!
//! # Example
//!
//! ```ignore
//! use reovim_module_layout::TilingLayout;
//! use reovim_driver_display::{TiledLayer, WindowId, SplitDirection, Rect, LayerId, Zone};
//!
//! let mut layout = TilingLayout::new(LayerId::new(0), Zone::Tiled, ZOrder::new(100));
//! let w1 = layout.add_first();
//! let w2 = layout.split(w1, SplitDirection::Vertical).unwrap();
//!
//! // Arrange returns positioned windows with layer context
//! let placements = layout.arrange(Rect::new(0, 0, 80, 24));
//! ```

use reovim_driver_display::{
    FocusPolicy, LayoutPolicy, NavigateDirection, Rect, SplitDirection, WindowId, WindowView,
    layout::{LayerId, TiledLayer, WindowPlacement, ZOrder, Zone},
};

use crate::{focus::VimFocusPolicy, split::SplitTree};

/// Tiling window layout manager.
///
/// Implements vim-style window splits using a binary tree structure.
/// Each window gets a portion of the screen based on the split tree.
///
/// # Layer Context
///
/// `TilingLayout` stores layer context (`layer_id`, `zone`, `z_base`) to create
/// proper `WindowPlacement` values when arranging windows. This context is set
/// when the layout is created and used for all windows in this tiled zone.
#[derive(Debug, Clone)]
pub struct TilingLayout {
    /// The split tree managing window positions.
    tree: SplitTree,
    /// Gap between windows in cells (configurable).
    gap: u16,
    /// Layer ID this layout belongs to.
    layer_id: LayerId,
    /// Zone within the layer (always Tiled for this layout).
    zone: Zone,
    /// Base z-order for windows in this zone.
    z_base: ZOrder,
    /// Cached screen size for winnr calculations.
    screen: Rect,
}

impl Default for TilingLayout {
    fn default() -> Self {
        Self {
            tree: SplitTree::new(),
            gap: 0,
            layer_id: LayerId::new(0),
            zone: Zone::Tiled,
            z_base: ZOrder::new(0),
            screen: Rect::new(0, 0, 80, 24),
        }
    }
}

impl TilingLayout {
    /// Create a new empty tiling layout with layer context.
    ///
    /// # Arguments
    ///
    /// * `layer_id` - The layer this layout belongs to
    /// * `zone` - The zone within the layer (typically `Zone::Tiled`)
    /// * `z_base` - Base z-order for windows in this zone
    #[must_use]
    pub const fn new(layer_id: LayerId, zone: Zone, z_base: ZOrder) -> Self {
        Self {
            tree: SplitTree::new(),
            gap: 0,
            layer_id,
            zone,
            z_base,
            screen: Rect::new(0, 0, 80, 24), // Default screen size
        }
    }

    /// Create a new tiling layout with gaps between windows.
    #[must_use]
    pub const fn with_gap(layer_id: LayerId, zone: Zone, z_base: ZOrder, gap: u16) -> Self {
        Self {
            tree: SplitTree::new(),
            gap,
            layer_id,
            zone,
            z_base,
            screen: Rect::new(0, 0, 80, 24),
        }
    }

    /// Update the cached screen size.
    ///
    /// Called on terminal resize to update winnr calculations.
    pub fn set_screen(&mut self, screen: Rect) {
        self.screen = screen;
    }

    /// Get the cached screen size.
    #[must_use]
    pub const fn screen(&self) -> Rect {
        self.screen
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
    /// Recursively equalizes all split nodes to 0.5 ratio.
    pub fn equalize_all(&mut self) {
        self.tree.equalize();
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

    /// Convert WindowPlacement to WindowView for VimFocusPolicy.
    fn placement_to_view(placement: &WindowPlacement) -> WindowView {
        WindowView::new(placement.window_id, placement.bounds)
    }
}

// =============================================================================
// TiledLayer Implementation (New Nested Compositor Architecture)
// =============================================================================

impl TiledLayer for TilingLayout {
    fn arrange(&self, bounds: Rect) -> Vec<WindowPlacement> {
        if self.tree.is_empty() {
            return Vec::new();
        }

        let bounds_list = self.tree.calculate_bounds((bounds.width, bounds.height));
        let total = bounds_list.len();

        bounds_list
            .into_iter()
            .enumerate()
            .map(|(i, (id, rect))| {
                let adjusted = self.apply_gaps(rect, i == 0, total);
                WindowPlacement::new(
                    id,
                    self.layer_id,
                    self.zone,
                    adjusted,
                    self.z_base.offset(u16::try_from(i).unwrap_or(0)),
                )
            })
            .collect()
    }

    fn add_first(&mut self) -> WindowId {
        self.tree.add_first_window()
    }

    fn split(&mut self, target: WindowId, direction: SplitDirection) -> Option<WindowId> {
        self.tree.split_window(target, direction)
    }

    fn close(&mut self, window: WindowId) -> Option<WindowId> {
        let screen_size = (self.screen.width, self.screen.height);
        let neighbor = self.tree.neighbor_for_focus(window, screen_size);
        self.tree.remove_window(window);
        neighbor
    }

    fn navigate(
        &self,
        from: WindowId,
        direction: NavigateDirection,
        views: &[WindowPlacement],
    ) -> Option<WindowId> {
        // Convert WindowPlacement to WindowView for VimFocusPolicy
        let window_views: Vec<WindowView> = views.iter().map(Self::placement_to_view).collect();
        VimFocusPolicy::new().next(direction, from, &window_views)
    }

    fn resize(&mut self, window: WindowId, direction: NavigateDirection, delta: i16) {
        // Convert direction + delta to ratio adjustment
        // Positive delta = expand window in that direction
        let ratio_delta = f32::from(delta) * 0.02; // 2% per unit

        // For resize: moving edge in direction means growing/shrinking
        // Left/Up with positive delta = shrink (move edge inward)
        // Right/Down with positive delta = grow (move edge outward)
        let adjusted_delta = match direction {
            NavigateDirection::Left | NavigateDirection::Up => -ratio_delta,
            NavigateDirection::Right | NavigateDirection::Down => ratio_delta,
        };

        self.tree.adjust_ratio(window, adjusted_delta);
    }

    fn equalize(&mut self) {
        self.tree.equalize();
    }

    fn windows(&self) -> Vec<WindowId> {
        self.tree.windows()
    }

    fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }

    fn cycle(&self, from: WindowId, forward: bool, _views: &[WindowPlacement]) -> Option<WindowId> {
        // Use winnr order from tree for cycling
        let screen_size = (self.screen.width, self.screen.height);
        self.tree.cycle(from, forward, screen_size)
    }
}

// =============================================================================
// LayoutPolicy Implementation (Legacy - for backward compatibility)
// =============================================================================

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
        let layout = TilingLayout::default();
        assert!(layout.is_empty());
        assert_eq!(layout.window_count(), 0);
        assert_eq!(layout.gap(), 0);
    }

    #[test]
    fn test_tiling_layout_with_gap() {
        let layout = TilingLayout::with_gap(LayerId::new(0), Zone::Tiled, ZOrder::new(0), 2);
        assert_eq!(layout.gap(), 2);
    }

    #[test]
    fn test_tiling_layout_add_first() {
        let mut layout = TilingLayout::default();
        let id = layout.add_first_window();

        assert!(!layout.is_empty());
        assert_eq!(layout.window_count(), 1);
        assert!(layout.contains(id));
    }

    #[test]
    fn test_tiling_layout_split_vertical() {
        let mut layout = TilingLayout::default();
        let first = layout.add_first_window();
        let second = layout.split_vertical(first);

        assert!(second.is_some());
        assert_eq!(layout.window_count(), 2);
    }

    #[test]
    fn test_tiling_layout_split_horizontal() {
        let mut layout = TilingLayout::default();
        let first = layout.add_first_window();
        let second = layout.split_horizontal(first);

        assert!(second.is_some());
        assert_eq!(layout.window_count(), 2);
    }

    #[test]
    fn test_tiling_layout_close_window() {
        let mut layout = TilingLayout::default();
        let first = layout.add_first_window();
        let second = layout.split_vertical(first).unwrap();

        assert!(layout.close_window(first));
        assert_eq!(layout.window_count(), 1);
        assert!(!layout.contains(first));
        assert!(layout.contains(second));
    }

    #[test]
    fn test_tiling_layout_close_others() {
        let mut layout = TilingLayout::default();
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
        let mut layout = TilingLayout::default();
        let id = layout.add_first_window();

        let views = LayoutPolicy::arrange(&layout, (80, 24), &[id]);

        assert_eq!(views.len(), 1);
        assert_eq!(views[0].window_id, id);
        assert_eq!(views[0].bounds, Rect::new(0, 0, 80, 24));
    }

    #[test]
    fn test_tiling_layout_arrange_vertical_split() {
        let mut layout = TilingLayout::default();
        let first = layout.add_first_window();
        let second = layout.split_vertical(first).unwrap();

        let views = LayoutPolicy::arrange(&layout, (80, 24), &[first, second]);

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
        let mut layout = TilingLayout::default();
        let first = layout.add_first_window();
        let second = layout.split_horizontal(first).unwrap();

        let views = LayoutPolicy::arrange(&layout, (80, 24), &[first, second]);

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
        let layout = TilingLayout::default();
        let views = LayoutPolicy::arrange(&layout, (80, 24), &[]);
        assert!(views.is_empty());
    }

    #[test]
    fn test_tiling_layout_arrange_filters_windows() {
        let mut layout = TilingLayout::default();
        let first = layout.add_first_window();
        let _second = layout.split_vertical(first).unwrap();

        // Only request first window
        let views = LayoutPolicy::arrange(&layout, (80, 24), &[first]);

        assert_eq!(views.len(), 1);
        assert_eq!(views[0].window_id, first);
    }

    #[test]
    fn test_tiling_layout_windows() {
        let mut layout = TilingLayout::default();
        let first = layout.add_first_window();
        let second = layout.split_vertical(first).unwrap();

        let windows = layout.windows();
        assert_eq!(windows.len(), 2);
        assert!(windows.contains(&first));
        assert!(windows.contains(&second));
    }

    #[test]
    fn test_tiling_layout_first_window() {
        let mut layout = TilingLayout::default();
        assert!(layout.first_window().is_none());

        let first = layout.add_first_window();
        assert_eq!(layout.first_window(), Some(first));
    }

    #[test]
    fn test_tiling_layout_set_gap() {
        let mut layout = TilingLayout::default();
        assert_eq!(layout.gap(), 0);

        layout.set_gap(2);
        assert_eq!(layout.gap(), 2);
    }

    // =========================================================================
    // TiledLayer Trait Tests
    // =========================================================================

    #[test]
    fn test_tiled_layer_arrange() {
        let mut layout = TilingLayout::default();
        let id = layout.add_first_window();

        let placements = TiledLayer::arrange(&layout, Rect::new(0, 0, 80, 24));

        assert_eq!(placements.len(), 1);
        assert_eq!(placements[0].window_id, id);
        assert_eq!(placements[0].bounds, Rect::new(0, 0, 80, 24));
        assert_eq!(placements[0].layer_id, LayerId::new(0));
        assert_eq!(placements[0].zone, Zone::Tiled);
    }

    #[test]
    fn test_tiled_layer_add_first() {
        let mut layout = TilingLayout::default();

        let id = TiledLayer::add_first(&mut layout);

        assert!(!TiledLayer::is_empty(&layout));
        assert_eq!(TiledLayer::windows(&layout).len(), 1);
        assert!(TiledLayer::windows(&layout).contains(&id));
    }

    #[test]
    fn test_tiled_layer_split() {
        let mut layout = TilingLayout::default();
        let first = TiledLayer::add_first(&mut layout);

        let second = TiledLayer::split(&mut layout, first, SplitDirection::Vertical);

        assert!(second.is_some());
        assert_eq!(TiledLayer::windows(&layout).len(), 2);
    }

    #[test]
    fn test_tiled_layer_close() {
        let mut layout = TilingLayout::new(LayerId::new(0), Zone::Tiled, ZOrder::new(100));
        layout.set_screen(Rect::new(0, 0, 80, 24));
        let first = TiledLayer::add_first(&mut layout);
        let second = TiledLayer::split(&mut layout, first, SplitDirection::Vertical).unwrap();

        // Close first, should return second as neighbor
        let neighbor = TiledLayer::close(&mut layout, first);

        assert_eq!(neighbor, Some(second));
        assert_eq!(TiledLayer::windows(&layout).len(), 1);
    }

    #[test]
    fn test_tiled_layer_close_last_window() {
        let mut layout = TilingLayout::new(LayerId::new(0), Zone::Tiled, ZOrder::new(100));
        layout.set_screen(Rect::new(0, 0, 80, 24));
        let first = TiledLayer::add_first(&mut layout);

        // Close last window, should return None
        let neighbor = TiledLayer::close(&mut layout, first);

        assert_eq!(neighbor, None);
    }

    #[test]
    fn test_tiled_layer_navigate() {
        let mut layout = TilingLayout::default();
        let first = TiledLayer::add_first(&mut layout);
        let second = TiledLayer::split(&mut layout, first, SplitDirection::Vertical).unwrap();

        let views = TiledLayer::arrange(&layout, Rect::new(0, 0, 80, 24));

        // Navigate right from first should find second
        let next = TiledLayer::navigate(&layout, first, NavigateDirection::Right, &views);
        assert_eq!(next, Some(second));

        // Navigate left from second should find first
        let prev = TiledLayer::navigate(&layout, second, NavigateDirection::Left, &views);
        assert_eq!(prev, Some(first));
    }

    #[test]
    fn test_tiled_layer_navigate_boundary() {
        let mut layout = TilingLayout::default();
        let first = TiledLayer::add_first(&mut layout);
        let _second = TiledLayer::split(&mut layout, first, SplitDirection::Vertical).unwrap();

        let views = TiledLayer::arrange(&layout, Rect::new(0, 0, 80, 24));

        // Navigate left from first should hit boundary (return None)
        let result = TiledLayer::navigate(&layout, first, NavigateDirection::Left, &views);
        assert_eq!(result, None);
    }

    #[test]
    fn test_tiled_layer_equalize() {
        let mut layout = TilingLayout::default();
        let first = TiledLayer::add_first(&mut layout);
        let _second = TiledLayer::split(&mut layout, first, SplitDirection::Vertical).unwrap();

        // Resize to unequal
        TiledLayer::resize(&mut layout, first, NavigateDirection::Right, 5);

        // Equalize should reset
        TiledLayer::equalize(&mut layout);

        let views = TiledLayer::arrange(&layout, Rect::new(0, 0, 80, 24));
        assert_eq!(views[0].bounds.width, 40);
        assert_eq!(views[1].bounds.width, 40);
    }

    #[test]
    fn test_tiled_layer_cycle() {
        let mut layout = TilingLayout::new(LayerId::new(0), Zone::Tiled, ZOrder::new(100));
        layout.set_screen(Rect::new(0, 0, 80, 24));
        let first = TiledLayer::add_first(&mut layout);
        let second = TiledLayer::split(&mut layout, first, SplitDirection::Vertical).unwrap();
        let third = TiledLayer::split(&mut layout, second, SplitDirection::Vertical).unwrap();

        let views = TiledLayer::arrange(&layout, Rect::new(0, 0, 80, 24));

        // Cycle forward: first -> second -> third -> first
        assert_eq!(TiledLayer::cycle(&layout, first, true, &views), Some(second));
        assert_eq!(TiledLayer::cycle(&layout, second, true, &views), Some(third));
        assert_eq!(TiledLayer::cycle(&layout, third, true, &views), Some(first));

        // Cycle backward: first -> third -> second -> first
        assert_eq!(TiledLayer::cycle(&layout, first, false, &views), Some(third));
    }

    #[test]
    fn test_tiled_layer_resize() {
        let mut layout = TilingLayout::default();
        let first = TiledLayer::add_first(&mut layout);
        let _second = TiledLayer::split(&mut layout, first, SplitDirection::Vertical).unwrap();

        // Initial bounds: 50/50
        let before = TiledLayer::arrange(&layout, Rect::new(0, 0, 80, 24));
        assert_eq!(before[0].bounds.width, 40);

        // Resize first window to grow right
        TiledLayer::resize(&mut layout, first, NavigateDirection::Right, 5);

        // After resize: first should be larger
        let after = TiledLayer::arrange(&layout, Rect::new(0, 0, 80, 24));
        assert!(after[0].bounds.width > 40);
    }

    #[test]
    fn test_tiled_layer_with_layer_context() {
        let layout = TilingLayout::new(LayerId::new(5), Zone::Tiled, ZOrder::new(500));
        let mut layout = layout;
        let id = TiledLayer::add_first(&mut layout);

        let placements = TiledLayer::arrange(&layout, Rect::new(0, 0, 80, 24));

        assert_eq!(placements[0].layer_id, LayerId::new(5));
        assert_eq!(placements[0].zone, Zone::Tiled);
        assert_eq!(placements[0].z_order, ZOrder::new(500));
        assert_eq!(placements[0].window_id, id);
    }
}
