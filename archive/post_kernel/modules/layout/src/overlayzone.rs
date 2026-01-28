//! Overlay window zone implementation.
//!
//! This module provides `OverlayZone`, which implements the `OverlayLayer` trait
//! from the display driver. Overlay windows are positioned using anchor constraints
//! and rendered above all tiled and floating windows within a layer.
//!
//! # Architecture
//!
//! Following the mechanism vs policy principle:
//! - **Mechanism** (display driver): `OverlayLayer` trait defines WHAT can be done
//! - **Policy** (this module): `OverlayZone` decides HOW overlays are positioned
//!
//! # Z-Order
//!
//! Overlay zone uses z-order slots 50-99 within each layer (50 windows max).
//! The `z_stack` vector maintains stacking order (index 0 = bottom, last = top).
//!
//! ```text
//! Layer 0: Overlay windows at z = 50, 51, 52... (based on z_stack position)
//! ```
//!
//! # Anchor Resolution
//!
//! Overlays are positioned using anchors that can reference:
//! - **Cursor**: Position relative to cursor in a window
//! - **Screen**: Absolute screen position
//! - **Center**: Centered on screen
//! - **Below**: Below another window
//!
//! Anchor resolution requires knowledge of existing window placements (from tiled
//! and float zones), which are passed to `arrange()`.

use std::collections::HashMap;

use reovim_driver_display::{
    Rect, WindowId,
    layout::{
        Anchor, LayerId, OverlayConstraints, OverlayLayer, OverlayWindow, WindowPlacement, ZOrder,
        Zone,
    },
};

/// Maximum number of overlay windows per layer.
///
/// Z-order slots 50-99 = 50 windows maximum.
pub const MAX_OVERLAY_WINDOWS: usize = 50;

/// Minimum overlay width in cells.
pub const MIN_OVERLAY_WIDTH: u16 = 5;

/// Minimum overlay height in cells.
pub const MIN_OVERLAY_HEIGHT: u16 = 1;

/// Overlay window zone manager.
///
/// Manages temporary UI elements like popups and menus within a layer's
/// overlay zone. Overlays are positioned using anchor constraints.
///
/// # Example
///
/// ```ignore
/// use reovim_module_layout::OverlayZone;
/// use reovim_driver_display::{Rect, WindowId, layout::{LayerId, ZOrder, OverlayConstraints, OverlayLayer}};
///
/// let mut zone = OverlayZone::new(LayerId::new(0), ZOrder::new(0));
/// let id = WindowId::from_raw(1);
/// let constraints = OverlayConstraints::centered().with_size(40, 10);
/// zone.show(id, constraints);
///
/// let placements = zone.arrange(Rect::new(0, 0, 80, 24), &[]);
/// assert_eq!(placements.len(), 1);
/// ```
#[derive(Debug, Clone)]
pub struct OverlayZone {
    /// Overlays by ID.
    overlays: HashMap<WindowId, OverlayWindow>,
    /// Z-stack: bottom to top (index 0 = lowest, last = highest).
    z_stack: Vec<WindowId>,
    /// Layer ID this zone belongs to.
    layer_id: LayerId,
    /// Base z-order for the layer.
    z_base: ZOrder,
}

impl OverlayZone {
    /// Create a new empty overlay zone.
    ///
    /// # Arguments
    ///
    /// * `layer_id` - The layer this zone belongs to
    /// * `z_base` - Base z-order for the layer
    #[must_use]
    pub fn new(layer_id: LayerId, z_base: ZOrder) -> Self {
        Self {
            overlays: HashMap::new(),
            z_stack: Vec::new(),
            layer_id,
            z_base,
        }
    }

