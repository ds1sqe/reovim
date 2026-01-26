//! Default layer implementation for the nested compositor.
//!
//! This module provides `DefaultLayer`, which implements `WindowLayerCompositor`
//! with tiled and float zones.
//!
//! # Architecture
//!
//! ```text
//! DefaultLayer (implements WindowLayerCompositor)
//! ├── Tiled Zone  (TilingLayout)  ✓ Implemented
//! ├── Float Zone  (FloatZone)     ✓ Implemented (#398)
//! └── Overlay Zone (stub)         ← Phase 3
//! ```
//!
//! # Focus Model
//!
//! `DefaultLayer` tracks the focused window within its zone. When a window
//! is added, split, or navigated to, the focus is updated automatically.

use reovim_driver_display::{
    NavigateDirection, Rect, SplitDirection, WindowId,
    layout::{
        FloatingLayer, LayerId, OverlayConstraints, TiledLayer, WindowLayerCompositor,
        WindowPlacement, ZOrder, Zone,
    },
};

use crate::{FloatZone, TilingLayout};

/// Default layer implementation with tiled and float zones.
///
/// This compositor wraps both `TilingLayout` (tiled zone) and `FloatZone`
/// (floating windows), implementing `WindowLayerCompositor`.
///
/// # Thread Safety
///
/// `DefaultLayer` is `Send + Sync` because both zone managers are `Send + Sync`.
#[derive(Debug, Clone)]
pub struct DefaultLayer {
    /// Layer identifier.
    id: LayerId,
    /// Human-readable label (e.g., "main").
    label: String,
    /// Tiled zone manager.
    tiled: TilingLayout,
    /// Float zone manager.
    float: FloatZone,
    /// Next float window ID (DefaultLayer generates IDs for float zone).
    next_float_id: usize,
    /// Currently focused window.
    focused: Option<WindowId>,
    /// Cached screen bounds for navigation/cycle.
    screen: Rect,
}

impl DefaultLayer {
    /// Create a new layer with the given ID and label.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique layer identifier
    /// * `label` - Human-readable label for shortcuts
    #[must_use]
    pub fn new(id: LayerId, label: impl Into<String>) -> Self {
        let z_base = ZOrder::layer_base(id); // Layer 0 = 0-99, Layer 1 = 100-199, etc.
        Self {
            id,
            label: label.into(),
            tiled: TilingLayout::new(id, Zone::Tiled, z_base),
            float: FloatZone::new(id, z_base),
            next_float_id: 1000, // Start float IDs at 1000 to avoid collision with tiled
            focused: None,
            screen: Rect::new(0, 0, 80, 24), // Default screen size
        }
    }

    /// Update the cached screen size.
    ///
    /// Called on terminal resize to update navigation/cycle calculations.
    pub fn set_screen(&mut self, screen: Rect) {
        self.screen = screen;
        self.tiled.set_screen(screen);
    }

    /// Get the layer label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Get the tiled layout (for testing/inspection).
    #[must_use]
    pub fn tiled(&self) -> &TilingLayout {
        &self.tiled
    }

    /// Get mutable access to tiled layout.
    #[must_use]
    pub fn tiled_mut(&mut self) -> &mut TilingLayout {
        &mut self.tiled
    }

    /// Get the float zone (for testing/inspection).
    #[must_use]
    pub fn float(&self) -> &FloatZone {
        &self.float
    }

    /// Compute centered float window bounds (80% of screen, centered).
    ///
    /// Ensures minimum size of 10x5 for very small screens.
    fn compute_centered_float_bounds(&self) -> Rect {
        // 80% of screen, minimum 10x5
        let width = ((self.screen.width * 80) / 100).max(crate::floatzone::MIN_FLOAT_WIDTH);
        let height = ((self.screen.height * 80) / 100).max(crate::floatzone::MIN_FLOAT_HEIGHT);
        let x = (self.screen.width.saturating_sub(width)) / 2;
        let y = (self.screen.height.saturating_sub(height)) / 2;
        Rect::new(x, y, width, height)
    }
}

impl WindowLayerCompositor for DefaultLayer {
    fn id(&self) -> LayerId {
        self.id
    }

