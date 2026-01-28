//! Floating window zone implementation.
//!
//! This module provides `FloatZone`, which implements the `FloatingLayer` trait
//! from the display driver. Floating windows can be freely positioned and sized,
//! rendered above tiled windows but below overlays.
//!
//! # Architecture
//!
//! Following the mechanism vs policy principle:
//! - **Mechanism** (display driver): `FloatingLayer` trait defines WHAT can be done
//! - **Policy** (this module): `FloatZone` decides HOW windows are managed
//!
//! # Z-Order
//!
//! Float zone uses z-order slots 10-49 within each layer (40 windows max).
//! The `z_stack` vector maintains stacking order (index 0 = bottom, last = top).
//!
//! ```text
//! Layer 0: Float windows at z = 10, 11, 12... (based on z_stack position)
//! ```

use std::collections::HashMap;

use reovim_driver_display::{
    Rect, WindowId,
    layout::{FloatingLayer, FloatingWindow, LayerId, WindowPlacement, ZOrder, Zone},
};

/// Maximum number of floating windows per layer.
///
/// Z-order slots 10-49 = 40 windows maximum.
pub const MAX_FLOAT_WINDOWS: usize = 40;

/// Minimum float window width in cells.
pub const MIN_FLOAT_WIDTH: u16 = 10;

/// Minimum float window height in cells.
pub const MIN_FLOAT_HEIGHT: u16 = 5;

/// Floating window zone manager.
///
/// Manages freely-positioned windows within a layer's float zone.
/// Windows can be moved, resized, and reordered in z-order.
///
/// # Example
///
/// ```ignore
/// use reovim_module_layout::FloatZone;
/// use reovim_driver_display::{Rect, WindowId, layout::{LayerId, ZOrder, FloatingLayer}};
///
/// let mut zone = FloatZone::new(LayerId::new(0), ZOrder::new(0));
/// let id = WindowId::from_raw(1);
/// zone.create(id, Rect::new(10, 5, 60, 20));
/// zone.raise(id);  // Bring to front
///
/// let placements = zone.arrange();
/// assert_eq!(placements.len(), 1);
/// ```
#[derive(Debug, Clone)]
pub struct FloatZone {
    /// Float windows by ID.
    windows: HashMap<WindowId, FloatingWindow>,
    /// Z-stack: bottom to top (index 0 = lowest, last = highest).
    z_stack: Vec<WindowId>,
    /// Layer ID this zone belongs to.
    layer_id: LayerId,
    /// Base z-order for the layer.
    z_base: ZOrder,
}

impl FloatZone {
    /// Create a new empty float zone.
    ///
    /// # Arguments
    ///
    /// * `layer_id` - The layer this zone belongs to
    /// * `z_base` - Base z-order for the layer
    #[must_use]
    pub fn new(layer_id: LayerId, z_base: ZOrder) -> Self {
        Self {
            windows: HashMap::new(),
            z_stack: Vec::new(),
            layer_id,
            z_base,
        }
    }

    /// Check if the zone is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    /// Get the number of floating windows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.windows.len()
    }

    /// Check if at capacity (40 windows max).
    #[must_use]
    pub fn is_at_capacity(&self) -> bool {
        self.windows.len() >= MAX_FLOAT_WINDOWS
    }

    /// Get the bounds of a floating window.
    #[must_use]
    pub fn bounds(&self, window: WindowId) -> Option<Rect> {
        self.windows.get(&window).map(|fw| fw.bounds)
    }

    /// Get the z-stack position of a window (0 = bottom).
    #[must_use]
    pub fn z_position(&self, window: WindowId) -> Option<usize> {
        self.z_stack.iter().position(|&id| id == window)
    }
}

impl FloatingLayer for FloatZone {
    /// Get all floating windows as placements, sorted by z-order.
    ///
    /// Windows are returned in z-stack order (lowest first), with z-order
    /// computed as `z_base + Zone::Float.z_offset() + stack_index`.
    fn arrange(&self) -> Vec<WindowPlacement> {
        self.z_stack
            .iter()
            .enumerate()
            .filter_map(|(index, &window_id)| {
                self.windows.get(&window_id).map(|fw| {
                    WindowPlacement::new(
                        window_id,
                        self.layer_id,
                        Zone::Float,
                        fw.bounds,
                        ZOrder::for_window(self.z_base, Zone::Float, index as u16),
                    )
                })
            })
            .collect()
    }