    /// Check if the zone is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.overlays.is_empty()
    }

    /// Get the number of overlay windows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.overlays.len()
    }

    /// Check if at capacity (50 overlays max).
    #[must_use]
    pub fn is_at_capacity(&self) -> bool {
        self.overlays.len() >= MAX_OVERLAY_WINDOWS
    }

    /// Get the constraints of an overlay.
    #[must_use]
    pub fn constraints(&self, window: WindowId) -> Option<&OverlayConstraints> {
        self.overlays.get(&window).map(|ow| &ow.constraints)
    }

    /// Get the z-stack position of an overlay (0 = bottom).
    #[must_use]
    pub fn z_position(&self, window: WindowId) -> Option<usize> {
        self.z_stack.iter().position(|&id| id == window)
    }

    /// Resolve an anchor to screen coordinates.
    ///
    /// # Arguments
    ///
    /// * `anchor` - The anchor to resolve
    /// * `screen` - Screen bounds for clamping
    /// * `width` - Overlay width for clamping calculation
    /// * `height` - Overlay height for clamping calculation
    /// * `placements` - Existing window placements for Cursor/Below anchors
    ///
    /// # Returns
    ///
    /// `(x, y)` screen coordinates. Falls back to `(0, 0)` if anchor references
    /// a window that doesn't exist in placements.
    fn resolve_anchor(
        &self,
        anchor: &Anchor,
        screen: Rect,
        width: u16,
        height: u16,
        placements: &[WindowPlacement],
    ) -> (u16, u16) {
        match anchor {
            Anchor::Cursor { window, line, col } => {
                // Find the referenced window in placements
                if let Some(wp) = placements.iter().find(|p| p.window_id == *window) {
                    // Position below cursor: window bounds + cursor offset
                    let x = wp.bounds.x.saturating_add(col.as_usize() as u16);
                    // +1 to position below the cursor line
                    let y = wp
                        .bounds
                        .y
                        .saturating_add(line.as_usize() as u16)
                        .saturating_add(1);
                    self.clamp_position(x, y, width, height, screen)
                } else {
                    // Window not found - fall back to origin (will be clamped)
                    (0, 0)
                }
            }
            Anchor::Screen { x, y } => self.clamp_position(*x, *y, width, height, screen),
            Anchor::Center => {
                // Center on screen
                let x = screen
                    .x
                    .saturating_add(screen.width.saturating_sub(width) / 2);
                let y = screen
                    .y
                    .saturating_add(screen.height.saturating_sub(height) / 2);
                (x, y)
            }
            Anchor::Below(window_id) => {
                // Find the referenced window in placements
                if let Some(wp) = placements.iter().find(|p| p.window_id == *window_id) {
                    // Position directly below the window
                    let x = wp.bounds.x;
                    let y = wp.bounds.y.saturating_add(wp.bounds.height);
                    self.clamp_position(x, y, width, height, screen)
                } else {
                    // Window not found - fall back to origin
                    (0, 0)
                }
            }
        }
    }

    /// Clamp position so overlay stays within screen bounds.
    ///
    /// # Arguments
    ///
    /// * `x`, `y` - Desired position
    /// * `width`, `height` - Overlay dimensions
    /// * `screen` - Screen bounds
    ///
    /// # Returns
    ///
    /// Clamped `(x, y)` ensuring overlay fits on screen.
    fn clamp_position(&self, x: u16, y: u16, width: u16, height: u16, screen: Rect) -> (u16, u16) {
        // Calculate maximum allowed position so overlay stays on screen
        let max_x = screen
            .x
            .saturating_add(screen.width)
            .saturating_sub(width)
            .max(screen.x);
        let max_y = screen
            .y
            .saturating_add(screen.height)
            .saturating_sub(height)
            .max(screen.y);

        // Clamp position to valid range
        let clamped_x = x.clamp(screen.x, max_x);
        let clamped_y = y.clamp(screen.y, max_y);

        (clamped_x, clamped_y)
    }

    /// Calculate overlay dimensions from constraints.
    ///
    /// Uses preferred size if specified, otherwise defaults to minimums.
    /// Clamps to max size if specified.
    fn calculate_size(&self, constraints: &OverlayConstraints) -> (u16, u16) {
        let mut width = constraints.preferred_width.unwrap_or(MIN_OVERLAY_WIDTH);
        let mut height = constraints.preferred_height.unwrap_or(MIN_OVERLAY_HEIGHT);

        // Apply minimum constraints
        width = width.max(MIN_OVERLAY_WIDTH);
        height = height.max(MIN_OVERLAY_HEIGHT);

        // Apply maximum constraints if specified
        if let Some(max_w) = constraints.max_width {
            width = width.min(max_w);
        }
        if let Some(max_h) = constraints.max_height {
            height = height.min(max_h);
        }

        (width, height)
    }
}