    fn arrange(&self, bounds: Rect) -> Vec<WindowPlacement> {
        // Get placements from both zones
        let mut placements = TiledLayer::arrange(&self.tiled, bounds);
        placements.extend(FloatingLayer::arrange(&self.float));
        // Sort by z-order for correct rendering (tiled first, then float)
        placements.sort_by_key(|p| p.z_order);
        placements
    }

    // =========================================================================
    // Tiled Zone
    // =========================================================================

    fn add_tiled(&mut self) -> WindowId {
        let id = TiledLayer::add_first(&mut self.tiled);
        self.focused = Some(id);
        id
    }

    fn split_tiled(&mut self, from: WindowId, direction: SplitDirection) -> Option<WindowId> {
        let new_id = TiledLayer::split(&mut self.tiled, from, direction)?;
        self.focused = Some(new_id);
        Some(new_id)
    }

    fn navigate_tiled(&self, from: WindowId, direction: NavigateDirection) -> Option<WindowId> {
        let views = self.arrange(self.screen);
        TiledLayer::navigate(&self.tiled, from, direction, &views)
    }

    fn resize_tiled(&mut self, window: WindowId, direction: NavigateDirection, delta: i16) {
        TiledLayer::resize(&mut self.tiled, window, direction, delta);
    }

    fn close_tiled(&mut self, window: WindowId) -> Option<WindowId> {
        let neighbor = TiledLayer::close(&mut self.tiled, window);
        self.focused = neighbor;
        neighbor
    }

    fn equalize_tiled(&mut self) {
        TiledLayer::equalize(&mut self.tiled);
    }

    fn cycle_tiled(&self, from: WindowId, forward: bool) -> Option<WindowId> {
        let views = self.arrange(self.screen);
        TiledLayer::cycle(&self.tiled, from, forward, &views)
    }

    // =========================================================================
    // Float Zone (#398)
    // =========================================================================

    fn create_float(&mut self, bounds: Rect) -> WindowId {
        // Generate a new window ID (DefaultLayer owns ID allocation for floats)
        let id = WindowId::from_raw(self.next_float_id);
        self.next_float_id += 1;
        FloatingLayer::create(&mut self.float, id, bounds);
        self.focused = Some(id);
        id
    }

    fn move_float(&mut self, window: WindowId, x: u16, y: u16) {
        FloatingLayer::move_to(&mut self.float, window, x, y);
    }

    fn resize_float(&mut self, window: WindowId, width: u16, height: u16) {
        FloatingLayer::resize(&mut self.float, window, width, height);
    }

    fn raise_float(&mut self, window: WindowId) {
        FloatingLayer::raise(&mut self.float, window);
    }

    fn lower_float(&mut self, window: WindowId) {
        FloatingLayer::lower(&mut self.float, window);
    }

    fn close_float(&mut self, window: WindowId) {
        if FloatingLayer::close(&mut self.float, window) {
            // If we closed the focused window, move focus to another window
            if self.focused == Some(window) {
                // Prefer another float, else a tiled window
                let floats = FloatingLayer::windows(&self.float);
                let tiled = TiledLayer::windows(&self.tiled);
                self.focused = floats.first().copied().or_else(|| tiled.first().copied());
            }
        }
    }

    fn toggle_float(&mut self, window: WindowId) {
        if TiledLayer::windows(&self.tiled).contains(&window) {
            // Tiled -> Float: Remove from tiled, create in float
            TiledLayer::close(&mut self.tiled, window);
            let bounds = self.compute_centered_float_bounds();
            let new_id = self.create_float(bounds);
            self.focused = Some(new_id);
        } else if FloatingLayer::contains(&self.float, window) {
            // Float -> Tiled: Remove from float, add to tiled
            FloatingLayer::close(&mut self.float, window);

            if self.tiled.is_empty() {
                // Create first tiled window
                let new_id = TiledLayer::add_first(&mut self.tiled);
                self.focused = Some(new_id);
            } else {
                // Find a tiled window to split from
                // If focused was the float we just closed, pick any tiled window
                let split_target = self
                    .focused
                    .filter(|f| TiledLayer::windows(&self.tiled).contains(f))
                    .or_else(|| TiledLayer::windows(&self.tiled).first().copied());

                if let Some(target) = split_target
                    && let Some(new_id) =
                        TiledLayer::split(&mut self.tiled, target, SplitDirection::Vertical)
                {
                    self.focused = Some(new_id);
                }
            }
        }
        // Else: Window not in either zone, no-op
    }

