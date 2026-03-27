//! Per-layer compositor implementation.
//!
//! Implements `WindowLayerCompositor` by delegating tiled operations to
//! `TiledZone` and stubbing float/overlay zones for MVP.

use reovim_driver_layout::{
    LayerId, NavigateDirection, OverlayConstraints, Rect, SplitDirection, TiledLayer, WindowId,
    WindowLayerCompositor, WindowPlacement, Zone,
};

use crate::tiled_zone::TiledZone;

/// Per-layer compositor managing tiled, float, and overlay zones.
#[derive(Debug, Clone)]
pub struct HybridLayerCompositor {
    layer_id: LayerId,
    tiled: TiledZone,
    focused: Option<WindowId>,
}

impl HybridLayerCompositor {
    /// Create a new layer compositor.
    pub const fn new(layer_id: LayerId) -> Self {
        Self {
            layer_id,
            tiled: TiledZone::new(layer_id),
            focused: None,
        }
    }
}

impl WindowLayerCompositor for HybridLayerCompositor {
    fn id(&self) -> LayerId {
        self.layer_id
    }

    fn arrange(&self, bounds: Rect) -> Vec<WindowPlacement> {
        self.tiled.arrange(bounds)
    }

    // =========================================================================
    // Tiled zone — delegates to TiledZone
    // =========================================================================

    fn add_tiled(&mut self) -> WindowId {
        let id = self.tiled.add_first();
        self.focused = Some(id);
        id
    }

    fn split_tiled(&mut self, from: WindowId, direction: SplitDirection) -> Option<WindowId> {
        let new_id = self.tiled.split(from, direction)?;
        self.focused = Some(new_id);
        Some(new_id)
    }

    fn navigate_tiled(&self, from: WindowId, direction: NavigateDirection) -> Option<WindowId> {
        let bounds = Rect::new(0, 0, 1000, 1000);
        let views = self.tiled.arrange(bounds);
        self.tiled.navigate(from, direction, &views)
    }

    fn resize_tiled(&mut self, window: WindowId, direction: NavigateDirection, delta: i16) {
        self.tiled.resize(window, direction, delta);
    }

    fn close_tiled(&mut self, window: WindowId) -> Option<WindowId> {
        let focus_target = self.tiled.close(window)?;
        self.focused = Some(focus_target);
        Some(focus_target)
    }

    fn equalize_tiled(&mut self) {
        self.tiled.equalize();
    }

    fn cycle_tiled(&self, from: WindowId, forward: bool) -> Option<WindowId> {
        let bounds = Rect::new(0, 0, 1000, 1000);
        let views = self.tiled.arrange(bounds);
        self.tiled.cycle(from, forward, &views)
    }

    // =========================================================================
    // Float zone — stubs for MVP
    // =========================================================================

    fn create_float(&mut self, _bounds: Rect) -> WindowId {
        WindowId::new()
    }

    fn move_float(&mut self, _window: WindowId, _x: u16, _y: u16) {}

    fn resize_float(&mut self, _window: WindowId, _width: u16, _height: u16) {}

    fn raise_float(&mut self, _window: WindowId) {}

    fn lower_float(&mut self, _window: WindowId) {}

    fn close_float(&mut self, _window: WindowId) {}

    fn toggle_float(&mut self, _window: WindowId) {}

    // =========================================================================
    // Overlay zone — stubs for MVP
    // =========================================================================

    fn show_overlay(&mut self, _constraints: OverlayConstraints) -> WindowId {
        WindowId::new()
    }

    fn hide_overlay(&mut self, _window: WindowId) {}

    fn resize_overlay(&mut self, _window: WindowId, _width: u16, _height: u16) {}

    fn hide_all_overlays(&mut self) {}

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
            Zone::Tiled => self.tiled.windows(),
            Zone::Float | Zone::Overlay => Vec::new(),
        }
    }

    fn zone_of(&self, window: WindowId) -> Option<Zone> {
        if self.tiled.windows().contains(&window) {
            Some(Zone::Tiled)
        } else {
            None
        }
    }
}

#[cfg(test)]
#[path = "hybrid_layer_tests.rs"]
mod tests;
