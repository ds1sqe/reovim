//! Root compositor implementation.
//!
//! Manages multiple layers, each with its own `HybridLayerCompositor`.
//! Implements `RootCompositor` from `reovim-driver-layout`.

use reovim_driver_layout::{
    CompositeResult, Layer, LayerConfig, LayerId, Rect, RootCompositor, WindowId,
    WindowLayerCompositor, WindowPlacement, ZOrder,
};

use crate::hybrid_layer::HybridLayerCompositor;

/// Root compositor managing multiple layers.
///
/// On creation, a "main" layer is created at `LayerId(0)`.
/// Each client gets an independent clone via `boxed_clone()`.
#[derive(Debug, Clone)]
pub struct HybridCompositor {
    layers: Vec<(Layer, HybridLayerCompositor)>,
    active_layer: Option<LayerId>,
    focused: Option<WindowId>,
    next_layer_id: u16,
    screen: Rect,
}

impl HybridCompositor {
    /// Create a new compositor with a "main" layer.
    #[must_use]
    pub fn new() -> Self {
        let layer_id = LayerId::new(0);
        let layer = Layer::new(layer_id, "main", ZOrder::layer_base(layer_id));
        let compositor = HybridLayerCompositor::new(layer_id);

        Self {
            layers: vec![(layer, compositor)],
            active_layer: Some(layer_id),
            focused: None,
            next_layer_id: 1,
            screen: Rect::default(),
        }
    }

    /// Find the layer index containing a window.
    fn layer_index_of(&self, window: WindowId) -> Option<usize> {
        self.layers
            .iter()
            .position(|(_, comp)| comp.windows_in_zone(reovim_driver_layout::Zone::Tiled).contains(&window))
    }

    /// Get mutable reference to a layer pair by ID.
    fn get_layer_mut(&mut self, id: LayerId) -> Option<&mut (Layer, HybridLayerCompositor)> {
        self.layers.iter_mut().find(|(l, _)| l.id == id)
    }

    /// Get reference to a layer pair by ID.
    fn get_layer(&self, id: LayerId) -> Option<&(Layer, HybridLayerCompositor)> {
        self.layers.iter().find(|(l, _)| l.id == id)
    }
}

impl Default for HybridCompositor {
    fn default() -> Self {
        Self::new()
    }
}

impl RootCompositor for HybridCompositor {
    fn composite(&self, screen: Rect) -> CompositeResult {
        let mut all_placements: Vec<WindowPlacement> = Vec::new();

        for (layer, comp) in &self.layers {
            if !layer.visible {
                continue;
            }
            let bounds = if layer.bounds == Rect::default() {
                screen
            } else {
                layer.bounds
            };
            let mut placements = comp.arrange(bounds);
            // Apply layer opacity to all placements
            for p in &mut placements {
                p.opacity = layer.opacity;
            }
            all_placements.extend(placements);
        }

        // Sort by z-order (lowest first)
        all_placements.sort_by_key(|p| p.z_order);

        CompositeResult {
            placements: all_placements,
            focused: self.focused,
            active_layer: self.active_layer,
            screen,
        }
    }

    // =========================================================================
    // Layer Management
    // =========================================================================

    fn create_layer(&mut self, config: LayerConfig) -> LayerId {
        let id = LayerId::new(self.next_layer_id);
        self.next_layer_id += 1;

        let mut layer = Layer::new(id, config.label, ZOrder::layer_base(id));
        layer.opacity = config.opacity;
        if let Some(bounds) = config.bounds {
            layer.bounds = bounds;
        }

        let comp = HybridLayerCompositor::new(id);
        self.layers.push((layer, comp));
        id
    }

    fn remove_layer(&mut self, layer: LayerId) {
        self.layers.retain(|(l, _)| l.id != layer);
        if self.active_layer == Some(layer) {
            self.active_layer = self.layers.first().map(|(l, _)| l.id);
        }
    }

    fn layer_by_label(&self, label: &str) -> Option<LayerId> {
        self.layers
            .iter()
            .find(|(l, _)| l.label == label)
            .map(|(l, _)| l.id)
    }

    fn layers(&self) -> Vec<&Layer> {
        self.layers.iter().map(|(l, _)| l).collect()
    }

    fn set_layer_visible(&mut self, layer: LayerId, visible: bool) {
        if let Some((l, _)) = self.get_layer_mut(layer) {
            l.visible = visible;
        }
    }

    fn set_layer_opacity(&mut self, layer: LayerId, opacity: f32) {
        if let Some((l, _)) = self.get_layer_mut(layer) {
            l.opacity = opacity;
        }
    }

    fn reorder_layer(&mut self, layer: LayerId, new_z: u16) {
        if let Some((l, _)) = self.get_layer_mut(layer) {
            l.z_base = ZOrder::new(new_z);
        }
    }

    // =========================================================================
    // Focus Management
    // =========================================================================

    fn set_active_layer(&mut self, layer: LayerId) {
        self.active_layer = Some(layer);
    }

    fn active_layer(&self) -> Option<LayerId> {
        self.active_layer
    }

    fn set_focus(&mut self, window: WindowId) {
        self.focused = Some(window);
        // Activate the layer containing this window
        if let Some(idx) = self.layer_index_of(window) {
            let layer_id = self.layers[idx].0.id;
            self.active_layer = Some(layer_id);
            self.layers[idx].1.set_focus(window);
        }
    }

    fn focused(&self) -> Option<WindowId> {
        self.focused
    }

    fn focus_at(&mut self, _x: u16, _y: u16) -> Option<WindowId> {
        // Click-through focus — not needed for keyboard-driven MVP
        None
    }

    // =========================================================================
    // Per-Layer Operations
    // =========================================================================

    fn layer_compositor(&self, layer: LayerId) -> Option<&dyn WindowLayerCompositor> {
        self.get_layer(layer)
            .map(|(_, comp)| comp as &dyn WindowLayerCompositor)
    }

    fn layer_compositor_mut(&mut self, layer: LayerId) -> Option<&mut dyn WindowLayerCompositor> {
        self.get_layer_mut(layer)
            .map(|(_, comp)| comp as &mut dyn WindowLayerCompositor)
    }

    fn window_count(&self) -> usize {
        self.layers
            .iter()
            .map(|(_, comp)| comp.windows_in_zone(reovim_driver_layout::Zone::Tiled).len())
            .sum()
    }

    fn set_screen(&mut self, screen: Rect) {
        self.screen = screen;
        for (layer, _) in &mut self.layers {
            if layer.bounds == Rect::default() {
                layer.bounds = screen;
            }
        }
    }

    fn layer_of(&self, window: WindowId) -> Option<LayerId> {
        self.layer_index_of(window)
            .map(|idx| self.layers[idx].0.id)
    }

    fn boxed_clone(&self) -> Box<dyn RootCompositor> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
#[path = "hybrid_compositor_tests.rs"]
mod tests;