    // =========================================================================
    // Overlay Zone (Phase 3 stubs)
    // =========================================================================

    fn show_overlay(&mut self, _constraints: OverlayConstraints) -> WindowId {
        unimplemented!("Overlay zone not implemented in Phase 1")
    }

    fn hide_overlay(&mut self, _window: WindowId) {
        unimplemented!("Overlay zone not implemented in Phase 1")
    }

    fn resize_overlay(&mut self, _window: WindowId, _width: u16, _height: u16) {
        unimplemented!("Overlay zone not implemented in Phase 1")
    }

    // =========================================================================
    // Focus
    // =========================================================================

    fn set_focus(&mut self, window: WindowId) {
        self.focused = Some(window);
    }

    fn focused(&self) -> Option<WindowId> {
        self.focused
    }

    fn windows_in_zone(&self, zone: Zone) -> Vec<WindowId> {
        match zone {
            Zone::Tiled => TiledLayer::windows(&self.tiled),
            Zone::Float => FloatingLayer::windows(&self.float),
            Zone::Overlay => Vec::new(), // Phase 3
        }
    }

    fn zone_of(&self, window: WindowId) -> Option<Zone> {
        if TiledLayer::windows(&self.tiled).contains(&window) {
            Some(Zone::Tiled)
        } else if FloatingLayer::contains(&self.float, window) {
            Some(Zone::Float)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_layer_new() {
        let layer = DefaultLayer::new(LayerId::new(0), "main");
        assert_eq!(layer.id(), LayerId::new(0));
        assert_eq!(layer.label(), "main");
        assert!(layer.focused().is_none());
    }

    #[test]
    fn test_default_layer_add_tiled() {
        let mut layer = DefaultLayer::new(LayerId::new(0), "main");
        let id = layer.add_tiled();

        assert_eq!(layer.focused(), Some(id));
        assert_eq!(layer.windows_in_zone(Zone::Tiled).len(), 1);
    }

    #[test]
    fn test_default_layer_split_tiled() {
        let mut layer = DefaultLayer::new(LayerId::new(0), "main");
        let first = layer.add_tiled();
        let second = layer.split_tiled(first, SplitDirection::Vertical).unwrap();

        // Focus moves to new window
        assert_eq!(layer.focused(), Some(second));
        assert_eq!(layer.windows_in_zone(Zone::Tiled).len(), 2);
    }

    #[test]
    fn test_default_layer_close_tiled() {
        let mut layer = DefaultLayer::new(LayerId::new(0), "main");
        layer.set_screen(Rect::new(0, 0, 80, 24));
        let first = layer.add_tiled();
        let second = layer.split_tiled(first, SplitDirection::Vertical).unwrap();

        // Close second, focus should move to first
        let neighbor = layer.close_tiled(second);
        assert_eq!(neighbor, Some(first));
        assert_eq!(layer.focused(), Some(first));
    }

    #[test]
    fn test_default_layer_navigate_tiled() {
        let mut layer = DefaultLayer::new(LayerId::new(0), "main");
        layer.set_screen(Rect::new(0, 0, 80, 24));
        let first = layer.add_tiled();
        let second = layer.split_tiled(first, SplitDirection::Vertical).unwrap();

        // Navigate right from first should find second
        let next = layer.navigate_tiled(first, NavigateDirection::Right);
        assert_eq!(next, Some(second));
    }

    #[test]
    fn test_default_layer_cycle_tiled() {
        let mut layer = DefaultLayer::new(LayerId::new(0), "main");
        layer.set_screen(Rect::new(0, 0, 80, 24));
        let first = layer.add_tiled();
        let second = layer.split_tiled(first, SplitDirection::Vertical).unwrap();

        // Cycle forward from first should find second
        let next = layer.cycle_tiled(first, true);
        assert_eq!(next, Some(second));

        // Cycle forward from second should wrap to first
        let next = layer.cycle_tiled(second, true);
        assert_eq!(next, Some(first));
    }

    #[test]
    fn test_default_layer_arrange() {
        let mut layer = DefaultLayer::new(LayerId::new(0), "main");
        let id = layer.add_tiled();

        let placements = layer.arrange(Rect::new(0, 0, 80, 24));

        assert_eq!(placements.len(), 1);
        assert_eq!(placements[0].window_id, id);
        assert_eq!(placements[0].layer_id, LayerId::new(0));
        assert_eq!(placements[0].zone, Zone::Tiled);
    }

    #[test]
    fn test_default_layer_zone_of() {
        let mut layer = DefaultLayer::new(LayerId::new(0), "main");
        let id = layer.add_tiled();

        assert_eq!(layer.zone_of(id), Some(Zone::Tiled));
        assert_eq!(layer.zone_of(WindowId::from_raw(999)), None);
    }

    #[test]
    fn test_default_layer_equalize() {
        let mut layer = DefaultLayer::new(LayerId::new(0), "main");
        layer.set_screen(Rect::new(0, 0, 80, 24));
        let first = layer.add_tiled();
        let _second = layer.split_tiled(first, SplitDirection::Vertical).unwrap();

        // Resize to unequal
        layer.resize_tiled(first, NavigateDirection::Right, 5);

        // Equalize
        layer.equalize_tiled();

        let placements = layer.arrange(Rect::new(0, 0, 80, 24));
        assert_eq!(placements[0].bounds.width, 40);
        assert_eq!(placements[1].bounds.width, 40);
    }

    #[test]
    fn test_default_layer_set_screen() {
        let mut layer = DefaultLayer::new(LayerId::new(0), "main");
        layer.set_screen(Rect::new(0, 0, 120, 40));

        // Verify screen is cached by checking arrange uses it
        let id = layer.add_tiled();
        let placements = layer.arrange(Rect::new(0, 0, 120, 40));
        assert_eq!(placements[0].window_id, id);
        assert_eq!(placements[0].bounds.width, 120);
        assert_eq!(placements[0].bounds.height, 40);
    }

    // =========================================================================
    // Float Zone Tests (#398 Phase 2)
    // =========================================================================

    #[test]
    fn test_default_layer_create_float() {
        let mut layer = DefaultLayer::new(LayerId::new(0), "main");
        let bounds = Rect::new(10, 5, 60, 20);
        let id = layer.create_float(bounds);

        assert!(layer.float().contains(id));
        assert_eq!(layer.focused(), Some(id));
        assert_eq!(layer.windows_in_zone(Zone::Float).len(), 1);
    }

    #[test]
    fn test_default_layer_close_float() {
        let mut layer = DefaultLayer::new(LayerId::new(0), "main");
        let id = layer.create_float(Rect::new(10, 5, 60, 20));

        assert!(layer.float().contains(id));

        layer.close_float(id);

        assert!(!layer.float().contains(id));
        assert_eq!(layer.windows_in_zone(Zone::Float).len(), 0);
    }

    #[test]
    fn test_default_layer_arrange_includes_float() {
        let mut layer = DefaultLayer::new(LayerId::new(0), "main");
        let tiled_id = layer.add_tiled();
        let float_id = layer.create_float(Rect::new(10, 5, 60, 20));

        let placements = layer.arrange(Rect::new(0, 0, 80, 24));

        assert_eq!(placements.len(), 2);
        // Both windows should be present
        let window_ids: Vec<_> = placements.iter().map(|p| p.window_id).collect();
        assert!(window_ids.contains(&tiled_id));
        assert!(window_ids.contains(&float_id));
    }

    #[test]
    fn test_default_layer_zone_of_float() {
        let mut layer = DefaultLayer::new(LayerId::new(0), "main");
        let float_id = layer.create_float(Rect::new(10, 5, 60, 20));

        assert_eq!(layer.zone_of(float_id), Some(Zone::Float));
    }

    #[test]
    fn test_default_layer_windows_in_zone_float() {
        let mut layer = DefaultLayer::new(LayerId::new(0), "main");
        let id1 = layer.create_float(Rect::new(10, 5, 60, 20));
        let id2 = layer.create_float(Rect::new(20, 10, 50, 15));

        let float_windows = layer.windows_in_zone(Zone::Float);
        assert_eq!(float_windows.len(), 2);
        assert!(float_windows.contains(&id1));
        assert!(float_windows.contains(&id2));
    }

    #[test]
    fn test_float_z_order_above_tiled() {
        let mut layer = DefaultLayer::new(LayerId::new(0), "main");
        let _tiled_id = layer.add_tiled();
        let _float_id = layer.create_float(Rect::new(10, 5, 60, 20));

        let placements = layer.arrange(Rect::new(0, 0, 80, 24));

        // Placements are sorted by z-order
        // Tiled zone z = 0-9, Float zone z = 10-49
        let tiled_placement = placements.iter().find(|p| p.zone == Zone::Tiled).unwrap();
        let float_placement = placements.iter().find(|p| p.zone == Zone::Float).unwrap();

        assert!(
            float_placement.z_order > tiled_placement.z_order,
            "Float z-order {} should be greater than tiled z-order {}",
            float_placement.z_order.as_u16(),
            tiled_placement.z_order.as_u16()
        );
    }

    #[test]
    fn test_toggle_tiled_to_float() {
        let mut layer = DefaultLayer::new(LayerId::new(0), "main");
        layer.set_screen(Rect::new(0, 0, 80, 24));
        let tiled_id = layer.add_tiled();

        // Toggle to float
        layer.toggle_float(tiled_id);

        // Tiled should now be empty, float should have a window
        assert_eq!(layer.windows_in_zone(Zone::Tiled).len(), 0);
        assert_eq!(layer.windows_in_zone(Zone::Float).len(), 1);
    }

    #[test]
    fn test_toggle_float_to_tiled_empty() {
        let mut layer = DefaultLayer::new(LayerId::new(0), "main");
        layer.set_screen(Rect::new(0, 0, 80, 24));
        let float_id = layer.create_float(Rect::new(10, 5, 60, 20));

        // Toggle to tiled (tiled zone is empty)
        layer.toggle_float(float_id);

        // Float should be empty, tiled should have a window
        assert_eq!(layer.windows_in_zone(Zone::Float).len(), 0);
        assert_eq!(layer.windows_in_zone(Zone::Tiled).len(), 1);
    }

    #[test]
    fn test_toggle_float_to_tiled_split() {
        let mut layer = DefaultLayer::new(LayerId::new(0), "main");
        layer.set_screen(Rect::new(0, 0, 80, 24));

        // Create a tiled window first
        let tiled_id = layer.add_tiled();
        layer.set_focus(tiled_id);

        // Create and toggle a float
        let float_id = layer.create_float(Rect::new(10, 5, 60, 20));
        layer.toggle_float(float_id);

        // Float should be empty, tiled should have 2 windows (original + split)
        assert_eq!(layer.windows_in_zone(Zone::Float).len(), 0);
        assert_eq!(layer.windows_in_zone(Zone::Tiled).len(), 2);
    }

    #[test]
    fn test_toggle_nonexistent() {
        let mut layer = DefaultLayer::new(LayerId::new(0), "main");
        layer.set_screen(Rect::new(0, 0, 80, 24));
        let tiled_id = layer.add_tiled();

        // Toggle a non-existent window (should be no-op)
        layer.toggle_float(WindowId::from_raw(9999));

        // Original state should be unchanged
        assert_eq!(layer.windows_in_zone(Zone::Tiled).len(), 1);
        assert!(layer.windows_in_zone(Zone::Tiled).contains(&tiled_id));
    }

    #[test]
    fn test_toggle_updates_focus() {
        let mut layer = DefaultLayer::new(LayerId::new(0), "main");
        layer.set_screen(Rect::new(0, 0, 80, 24));
        let tiled_id = layer.add_tiled();
        layer.set_focus(tiled_id);

        // Toggle to float
        layer.toggle_float(tiled_id);

        // Focus should be on the new float window
        let focused = layer.focused().unwrap();
        assert_eq!(layer.zone_of(focused), Some(Zone::Float));
    }

    #[test]
    fn test_toggle_float_bounds() {
        let mut layer = DefaultLayer::new(LayerId::new(0), "main");
        layer.set_screen(Rect::new(0, 0, 100, 50));
        let tiled_id = layer.add_tiled();

        // Toggle to float
        layer.toggle_float(tiled_id);

        // Check bounds are 80% centered
        let float_windows = layer.windows_in_zone(Zone::Float);
        let float_id = float_windows[0];
        let bounds = layer.float().bounds(float_id).unwrap();

        // 80% of 100 = 80, 80% of 50 = 40
        assert_eq!(bounds.width, 80);
        assert_eq!(bounds.height, 40);
        // Centered: x = (100-80)/2 = 10, y = (50-40)/2 = 5
        assert_eq!(bounds.x, 10);
        assert_eq!(bounds.y, 5);
    }

    // =========================================================================
    // Phase 5: Navigation Edge Cases (#398)
    // =========================================================================

    #[test]
    fn test_navigate_includes_float_windows() {
        let mut layer = DefaultLayer::new(LayerId::new(0), "main");
        layer.set_screen(Rect::new(0, 0, 100, 50));

        // Create a tiled window on the left (0-50 width)
        let tiled_id = layer.add_tiled();

        // Create a float window on the right (position 60, width 30)
        let float_id = layer.create_float(Rect::new(60, 10, 30, 30));

        // Navigate right from tiled should find float
        let placements = layer.arrange(layer.screen);
        assert!(placements.len() >= 2, "Should have both tiled and float windows");

        // Both windows should be in placements
        let window_ids: Vec<_> = placements.iter().map(|p| p.window_id).collect();
        assert!(window_ids.contains(&tiled_id), "Should contain tiled window");
        assert!(window_ids.contains(&float_id), "Should contain float window");
    }

    #[test]
    fn test_cycle_tiled_stays_in_tiled_zone() {
        let mut layer = DefaultLayer::new(LayerId::new(0), "main");
        layer.set_screen(Rect::new(0, 0, 100, 50));

        // Create two tiled windows
        let tiled1 = layer.add_tiled();
        let tiled2 = layer.split_tiled(tiled1, SplitDirection::Vertical).unwrap();

        // Create float window (should not participate in tiled cycling)
        let _float_id = layer.create_float(Rect::new(60, 10, 30, 30));

        // Cycle from tiled1 should reach tiled2 (not float)
        let next = layer.cycle_tiled(tiled1, true);
        assert_eq!(next, Some(tiled2), "Cycling should stay within tiled zone");

        // Cycle from tiled2 should wrap back to tiled1 (not float)
        let next = layer.cycle_tiled(tiled2, true);
        assert_eq!(next, Some(tiled1), "Cycling should wrap within tiled zone");
    }

    #[test]
    fn test_close_float_focus_moves_to_tiled() {
        let mut layer = DefaultLayer::new(LayerId::new(0), "main");
        layer.set_screen(Rect::new(0, 0, 100, 50));

        // Create tiled window
        let tiled_id = layer.add_tiled();

        // Create and focus float window
        let float_id = layer.create_float(Rect::new(60, 10, 30, 30));
        assert_eq!(layer.focused(), Some(float_id));

        // Close float window
        layer.close_float(float_id);

        // Focus should move to tiled window
        assert_eq!(layer.focused(), Some(tiled_id));
    }

    #[test]
    fn test_raise_and_lower_float() {
        let mut layer = DefaultLayer::new(LayerId::new(0), "main");
        layer.set_screen(Rect::new(0, 0, 100, 50));

        // Create two float windows
        let float1 = layer.create_float(Rect::new(10, 10, 30, 30));
        let float2 = layer.create_float(Rect::new(20, 20, 30, 30));

        // float2 is on top (created last)
        let placements = layer.arrange(layer.screen);
        let float_placements: Vec<_> = placements
            .iter()
            .filter(|p| p.zone == Zone::Float)
            .collect();

        // Sort by z-order to check stack order
        let top_float = float_placements
            .iter()
            .max_by_key(|p| p.z_order)
            .unwrap()
            .window_id;
        assert_eq!(top_float, float2, "float2 should be on top initially");

        // Raise float1 to top
        layer.raise_float(float1);
        let placements = layer.arrange(layer.screen);
        let float_placements: Vec<_> = placements
            .iter()
            .filter(|p| p.zone == Zone::Float)
            .collect();
        let top_float = float_placements
            .iter()
            .max_by_key(|p| p.z_order)
            .unwrap()
            .window_id;
        assert_eq!(top_float, float1, "float1 should be on top after raise");

        // Lower float1 to bottom
        layer.lower_float(float1);
        let placements = layer.arrange(layer.screen);
        let float_placements: Vec<_> = placements
            .iter()
            .filter(|p| p.zone == Zone::Float)
            .collect();
        let top_float = float_placements
            .iter()
            .max_by_key(|p| p.z_order)
            .unwrap()
            .window_id;
        assert_eq!(top_float, float2, "float2 should be on top after lowering float1");
    }
}