    /// Create a new floating window.
    ///
    /// The window is added to the top of the z-stack (highest z-order).
    /// If at capacity (40 windows), logs a warning and returns the same ID
    /// without creating the window.
    ///
    /// # Arguments
    ///
    /// * `id` - Window identifier (provided by caller)
    /// * `bounds` - Initial position and size
    ///
    /// # Returns
    ///
    /// The window ID (same as input).
    fn create(&mut self, id: WindowId, bounds: Rect) -> WindowId {
        // Check capacity - silently reject if at limit (40 windows max)
        if self.is_at_capacity() {
            return id;
        }

        // Clamp bounds to minimum size
        let clamped_bounds = Rect::new(
            bounds.x,
            bounds.y,
            bounds.width.max(MIN_FLOAT_WIDTH),
            bounds.height.max(MIN_FLOAT_HEIGHT),
        );

        let z_order = ZOrder::for_window(self.z_base, Zone::Float, self.z_stack.len() as u16);
        let floating_window = FloatingWindow {
            id,
            bounds: clamped_bounds,
            z_order,
        };

        self.windows.insert(id, floating_window);
        self.z_stack.push(id); // Add to top of stack
        id
    }

    /// Close a floating window.
    ///
    /// # Returns
    ///
    /// `true` if the window existed and was closed.
    fn close(&mut self, window: WindowId) -> bool {
        if self.windows.remove(&window).is_some() {
            self.z_stack.retain(|&id| id != window);
            true
        } else {
            false
        }
    }

    /// Move window to a new position.
    ///
    /// Does nothing if the window doesn't exist.
    fn move_to(&mut self, window: WindowId, x: u16, y: u16) {
        if let Some(fw) = self.windows.get_mut(&window) {
            fw.bounds.x = x;
            fw.bounds.y = y;
        }
    }

    /// Resize window.
    ///
    /// Dimensions are clamped to minimum (10x5).
    /// Does nothing if the window doesn't exist.
    fn resize(&mut self, window: WindowId, width: u16, height: u16) {
        if let Some(fw) = self.windows.get_mut(&window) {
            fw.bounds.width = width.max(MIN_FLOAT_WIDTH);
            fw.bounds.height = height.max(MIN_FLOAT_HEIGHT);
        }
    }

    /// Bring window to front (highest z-order).
    ///
    /// Moves the window to the top of the z-stack.
    /// Does nothing if the window doesn't exist.
    fn raise(&mut self, window: WindowId) {
        if let Some(pos) = self.z_stack.iter().position(|&id| id == window) {
            self.z_stack.remove(pos);
            self.z_stack.push(window);
        }
    }

    /// Send window to back (lowest z-order in float zone).
    ///
    /// Moves the window to the bottom of the z-stack.
    /// Does nothing if the window doesn't exist.
    fn lower(&mut self, window: WindowId) {
        if let Some(pos) = self.z_stack.iter().position(|&id| id == window) {
            self.z_stack.remove(pos);
            self.z_stack.insert(0, window);
        }
    }

    /// Get all floating window IDs.
    fn windows(&self) -> Vec<WindowId> {
        self.z_stack.clone()
    }

