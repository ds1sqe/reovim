//! Hybrid compositor implementation for the nested compositor architecture.
//!
//! This module provides `HybridCompositor`, which implements `RootCompositor`
//! for managing multiple layers. It follows the Hyprland-inspired model where
//! each layer is a self-contained mini-compositor.
//!
//! # Architecture
//!
//! ```text
//! HybridCompositor (implements RootCompositor)
//! ├── Layer 0 ("main", z=0)     ← DefaultLayer
//! │   └── WindowLayerCompositor
//! │       ├── Tiled Zone
//! │       ├── Float Zone (Phase 2)
//! │       └── Overlay Zone (Phase 3)
//! │
//! └── Layer 1 ("popup", z=100)  ← DefaultLayer
//!     └── WindowLayerCompositor
//!         └── ...
//! ```
//!
//! # Focus Model
//!
//! The compositor tracks:
//! - **Active layer**: Which layer receives keyboard input
//! - **Focus per layer**: Each layer tracks its own focused window
//!
//! When focusing a window, its layer automatically becomes active.

use std::collections::HashMap;

use reovim_driver_display::{
    Rect, WindowId,
    layout::{CompositeResult, Layer, LayerConfig, LayerId, RootCompositor, WindowLayerCompositor},
};

use crate::layer::DefaultLayer;

/// Hybrid compositor that manages multiple layers.
///
/// This is the top-level compositor implementing `RootCompositor`.
/// Each layer is a `DefaultLayer` implementing `WindowLayerCompositor`.
///
/// # Phase 1 Simplification
///
/// In Phase 1, we use a single "main" layer. Multi-layer support is
/// available but primarily used for future phases (popup layers, etc.).
///
/// # Thread Safety
///
/// `HybridCompositor` is `Send + Sync` because all internal state uses
/// thread-safe types (`HashMap`, `Vec` of `DefaultLayer`).
#[derive(Debug, Clone)]
pub struct HybridCompositor {
    /// Layers indexed by ID for O(1) lookup.
    layers: HashMap<LayerId, DefaultLayer>,
    /// Layers in z-order (lowest first).
    z_order: Vec<LayerId>,
    /// Currently active layer (receives keyboard input).
    active_layer: Option<LayerId>,
    /// Next layer ID to allocate.
    next_layer_id: u16,
    /// Cached screen size for arrange calculations.
    screen: Rect,
}

impl Default for HybridCompositor {
    fn default() -> Self {
        Self::new()
    }
}

impl HybridCompositor {
    /// Create a new compositor.
    ///
    /// Starts with no layers. Use `create_layer` to add the initial layer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            layers: HashMap::new(),
            z_order: Vec::new(),
            active_layer: None,
            next_layer_id: 0,
            screen: Rect::new(0, 0, 80, 24),
        }
    }

    /// Create a compositor with a default "main" layer.
    ///
    /// This is the typical setup for Phase 1 with a single tiled layer.
    #[must_use]
    pub fn with_main_layer() -> Self {
        let mut compositor = Self::new();
        let layer_id = compositor.create_layer(LayerConfig::fullscreen("main"));
        compositor.set_active_layer(layer_id);
        compositor
    }

    /// Update the screen size for all layers.
    ///
    /// Called on terminal resize.
    pub fn set_screen(&mut self, screen: Rect) {
        self.screen = screen;
        for layer in self.layers.values_mut() {
            layer.set_screen(screen);
        }
    }

    /// Get layer by ID (for direct access).
    #[must_use]
    pub fn get_layer(&self, id: LayerId) -> Option<&DefaultLayer> {
        self.layers.get(&id)
    }

    /// Get mutable layer by ID.
    #[must_use]
    pub fn get_layer_mut(&mut self, id: LayerId) -> Option<&mut DefaultLayer> {
        self.layers.get_mut(&id)
    }

    /// Allocate the next layer ID.
    fn allocate_layer_id(&mut self) -> LayerId {
        let id = LayerId::new(self.next_layer_id);
        self.next_layer_id += 1;
        id
    }
}