impl OverlayLayer for OverlayZone {
    /// Get all overlays as placements, sorted by z-order.
    ///
    /// Windows are returned in z-stack order (lowest first), with z-order
    /// computed as `z_base + Zone::Overlay.z_offset() + stack_index`.
    ///
    /// Anchor resolution uses the provided `window_placements` to find
    /// positions for `Cursor` and `Below` anchors.
    fn arrange(&self, screen: Rect, window_placements: &[WindowPlacement]) -> Vec<WindowPlacement> {
        self.z_stack
            .iter()
            .enumerate()
            .filter_map(|(index, &window_id)| {
                self.overlays.get(&window_id).map(|ow| {
                    // Calculate size from constraints
                    let (width, height) = self.calculate_size(&ow.constraints);

                    // Resolve anchor to screen position
                    let (x, y) = self.resolve_anchor(
                        &ow.constraints.anchor,
                        screen,
                        width,
                        height,
                        window_placements,
                    );

                    let bounds = Rect::new(x, y, width, height);
                    let z_order = ZOrder::for_window(self.z_base, Zone::Overlay, index as u16);

                    WindowPlacement::new(window_id, self.layer_id, Zone::Overlay, bounds, z_order)
                })
            })
            .collect()
    }

    /// Show an overlay with constraints.
    ///
    /// The overlay is added to the top of the z-stack (highest z-order).
    /// If at capacity (50 overlays), silently ignores the request.
    ///
    /// # Arguments
    ///
    /// * `id` - Window identifier (provided by caller)
    /// * `constraints` - Positioning and size constraints
    fn show(&mut self, id: WindowId, constraints: OverlayConstraints) {
        // Check capacity - silently reject if at limit
        if self.is_at_capacity() {
            return;
        }

        // If overlay already exists, update constraints and raise to top
        if self.overlays.contains_key(&id) {
            if let Some(ow) = self.overlays.get_mut(&id) {
                ow.constraints = constraints;
            }
            // Raise to top
            self.z_stack.retain(|&x| x != id);
            self.z_stack.push(id);
            return;
        }

        let z_order = ZOrder::for_window(self.z_base, Zone::Overlay, self.z_stack.len() as u16);
        let overlay_window = OverlayWindow {
            id,
            constraints,
            computed_bounds: Rect::default(), // Will be computed in arrange()
            z_order,
        };

        self.overlays.insert(id, overlay_window);
        self.z_stack.push(id); // Add to top of stack
    }

    /// Hide (remove) an overlay.
    ///
    /// # Returns
    ///
    /// `true` if the overlay existed and was hidden.
    fn hide(&mut self, id: WindowId) -> bool {
        if self.overlays.remove(&id).is_some() {
            self.z_stack.retain(|&x| x != id);
            true
        } else {
            false
        }
    }

    /// Update overlay size (content changed).
    ///
    /// Updates the preferred size in constraints.
    /// Does nothing if the overlay doesn't exist.
    fn update_size(&mut self, id: WindowId, width: u16, height: u16) {
        if let Some(ow) = self.overlays.get_mut(&id) {
            ow.constraints.preferred_width = Some(width.max(MIN_OVERLAY_WIDTH));
            ow.constraints.preferred_height = Some(height.max(MIN_OVERLAY_HEIGHT));
        }
    }

    /// Check if overlay is visible.
    fn is_visible(&self, id: WindowId) -> bool {
        self.overlays.contains_key(&id)
    }

    /// Get all visible overlay IDs.
    fn visible_overlays(&self) -> Vec<WindowId> {
        self.z_stack.clone()
    }