    /// Check if window is in floating layer.
    fn contains(&self, window: WindowId) -> bool {
        self.windows.contains_key(&window)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_zone() -> FloatZone {
        FloatZone::new(LayerId::new(0), ZOrder::new(0))
    }

    fn test_bounds() -> Rect {
        Rect::new(10, 5, 60, 20)
    }

    #[test]
    fn test_float_zone_new() {
        let zone = test_zone();
        assert!(zone.is_empty());
        assert_eq!(zone.len(), 0);
        assert!(!zone.is_at_capacity());
    }

    #[test]
    fn test_create_float() {
        let mut zone = test_zone();
        let id = WindowId::from_raw(1);
        let bounds = test_bounds();

        let returned_id = zone.create(id, bounds);

        assert_eq!(returned_id, id);
        assert!(zone.contains(id));
        assert_eq!(zone.len(), 1);
        assert_eq!(zone.bounds(id), Some(bounds));
    }

    #[test]
    fn test_create_multiple_floats() {
        let mut zone = test_zone();
        let id1 = WindowId::from_raw(1);
        let id2 = WindowId::from_raw(2);
        let id3 = WindowId::from_raw(3);

        zone.create(id1, Rect::new(0, 0, 20, 10));
        zone.create(id2, Rect::new(10, 5, 30, 15));
        zone.create(id3, Rect::new(20, 10, 40, 20));

        assert_eq!(zone.len(), 3);
        assert!(zone.contains(id1));
        assert!(zone.contains(id2));
        assert!(zone.contains(id3));

        // Z-stack order: id1 (bottom), id2, id3 (top)
        assert_eq!(zone.z_position(id1), Some(0));
        assert_eq!(zone.z_position(id2), Some(1));
        assert_eq!(zone.z_position(id3), Some(2));
    }

    #[test]
    fn test_move_float() {
        let mut zone = test_zone();
        let id = WindowId::from_raw(1);
        zone.create(id, Rect::new(10, 10, 40, 20));

        zone.move_to(id, 50, 30);

        let bounds = zone.bounds(id).unwrap();
        assert_eq!(bounds.x, 50);
        assert_eq!(bounds.y, 30);
        // Size unchanged
        assert_eq!(bounds.width, 40);
        assert_eq!(bounds.height, 20);
    }

    #[test]
    fn test_resize_float() {
        let mut zone = test_zone();
        let id = WindowId::from_raw(1);
        zone.create(id, Rect::new(10, 10, 40, 20));

        zone.resize(id, 80, 30);

        let bounds = zone.bounds(id).unwrap();
        // Position unchanged
        assert_eq!(bounds.x, 10);
        assert_eq!(bounds.y, 10);
        // Size changed
        assert_eq!(bounds.width, 80);
        assert_eq!(bounds.height, 30);
    }

    #[test]
    fn test_resize_float_clamps_to_minimum() {
        let mut zone = test_zone();
        let id = WindowId::from_raw(1);
        zone.create(id, Rect::new(10, 10, 40, 20));

        zone.resize(id, 5, 2); // Below minimum

        let bounds = zone.bounds(id).unwrap();
        assert_eq!(bounds.width, MIN_FLOAT_WIDTH);
        assert_eq!(bounds.height, MIN_FLOAT_HEIGHT);
    }

    #[test]
    fn test_raise_float() {
        let mut zone = test_zone();
        let id1 = WindowId::from_raw(1);
        let id2 = WindowId::from_raw(2);
        let id3 = WindowId::from_raw(3);

        zone.create(id1, test_bounds());
        zone.create(id2, test_bounds());
        zone.create(id3, test_bounds());

        // Initially: id1 (0), id2 (1), id3 (2)
        assert_eq!(zone.z_position(id1), Some(0));

        // Raise id1 to top
        zone.raise(id1);

        // Now: id2 (0), id3 (1), id1 (2)
        assert_eq!(zone.z_position(id2), Some(0));
        assert_eq!(zone.z_position(id3), Some(1));
        assert_eq!(zone.z_position(id1), Some(2));
    }

    #[test]
    fn test_lower_float() {
        let mut zone = test_zone();
        let id1 = WindowId::from_raw(1);
        let id2 = WindowId::from_raw(2);
        let id3 = WindowId::from_raw(3);

        zone.create(id1, test_bounds());
        zone.create(id2, test_bounds());
        zone.create(id3, test_bounds());

        // Initially: id1 (0), id2 (1), id3 (2)
        assert_eq!(zone.z_position(id3), Some(2));

        // Lower id3 to bottom
        zone.lower(id3);

        // Now: id3 (0), id1 (1), id2 (2)
        assert_eq!(zone.z_position(id3), Some(0));
        assert_eq!(zone.z_position(id1), Some(1));
        assert_eq!(zone.z_position(id2), Some(2));
    }

    #[test]
    fn test_close_float() {
        let mut zone = test_zone();
        let id1 = WindowId::from_raw(1);
        let id2 = WindowId::from_raw(2);

        zone.create(id1, test_bounds());
        zone.create(id2, test_bounds());
        assert_eq!(zone.len(), 2);

        let closed = zone.close(id1);

        assert!(closed);
        assert!(!zone.contains(id1));
        assert!(zone.contains(id2));
        assert_eq!(zone.len(), 1);
        assert_eq!(zone.z_position(id2), Some(0));
    }

    #[test]
    fn test_close_nonexistent() {
        let mut zone = test_zone();
        let id = WindowId::from_raw(999);

        let closed = zone.close(id);

        assert!(!closed);
        assert!(zone.is_empty());
    }

    #[test]
    fn test_arrange_sorted_by_z() {
        let mut zone = test_zone();
        let id1 = WindowId::from_raw(1);
        let id2 = WindowId::from_raw(2);
        let id3 = WindowId::from_raw(3);

        zone.create(id1, Rect::new(0, 0, 20, 10));
        zone.create(id2, Rect::new(10, 5, 30, 15));
        zone.create(id3, Rect::new(20, 10, 40, 20));

        let placements = zone.arrange();

        assert_eq!(placements.len(), 3);

        // Verify z-order increases
        assert!(placements[0].z_order < placements[1].z_order);
        assert!(placements[1].z_order < placements[2].z_order);

        // Verify order matches z_stack
        assert_eq!(placements[0].window_id, id1);
        assert_eq!(placements[1].window_id, id2);
        assert_eq!(placements[2].window_id, id3);

        // Verify zone is Float
        for p in &placements {
            assert_eq!(p.zone, Zone::Float);
        }
    }

    #[test]
    fn test_windows_returns_all_ids() {
        let mut zone = test_zone();
        let id1 = WindowId::from_raw(1);
        let id2 = WindowId::from_raw(2);

        zone.create(id1, test_bounds());
        zone.create(id2, test_bounds());

        let windows = zone.windows();

        assert_eq!(windows.len(), 2);
        assert!(windows.contains(&id1));
        assert!(windows.contains(&id2));
    }

    #[test]
    fn test_contains() {
        let mut zone = test_zone();
        let id1 = WindowId::from_raw(1);
        let id2 = WindowId::from_raw(2);

        zone.create(id1, test_bounds());

        assert!(zone.contains(id1));
        assert!(!zone.contains(id2));
    }

    #[test]
    fn test_float_at_capacity_40_windows() {
        let mut zone = test_zone();

        // Create 40 windows (at capacity)
        for i in 0..MAX_FLOAT_WINDOWS {
            let id = WindowId::from_raw(i);
            zone.create(id, test_bounds());
        }

        assert_eq!(zone.len(), MAX_FLOAT_WINDOWS);
        assert!(zone.is_at_capacity());

        // Try to create 41st window - should not increase count
        let id41 = WindowId::from_raw(999);
        zone.create(id41, test_bounds());

        assert_eq!(zone.len(), MAX_FLOAT_WINDOWS);
        assert!(!zone.contains(id41));
    }

    #[test]
    fn test_create_clamps_to_minimum_bounds() {
        let mut zone = test_zone();
        let id = WindowId::from_raw(1);

        // Create with bounds smaller than minimum
        zone.create(id, Rect::new(10, 10, 5, 2));

        let bounds = zone.bounds(id).unwrap();
        assert_eq!(bounds.x, 10); // Position unchanged
        assert_eq!(bounds.y, 10);
        assert_eq!(bounds.width, MIN_FLOAT_WIDTH); // Clamped
        assert_eq!(bounds.height, MIN_FLOAT_HEIGHT); // Clamped
    }

    #[test]
    fn test_move_nonexistent_window_is_noop() {
        let mut zone = test_zone();
        let id = WindowId::from_raw(999);

        // Should not panic
        zone.move_to(id, 50, 50);

        assert!(!zone.contains(id));
    }

    #[test]
    fn test_resize_nonexistent_window_is_noop() {
        let mut zone = test_zone();
        let id = WindowId::from_raw(999);

        // Should not panic
        zone.resize(id, 100, 100);

        assert!(!zone.contains(id));
    }

    #[test]
    fn test_raise_nonexistent_window_is_noop() {
        let mut zone = test_zone();
        let id = WindowId::from_raw(999);

        // Should not panic
        zone.raise(id);

        assert!(zone.is_empty());
    }

    #[test]
    fn test_lower_nonexistent_window_is_noop() {
        let mut zone = test_zone();
        let id = WindowId::from_raw(999);

        // Should not panic
        zone.lower(id);

        assert!(zone.is_empty());
    }

    #[test]
    fn test_z_order_values_are_in_float_zone_range() {
        let mut zone = FloatZone::new(LayerId::new(0), ZOrder::new(0));
        let id1 = WindowId::from_raw(1);
        let id2 = WindowId::from_raw(2);

        zone.create(id1, test_bounds());
        zone.create(id2, test_bounds());

        let placements = zone.arrange();

        // Float zone offset is 10
        let float_offset = Zone::Float.z_offset();
        assert_eq!(float_offset, 10);

        // First window: z = 0 + 10 + 0 = 10
        assert_eq!(placements[0].z_order.as_u16(), 10);
        // Second window: z = 0 + 10 + 1 = 11
        assert_eq!(placements[1].z_order.as_u16(), 11);
    }

    #[test]
    fn test_arrange_empty_zone_returns_empty() {
        let zone = test_zone();
        let placements = zone.arrange();
        assert!(placements.is_empty());
    }

    #[test]
    fn test_raise_already_top_is_noop() {
        let mut zone = test_zone();
        let id1 = WindowId::from_raw(1);
        let id2 = WindowId::from_raw(2);

        zone.create(id1, test_bounds());
        zone.create(id2, test_bounds()); // id2 is already top

        zone.raise(id2);

        // Order should be unchanged
        assert_eq!(zone.z_position(id1), Some(0));
        assert_eq!(zone.z_position(id2), Some(1));
    }

    #[test]
    fn test_lower_already_bottom_is_noop() {
        let mut zone = test_zone();
        let id1 = WindowId::from_raw(1);
        let id2 = WindowId::from_raw(2);

        zone.create(id1, test_bounds()); // id1 is already bottom
        zone.create(id2, test_bounds());

        zone.lower(id1);

        // Order should be unchanged
        assert_eq!(zone.z_position(id1), Some(0));
        assert_eq!(zone.z_position(id2), Some(1));
    }
}