impl RootCompositor for HybridCompositor {
    fn composite(&self, screen: Rect) -> CompositeResult {
        let mut placements = Vec::new();

        // Collect placements from all visible layers in z-order
        for &layer_id in &self.z_order {
            if let Some(layer) = self.layers.get(&layer_id) {
                let layer_placements = layer.arrange(screen);
                placements.extend(layer_placements);
            }
        }

        // Sort by z-order (lowest first)
        placements.sort_by_key(|p| p.z_order);

        let focused = self
            .active_layer
            .and_then(|id| self.layers.get(&id))
            .and_then(|layer| layer.focused());

        CompositeResult {
            placements,
            focused,
            active_layer: self.active_layer,
            screen,
        }
    }

    // =========================================================================
    // Layer Management
    // =========================================================================

    fn create_layer(&mut self, config: LayerConfig) -> LayerId {
        let id = self.allocate_layer_id();

        // Note: DefaultLayer handles z_base internally based on layer_id
        // (Layer 0 = 0-99, Layer 1 = 100-199, etc.)
        let mut layer = DefaultLayer::new(id, &config.label);
        layer.set_screen(self.screen);

        self.layers.insert(id, layer);
        self.z_order.push(id);

        // If this is the first layer, make it active
        if self.active_layer.is_none() {
            self.active_layer = Some(id);
        }

        id
    }

    fn remove_layer(&mut self, layer: LayerId) {
        self.layers.remove(&layer);
        self.z_order.retain(|&id| id != layer);

        // If we removed the active layer, switch to first available
        if self.active_layer == Some(layer) {
            self.active_layer = self.z_order.first().copied();
        }
    }

    fn layer_by_label(&self, label: &str) -> Option<LayerId> {
        for (&id, layer) in &self.layers {
            if layer.label() == label {
                return Some(id);
            }
        }
        None
    }

    fn layers(&self) -> Vec<&Layer> {
        // Return Layer references in z-order
        // Note: DefaultLayer doesn't directly expose Layer, so we create them
        // For Phase 1, we return empty since we'd need to refactor DefaultLayer
        // to hold/expose Layer. This is acceptable for Phase 1.
        Vec::new()
    }

    fn set_layer_visible(&mut self, layer: LayerId, visible: bool) {
        // Phase 1: Layers are always visible
        // Future: Add visibility field to DefaultLayer
        let _ = (layer, visible);
    }

    fn set_layer_opacity(&mut self, layer: LayerId, opacity: f32) {
        // Phase 1: Layers are always opaque
        // Future: Add opacity field to DefaultLayer
        let _ = (layer, opacity);
    }

    fn reorder_layer(&mut self, layer: LayerId, new_z: u16) {
        // Remove from current position
        self.z_order.retain(|&id| id != layer);

        // Find insert position based on new z-order
        let insert_pos = self
            .z_order
            .iter()
            .position(|&id| {
                self.layers
                    .get(&id)
                    .is_some_and(|_| id.as_u16() * 100 > new_z)
            })
            .unwrap_or(self.z_order.len());

        self.z_order.insert(insert_pos, layer);
    }

    // =========================================================================
    // Focus Management
    // =========================================================================

    fn set_active_layer(&mut self, layer: LayerId) {
        if self.layers.contains_key(&layer) {
            self.active_layer = Some(layer);
        }
    }

    fn active_layer(&self) -> Option<LayerId> {
        self.active_layer
    }

    fn set_focus(&mut self, window: WindowId) {
        // Find which layer contains the window
        if let Some(layer_id) = self.layer_of(window) {
            // Activate that layer
            self.active_layer = Some(layer_id);

            // Set focus within the layer
            if let Some(layer) = self.layers.get_mut(&layer_id) {
                layer.set_focus(window);
            }
        }
    }

    fn focused(&self) -> Option<WindowId> {
        self.active_layer
            .and_then(|id| self.layers.get(&id))
            .and_then(|layer| layer.focused())
    }