    /// Hide all overlays.
    fn hide_all(&mut self) {
        self.overlays.clear();
        self.z_stack.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn test_zone() -> OverlayZone {
        OverlayZone::new(LayerId::new(0), ZOrder::new(0))
    }

    fn test_screen() -> Rect {
        Rect::new(0, 0, 80, 24)
    }

    fn centered_constraints() -> OverlayConstraints {
        OverlayConstraints::centered().with_size(40, 10)
    }

    #[test]
    fn test_overlay_zone_new() {
        let zone = test_zone();
        assert!(zone.is_empty());
        assert_eq!(zone.len(), 0);
        assert!(!zone.is_at_capacity());
    }

    #[test]
    fn test_show_overlay() {
        let mut zone = test_zone();
        let id = WindowId::from_raw(1);
        let constraints = centered_constraints();

        zone.show(id, constraints);

        assert!(zone.is_visible(id));
        assert_eq!(zone.len(), 1);
        assert!(zone.constraints(id).is_some());
    }

    #[test]
    fn test_hide_overlay() {
        let mut zone = test_zone();
        let id = WindowId::from_raw(1);

        zone.show(id, centered_constraints());
        assert!(zone.is_visible(id));

        let hidden = zone.hide(id);

        assert!(hidden);
        assert!(!zone.is_visible(id));
        assert_eq!(zone.len(), 0);
    }

    #[test]
    fn test_hide_nonexistent_overlay() {
        let mut zone = test_zone();
        let id = WindowId::from_raw(999);

        let hidden = zone.hide(id);

        assert!(!hidden);
        assert!(zone.is_empty());
    }

    #[test]
    fn test_overlay_z_order() {
        let mut zone = test_zone();
        let id1 = WindowId::from_raw(1);
        let id2 = WindowId::from_raw(2);
        let id3 = WindowId::from_raw(3);

        zone.show(id1, centered_constraints());
        zone.show(id2, centered_constraints());
        zone.show(id3, centered_constraints());

        // Z-stack order: id1 (bottom), id2, id3 (top)
        assert_eq!(zone.z_position(id1), Some(0));
        assert_eq!(zone.z_position(id2), Some(1));
        assert_eq!(zone.z_position(id3), Some(2));

        let placements = zone.arrange(test_screen(), &[]);

        // Verify z-order increases (overlay base is 50)
        assert_eq!(placements[0].z_order.as_u16(), 50);
        assert_eq!(placements[1].z_order.as_u16(), 51);
        assert_eq!(placements[2].z_order.as_u16(), 52);
    }

    #[test]
    fn test_anchor_cursor_resolution() {
        let mut zone = test_zone();
        let overlay_id = WindowId::from_raw(100);
        let window_id = WindowId::from_raw(1);

        // Create overlay anchored to cursor in window at line 5, col 10
        let constraints = OverlayConstraints::at_cursor_raw(window_id, 5, 10).with_size(20, 5);
        zone.show(overlay_id, constraints);

        // Create a placement for the referenced window
        let window_placement = WindowPlacement::new(
            window_id,
            LayerId::new(0),
            Zone::Tiled,
            Rect::new(0, 0, 40, 20), // Window at (0,0) with size 40x20
            ZOrder::new(0),
        );

        let placements = zone.arrange(test_screen(), &[window_placement]);

        assert_eq!(placements.len(), 1);
        // Expected: x = 0 + 10 = 10, y = 0 + 5 + 1 = 6 (below cursor)
        assert_eq!(placements[0].bounds.x, 10);
        assert_eq!(placements[0].bounds.y, 6);
    }

    #[test]
    fn test_anchor_screen_resolution() {
        let mut zone = test_zone();
        let id = WindowId::from_raw(1);

        // Anchor to specific screen position
        let constraints = OverlayConstraints::at_position(30, 10).with_size(20, 5);
        zone.show(id, constraints);

        let placements = zone.arrange(test_screen(), &[]);

        assert_eq!(placements.len(), 1);
        assert_eq!(placements[0].bounds.x, 30);
        assert_eq!(placements[0].bounds.y, 10);
    }

    #[test]
    fn test_anchor_center_resolution() {
        let mut zone = test_zone();
        let id = WindowId::from_raw(1);

        let constraints = OverlayConstraints::centered().with_size(20, 10);
        zone.show(id, constraints);

        let screen = Rect::new(0, 0, 80, 24);
        let placements = zone.arrange(screen, &[]);

        assert_eq!(placements.len(), 1);
        // Center: x = (80 - 20) / 2 = 30, y = (24 - 10) / 2 = 7
        assert_eq!(placements[0].bounds.x, 30);
        assert_eq!(placements[0].bounds.y, 7);
    }

    #[test]
    fn test_anchor_below_resolution() {
        let mut zone = test_zone();
        let overlay_id = WindowId::from_raw(100);
        let window_id = WindowId::from_raw(1);

        // Create overlay anchored below a window
        let constraints = OverlayConstraints {
            anchor: Anchor::Below(window_id),
            preferred_width: Some(30),
            preferred_height: Some(5),
            max_width: None,
            max_height: None,
        };
        zone.show(overlay_id, constraints);

        // Create a placement for the referenced window at (10, 5) with height 10
        let window_placement = WindowPlacement::new(
            window_id,
            LayerId::new(0),
            Zone::Tiled,
            Rect::new(10, 5, 40, 10),
            ZOrder::new(0),
        );

        let placements = zone.arrange(test_screen(), &[window_placement]);

        assert_eq!(placements.len(), 1);
        // Expected: x = 10 (same as window), y = 5 + 10 = 15 (below window)
        assert_eq!(placements[0].bounds.x, 10);
        assert_eq!(placements[0].bounds.y, 15);
    }

    #[test]
    fn test_clamp_to_screen() {
        let mut zone = test_zone();
        let id = WindowId::from_raw(1);

        // Anchor to position that would go off screen
        let constraints = OverlayConstraints::at_position(70, 20).with_size(20, 10);
        zone.show(id, constraints);

        let screen = Rect::new(0, 0, 80, 24);
        let placements = zone.arrange(screen, &[]);

        assert_eq!(placements.len(), 1);
        // Should be clamped: max_x = 80 - 20 = 60, max_y = 24 - 10 = 14
        assert_eq!(placements[0].bounds.x, 60);
        assert_eq!(placements[0].bounds.y, 14);
    }

    #[test]
    fn test_max_overlay_capacity() {
        let mut zone = test_zone();

        // Create MAX_OVERLAY_WINDOWS overlays
        for i in 0..MAX_OVERLAY_WINDOWS {
            let id = WindowId::from_raw(i);
            zone.show(id, centered_constraints());
        }

        assert_eq!(zone.len(), MAX_OVERLAY_WINDOWS);
        assert!(zone.is_at_capacity());

        // Try to create one more - should be silently ignored
        let extra_id = WindowId::from_raw(999);
        zone.show(extra_id, centered_constraints());

        assert_eq!(zone.len(), MAX_OVERLAY_WINDOWS);
        assert!(!zone.is_visible(extra_id));
    }

    #[test]
    fn test_update_size() {
        let mut zone = test_zone();
        let id = WindowId::from_raw(1);

        zone.show(id, OverlayConstraints::centered().with_size(20, 10));

        // Update size
        zone.update_size(id, 40, 15);

        let constraints = zone.constraints(id).unwrap();
        assert_eq!(constraints.preferred_width, Some(40));
        assert_eq!(constraints.preferred_height, Some(15));
    }

    #[test]
    fn test_hide_all() {
        let mut zone = test_zone();

        zone.show(WindowId::from_raw(1), centered_constraints());
        zone.show(WindowId::from_raw(2), centered_constraints());
        zone.show(WindowId::from_raw(3), centered_constraints());

        assert_eq!(zone.len(), 3);

        zone.hide_all();

        assert!(zone.is_empty());
        assert_eq!(zone.len(), 0);
    }

    #[test]
    fn test_arrange_sorted_by_z() {
        let mut zone = test_zone();
        let id1 = WindowId::from_raw(1);
        let id2 = WindowId::from_raw(2);
        let id3 = WindowId::from_raw(3);

        zone.show(id1, OverlayConstraints::at_position(0, 0).with_size(10, 5));
        zone.show(id2, OverlayConstraints::at_position(10, 5).with_size(15, 5));
        zone.show(id3, OverlayConstraints::at_position(20, 10).with_size(20, 5));

        let placements = zone.arrange(test_screen(), &[]);

        assert_eq!(placements.len(), 3);

        // Verify z-order increases
        assert!(placements[0].z_order < placements[1].z_order);
        assert!(placements[1].z_order < placements[2].z_order);

        // Verify order matches z_stack
        assert_eq!(placements[0].window_id, id1);
        assert_eq!(placements[1].window_id, id2);
        assert_eq!(placements[2].window_id, id3);

        // Verify zone is Overlay
        for p in &placements {
            assert_eq!(p.zone, Zone::Overlay);
        }
    }

    #[test]
    fn test_anchor_cursor_window_not_in_placements() {
        let mut zone = test_zone();
        let overlay_id = WindowId::from_raw(100);
        let nonexistent_window = WindowId::from_raw(999);

        // Anchor to window that doesn't exist in placements
        let constraints =
            OverlayConstraints::at_cursor_raw(nonexistent_window, 5, 10).with_size(20, 5);
        zone.show(overlay_id, constraints);

        // Empty placements - window won't be found
        let placements = zone.arrange(test_screen(), &[]);

        assert_eq!(placements.len(), 1);
        // Should fall back to (0, 0)
        assert_eq!(placements[0].bounds.x, 0);
        assert_eq!(placements[0].bounds.y, 0);
    }

    #[test]
    fn test_anchor_below_window_not_in_placements() {
        let mut zone = test_zone();
        let overlay_id = WindowId::from_raw(100);
        let nonexistent_window = WindowId::from_raw(999);

        // Anchor below window that doesn't exist
        let constraints = OverlayConstraints {
            anchor: Anchor::Below(nonexistent_window),
            preferred_width: Some(20),
            preferred_height: Some(5),
            max_width: None,
            max_height: None,
        };
        zone.show(overlay_id, constraints);

        let placements = zone.arrange(test_screen(), &[]);

        assert_eq!(placements.len(), 1);
        // Should fall back to (0, 0)
        assert_eq!(placements[0].bounds.x, 0);
        assert_eq!(placements[0].bounds.y, 0);
    }

    #[test]
    fn test_visible_overlays_returns_all_ids() {
        let mut zone = test_zone();
        let id1 = WindowId::from_raw(1);
        let id2 = WindowId::from_raw(2);

        zone.show(id1, centered_constraints());
        zone.show(id2, centered_constraints());

        let visible = zone.visible_overlays();

        assert_eq!(visible.len(), 2);
        assert!(visible.contains(&id1));
        assert!(visible.contains(&id2));
    }

    #[test]
    fn test_is_visible() {
        let mut zone = test_zone();
        let id1 = WindowId::from_raw(1);
        let id2 = WindowId::from_raw(2);

        zone.show(id1, centered_constraints());

        assert!(zone.is_visible(id1));
        assert!(!zone.is_visible(id2));
    }

    #[test]
    fn test_z_order_values_are_in_overlay_zone_range() {
        let mut zone = OverlayZone::new(LayerId::new(0), ZOrder::new(0));
        let id1 = WindowId::from_raw(1);
        let id2 = WindowId::from_raw(2);

        zone.show(id1, centered_constraints());
        zone.show(id2, centered_constraints());

        let placements = zone.arrange(test_screen(), &[]);

        // Overlay zone offset is 50
        let overlay_offset = Zone::Overlay.z_offset();
        assert_eq!(overlay_offset, 50);

        // First window: z = 0 + 50 + 0 = 50
        assert_eq!(placements[0].z_order.as_u16(), 50);
        // Second window: z = 0 + 50 + 1 = 51
        assert_eq!(placements[1].z_order.as_u16(), 51);
    }

    #[test]
    fn test_arrange_empty_zone_returns_empty() {
        let zone = test_zone();
        let placements = zone.arrange(test_screen(), &[]);
        assert!(placements.is_empty());
    }

    #[test]
    fn test_show_existing_overlay_updates_and_raises() {
        let mut zone = test_zone();
        let id1 = WindowId::from_raw(1);
        let id2 = WindowId::from_raw(2);

        zone.show(id1, OverlayConstraints::at_position(10, 10).with_size(20, 5));
        zone.show(id2, centered_constraints());

        // id1 is at bottom (position 0), id2 is at top (position 1)
        assert_eq!(zone.z_position(id1), Some(0));
        assert_eq!(zone.z_position(id2), Some(1));

        // Show id1 again with different constraints - should update and raise
        zone.show(id1, OverlayConstraints::at_position(30, 20).with_size(25, 8));

        // id1 should now be at top
        assert_eq!(zone.z_position(id2), Some(0));
        assert_eq!(zone.z_position(id1), Some(1));

        // Constraints should be updated
        let constraints = zone.constraints(id1).unwrap();
        assert_eq!(constraints.preferred_width, Some(25));
        assert_eq!(constraints.preferred_height, Some(8));
    }

    #[test]
    fn test_update_size_clamps_to_minimum() {
        let mut zone = test_zone();
        let id = WindowId::from_raw(1);

        zone.show(id, centered_constraints());

        // Try to update to size smaller than minimum
        zone.update_size(id, 2, 0);

        let constraints = zone.constraints(id).unwrap();
        assert_eq!(constraints.preferred_width, Some(MIN_OVERLAY_WIDTH));
        assert_eq!(constraints.preferred_height, Some(MIN_OVERLAY_HEIGHT));
    }

    #[test]
    fn test_update_size_nonexistent_is_noop() {
        let mut zone = test_zone();
        let id = WindowId::from_raw(999);

        // Should not panic
        zone.update_size(id, 50, 50);

        assert!(!zone.is_visible(id));
    }
}
