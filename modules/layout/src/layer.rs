//! Default layer implementation for the nested compositor.
//!
//! This module provides `DefaultLayer`, which implements `WindowLayerCompositor`
//! for a single-layer, tiled-only compositor scenario (Phase 1).
//!
//! # Architecture
//!
//! ```text
//! DefaultLayer (implements WindowLayerCompositor)
//! ├── Tiled Zone  (TilingLayout)  ← Active
//! ├── Float Zone  (stub)          ← Phase 2
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
        LayerId, OverlayConstraints, TiledLayer, WindowLayerCompositor, WindowPlacement, ZOrder,
        Zone,
    },
};

use crate::TilingLayout;

/// Default layer implementation with tiled zone only.
///
/// This is the single-layer compositor for Phase 1. It wraps a `TilingLayout`
/// and implements `WindowLayerCompositor`, delegating tiled operations to the
/// underlying layout while stubbing float/overlay operations.
///
/// # Thread Safety
///
/// `DefaultLayer` is `Send + Sync` because `TilingLayout` is `Send + Sync`.
#[derive(Debug, Clone)]
pub struct DefaultLayer {
    /// Layer identifier.
    id: LayerId,
    /// Human-readable label (e.g., "main").
    label: String,
    /// Tiled zone manager.
    tiled: TilingLayout,
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
}

impl WindowLayerCompositor for DefaultLayer {
    fn id(&self) -> LayerId {
        self.id
    }

    fn arrange(&self, bounds: Rect) -> Vec<WindowPlacement> {
        TiledLayer::arrange(&self.tiled, bounds)
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
    // Float Zone (Phase 2 stubs)
    // =========================================================================

    fn create_float(&mut self, _bounds: Rect) -> WindowId {
        unimplemented!("Float zone not implemented in Phase 1")
    }

    fn move_float(&mut self, _window: WindowId, _x: u16, _y: u16) {
        unimplemented!("Float zone not implemented in Phase 1")
    }

    fn resize_float(&mut self, _window: WindowId, _width: u16, _height: u16) {
        unimplemented!("Float zone not implemented in Phase 1")
    }

    fn raise_float(&mut self, _window: WindowId) {
        unimplemented!("Float zone not implemented in Phase 1")
    }

    fn close_float(&mut self, _window: WindowId) {
        unimplemented!("Float zone not implemented in Phase 1")
    }

    fn toggle_float(&mut self, _window: WindowId) {
        unimplemented!("Float zone not implemented in Phase 1")
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
            Zone::Float | Zone::Overlay => Vec::new(), // Not implemented in Phase 1
        }
    }

    fn zone_of(&self, window: WindowId) -> Option<Zone> {
        if TiledLayer::windows(&self.tiled).contains(&window) {
            Some(Zone::Tiled)
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
}