    fn focus_at(&mut self, x: u16, y: u16) -> Option<WindowId> {
        // Check layers in reverse z-order (top to bottom)
        let result = self.composite(self.screen);

        // Find topmost window containing the point
        let target = result
            .placements
            .iter()
            .rev()
            .find(|p| {
                p.focusable
                    && p.visible
                    && p.bounds.x <= x
                    && x < p.bounds.x + p.bounds.width
                    && p.bounds.y <= y
                    && y < p.bounds.y + p.bounds.height
            })
            .map(|p| p.window_id);

        if let Some(window) = target {
            self.set_focus(window);
        }

        target
    }

    // =========================================================================
    // Per-Layer Operations
    // =========================================================================

    fn layer_compositor(&self, layer: LayerId) -> Option<&dyn WindowLayerCompositor> {
        self.layers
            .get(&layer)
            .map(|l| l as &dyn WindowLayerCompositor)
    }

    fn layer_compositor_mut(&mut self, layer: LayerId) -> Option<&mut dyn WindowLayerCompositor> {
        self.layers
            .get_mut(&layer)
            .map(|l| l as &mut dyn WindowLayerCompositor)
    }

    fn window_count(&self) -> usize {
        self.layers
            .values()
            .map(|layer| {
                use reovim_driver_display::layout::Zone;
                layer.windows_in_zone(Zone::Tiled).len()
                    + layer.windows_in_zone(Zone::Float).len()
                    + layer.windows_in_zone(Zone::Overlay).len()
            })
            .sum()
    }

    fn set_screen(&mut self, screen: Rect) {
        self.screen = screen;
        // Propagate to all layers
        for layer in self.layers.values_mut() {
            layer.set_screen(screen);
        }
    }

    fn layer_of(&self, window: WindowId) -> Option<LayerId> {
        for (&layer_id, layer) in &self.layers {
            if layer.zone_of(window).is_some() {
                return Some(layer_id);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_driver_display::{SplitDirection, layout::Zone},
    };

    #[test]
    fn test_compositor_new() {
        let compositor = HybridCompositor::new();
        assert!(compositor.active_layer().is_none());
        assert_eq!(compositor.window_count(), 0);
    }

    #[test]
    fn test_compositor_with_main_layer() {
        let compositor = HybridCompositor::with_main_layer();
        assert!(compositor.active_layer().is_some());

        let layer_id = compositor.layer_by_label("main");
        assert!(layer_id.is_some());
    }

    #[test]
    fn test_create_layer() {
        let mut compositor = HybridCompositor::new();
        let id = compositor.create_layer(LayerConfig::fullscreen("test"));

        assert_eq!(compositor.active_layer(), Some(id));
        assert!(compositor.get_layer(id).is_some());
    }

    #[test]
    fn test_create_multiple_layers() {
        let mut compositor = HybridCompositor::new();
        let id1 = compositor.create_layer(LayerConfig::fullscreen("layer1"));
        let id2 = compositor.create_layer(LayerConfig::fullscreen("layer2"));

        // First layer should be active
        assert_eq!(compositor.active_layer(), Some(id1));

        // Both layers should exist
        assert!(compositor.get_layer(id1).is_some());
        assert!(compositor.get_layer(id2).is_some());
    }

    #[test]
    fn test_remove_layer() {
        let mut compositor = HybridCompositor::new();
        let id1 = compositor.create_layer(LayerConfig::fullscreen("layer1"));
        let id2 = compositor.create_layer(LayerConfig::fullscreen("layer2"));

        compositor.remove_layer(id1);

        assert!(compositor.get_layer(id1).is_none());
        assert!(compositor.get_layer(id2).is_some());
        // Active should switch to remaining layer
        assert_eq!(compositor.active_layer(), Some(id2));
    }

    #[test]
    fn test_layer_by_label() {
        let mut compositor = HybridCompositor::new();
        compositor.create_layer(LayerConfig::fullscreen("main"));
        compositor.create_layer(LayerConfig::fullscreen("popup"));

        assert!(compositor.layer_by_label("main").is_some());
        assert!(compositor.layer_by_label("popup").is_some());
        assert!(compositor.layer_by_label("nonexistent").is_none());
    }

    #[test]
    fn test_add_window_to_layer() {
        let mut compositor = HybridCompositor::with_main_layer();
        let layer_id = compositor.active_layer().unwrap();

        let layer = compositor.get_layer_mut(layer_id).unwrap();
        let window_id = layer.add_tiled();

        assert_eq!(compositor.window_count(), 1);
        assert_eq!(compositor.layer_of(window_id), Some(layer_id));
    }

    #[test]
    fn test_composite_empty() {
        let compositor = HybridCompositor::new();
        let result = compositor.composite(Rect::new(0, 0, 80, 24));

        assert!(result.placements.is_empty());
        assert!(result.focused.is_none());
        assert!(result.active_layer.is_none());
    }

    #[test]
    fn test_composite_with_windows() {
        let mut compositor = HybridCompositor::with_main_layer();
        let layer_id = compositor.active_layer().unwrap();

        let layer = compositor.get_layer_mut(layer_id).unwrap();
        let w1 = layer.add_tiled();
        let _w2 = layer.split_tiled(w1, SplitDirection::Vertical);

        let result = compositor.composite(Rect::new(0, 0, 80, 24));

        assert_eq!(result.placements.len(), 2);
        assert!(result.focused.is_some());
        assert_eq!(result.active_layer, Some(layer_id));
    }

    #[test]
    fn test_composite_z_order() {
        let mut compositor = HybridCompositor::with_main_layer();
        let layer_id = compositor.active_layer().unwrap();

        let layer = compositor.get_layer_mut(layer_id).unwrap();
        let w1 = layer.add_tiled();
        let _w2 = layer.split_tiled(w1, SplitDirection::Vertical);

        let result = compositor.composite(Rect::new(0, 0, 80, 24));

        // Verify placements are sorted by z-order
        let z_orders: Vec<_> = result.placements.iter().map(|p| p.z_order).collect();
        let mut sorted = z_orders.clone();
        sorted.sort();
        assert_eq!(z_orders, sorted);
    }

    #[test]
    fn test_set_focus() {
        let mut compositor = HybridCompositor::with_main_layer();
        let layer_id = compositor.active_layer().unwrap();

        let layer = compositor.get_layer_mut(layer_id).unwrap();
        let w1 = layer.add_tiled();
        let w2 = layer.split_tiled(w1, SplitDirection::Vertical).unwrap();

        // Focus is on w2 after split
        assert_eq!(compositor.focused(), Some(w2));

        // Switch focus to w1
        compositor.set_focus(w1);
        assert_eq!(compositor.focused(), Some(w1));
    }

    #[test]
    fn test_set_focus_activates_layer() {
        let mut compositor = HybridCompositor::new();
        let id1 = compositor.create_layer(LayerConfig::fullscreen("layer1"));
        let id2 = compositor.create_layer(LayerConfig::fullscreen("layer2"));

        // Add window to layer2
        let layer2 = compositor.get_layer_mut(id2).unwrap();
        let window = layer2.add_tiled();

        // Layer1 is active (first created)
        assert_eq!(compositor.active_layer(), Some(id1));

        // Focus window in layer2
        compositor.set_focus(window);

        // Layer2 should now be active
        assert_eq!(compositor.active_layer(), Some(id2));
    }

    #[test]
    fn test_focus_at() {
        let mut compositor = HybridCompositor::with_main_layer();
        compositor.set_screen(Rect::new(0, 0, 80, 24));

        let layer_id = compositor.active_layer().unwrap();
        let layer = compositor.get_layer_mut(layer_id).unwrap();
        let w1 = layer.add_tiled();
        let _w2 = layer.split_tiled(w1, SplitDirection::Vertical);

        // Focus at left side (should be w1)
        let focused = compositor.focus_at(10, 10);
        assert_eq!(focused, Some(w1));
    }

    #[test]
    fn test_focus_at_right_window() {
        let mut compositor = HybridCompositor::with_main_layer();
        compositor.set_screen(Rect::new(0, 0, 80, 24));

        let layer_id = compositor.active_layer().unwrap();
        let layer = compositor.get_layer_mut(layer_id).unwrap();
        let w1 = layer.add_tiled();
        let w2 = layer.split_tiled(w1, SplitDirection::Vertical).unwrap();

        // Focus at right side (should be w2)
        let focused = compositor.focus_at(60, 10);
        assert_eq!(focused, Some(w2));
    }

    #[test]
    fn test_layer_compositor() {
        let mut compositor = HybridCompositor::with_main_layer();
        let layer_id = compositor.active_layer().unwrap();

        // Get immutable compositor
        let layer = compositor.layer_compositor(layer_id);
        assert!(layer.is_some());
        assert_eq!(layer.unwrap().id(), layer_id);

        // Get mutable compositor
        let layer = compositor.layer_compositor_mut(layer_id);
        assert!(layer.is_some());

        // Add window through trait object
        let window = layer.unwrap().add_tiled();
        assert_eq!(compositor.window_count(), 1);
        assert_eq!(compositor.layer_of(window), Some(layer_id));
    }

    #[test]
    fn test_set_active_layer() {
        let mut compositor = HybridCompositor::new();
        let id1 = compositor.create_layer(LayerConfig::fullscreen("layer1"));
        let id2 = compositor.create_layer(LayerConfig::fullscreen("layer2"));

        assert_eq!(compositor.active_layer(), Some(id1));

        compositor.set_active_layer(id2);
        assert_eq!(compositor.active_layer(), Some(id2));
    }

    #[test]
    fn test_window_count_multiple_layers() {
        let mut compositor = HybridCompositor::new();
        let id1 = compositor.create_layer(LayerConfig::fullscreen("layer1"));
        let id2 = compositor.create_layer(LayerConfig::fullscreen("layer2"));

        // Add windows to both layers
        let layer1 = compositor.get_layer_mut(id1).unwrap();
        let w1 = layer1.add_tiled();
        let _ = layer1.split_tiled(w1, SplitDirection::Vertical);

        let layer2 = compositor.get_layer_mut(id2).unwrap();
        let _ = layer2.add_tiled();

        assert_eq!(compositor.window_count(), 3);
    }

    #[test]
    fn test_layer_of_returns_correct_layer() {
        // Test layer_of with single layer (avoids duplicate WindowId issue)
        // Note: In Phase 1, each layer has its own WindowId counter.
        // In the real system, the kernel allocates globally unique IDs.
        let mut compositor = HybridCompositor::with_main_layer();
        let layer_id = compositor.active_layer().unwrap();

        let layer = compositor.get_layer_mut(layer_id).unwrap();
        let w1 = layer.add_tiled();
        let w2 = layer.split_tiled(w1, SplitDirection::Vertical).unwrap();

        assert_eq!(compositor.layer_of(w1), Some(layer_id));
        assert_eq!(compositor.layer_of(w2), Some(layer_id));
        assert_eq!(compositor.layer_of(WindowId::from_raw(999)), None);
    }

    #[test]
    fn test_set_screen_propagates() {
        let mut compositor = HybridCompositor::with_main_layer();
        let new_screen = Rect::new(0, 0, 120, 40);

        compositor.set_screen(new_screen);

        let result = compositor.composite(new_screen);
        assert_eq!(result.screen, new_screen);
    }

    #[test]
    fn test_remove_active_layer_switches_focus() {
        let mut compositor = HybridCompositor::new();
        let id1 = compositor.create_layer(LayerConfig::fullscreen("layer1"));
        let id2 = compositor.create_layer(LayerConfig::fullscreen("layer2"));

        // Make id2 active
        compositor.set_active_layer(id2);
        assert_eq!(compositor.active_layer(), Some(id2));

        // Remove id2
        compositor.remove_layer(id2);

        // Should switch to id1
        assert_eq!(compositor.active_layer(), Some(id1));
    }

    #[test]
    fn test_windows_in_zone() {
        let mut compositor = HybridCompositor::with_main_layer();
        let layer_id = compositor.active_layer().unwrap();

        let layer = compositor.layer_compositor(layer_id).unwrap();
        assert!(layer.windows_in_zone(Zone::Tiled).is_empty());

        let layer = compositor.layer_compositor_mut(layer_id).unwrap();
        let w1 = layer.add_tiled();
        let _w2 = layer.split_tiled(w1, SplitDirection::Vertical);

        let layer = compositor.layer_compositor(layer_id).unwrap();
        assert_eq!(layer.windows_in_zone(Zone::Tiled).len(), 2);
    }
}
