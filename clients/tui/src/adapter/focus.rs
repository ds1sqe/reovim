//! Focus management trait implementation for TUI.
//!
//! This module provides an adapter between the common client model's `FocusManager`
//! trait and TUI's `RootCompositor` focus handling.

use std::sync::{Arc, Mutex};

use {
    reovim_client_model::traits::{Focus, FocusManager},
    reovim_driver_display::{WindowId, layout::RootCompositor},
};

/// Adapter implementing the common `FocusManager` trait for TUI.
///
/// Tracks focus state and synchronizes with the underlying `RootCompositor`.
/// Supports focus transitions between panels (editor viewports) and overlays
/// (popups, menus).
///
/// # ID Conversion
///
/// The common model uses `u64` for viewport IDs while TUI uses `WindowId` (usize).
/// This adapter handles the conversion transparently.
pub struct TuiFocusManager<C: RootCompositor + 'static> {
    /// The underlying root compositor (for syncing panel focus).
    compositor: Arc<Mutex<C>>,
    /// Current focus state.
    current: Focus,
    /// Last focused panel (for `return_to_panel`).
    last_panel: u64,
}

impl<C: RootCompositor + 'static> TuiFocusManager<C> {
    /// Create a new focus manager.
    ///
    /// # Arguments
    ///
    /// * `compositor` - The root compositor to sync panel focus with
    /// * `initial_viewport` - The initially focused viewport ID
    #[must_use]
    pub const fn new(compositor: Arc<Mutex<C>>, initial_viewport: u64) -> Self {
        Self {
            compositor,
            current: Focus::Panel(initial_viewport),
            last_panel: initial_viewport,
        }
    }

    /// Get a reference to the underlying compositor.
    #[must_use]
    pub fn compositor(&self) -> Arc<Mutex<C>> {
        Arc::clone(&self.compositor)
    }

    /// Convert a `u64` to `WindowId`.
    #[must_use]
    const fn u64_to_window_id(id: u64) -> WindowId {
        #[allow(clippy::cast_possible_truncation)]
        WindowId::from_raw(id as usize)
    }

    /// Sync the focus state with the compositor.
    ///
    /// Only syncs if the current focus is on a panel (compositor doesn't
    /// know about overlay focus).
    fn sync_with_compositor(&self) {
        if let Focus::Panel(viewport_id) = &self.current {
            let window_id = Self::u64_to_window_id(*viewport_id);
            if let Ok(mut compositor) = self.compositor.lock() {
                compositor.set_focus(window_id);
            }
        }
    }
}

#[allow(clippy::significant_drop_tightening)]
impl<C: RootCompositor + 'static> FocusManager for TuiFocusManager<C> {
    fn current(&self) -> &Focus {
        &self.current
    }

    fn focus_panel(&mut self, viewport_id: u64) {
        self.last_panel = viewport_id;
        self.current = Focus::Panel(viewport_id);
        self.sync_with_compositor();
    }

    fn focus_overlay(&mut self, overlay_id: &str) {
        // Remember the last panel before switching to overlay
        if let Focus::Panel(id) = self.current {
            self.last_panel = id;
        }
        self.current = Focus::Overlay(overlay_id.to_string());
        // Note: Compositor focus stays on the panel - overlays are
        // rendered on top but the panel retains compositor focus
    }

    fn return_to_panel(&mut self) {
        self.current = Focus::Panel(self.last_panel);
        self.sync_with_compositor();
    }
}

#[cfg(test)]
#[allow(clippy::significant_drop_tightening)]
mod tests {
    use {
        super::*,
        reovim_driver_display::{
            Rect,
            layout::{
                CompositeResult, Layer, LayerConfig, LayerId, RootCompositor, WindowLayerCompositor,
            },
        },
    };

    // Mock compositor for testing
    struct MockCompositor {
        focused: Option<WindowId>,
    }

    impl MockCompositor {
        fn new(initial_focus: WindowId) -> Self {
            Self {
                focused: Some(initial_focus),
            }
        }
    }

    impl RootCompositor for MockCompositor {
        fn composite(&self, screen: Rect) -> CompositeResult {
            CompositeResult::empty(screen)
        }

        fn create_layer(&mut self, _config: LayerConfig) -> LayerId {
            LayerId::new(0)
        }

        fn remove_layer(&mut self, _layer: LayerId) {}

        fn layer_by_label(&self, _label: &str) -> Option<LayerId> {
            None
        }

        fn layers(&self) -> Vec<&Layer> {
            Vec::new()
        }

        fn set_layer_visible(&mut self, _layer: LayerId, _visible: bool) {}

        fn set_layer_opacity(&mut self, _layer: LayerId, _opacity: f32) {}

        fn reorder_layer(&mut self, _layer: LayerId, _new_z: u16) {}

        fn set_active_layer(&mut self, _layer: LayerId) {}

        fn active_layer(&self) -> Option<LayerId> {
            None
        }

        fn set_focus(&mut self, window: WindowId) {
            self.focused = Some(window);
        }

        fn focused(&self) -> Option<WindowId> {
            self.focused
        }

        fn focus_at(&mut self, _x: u16, _y: u16) -> Option<WindowId> {
            self.focused
        }

        fn layer_compositor(&self, _layer: LayerId) -> Option<&dyn WindowLayerCompositor> {
            None
        }

        fn layer_compositor_mut(
            &mut self,
            _layer: LayerId,
        ) -> Option<&mut dyn WindowLayerCompositor> {
            None
        }

        fn window_count(&self) -> usize {
            1
        }

        fn set_screen(&mut self, _screen: Rect) {}

        fn layer_of(&self, _window: WindowId) -> Option<LayerId> {
            None
        }

        fn boxed_clone(&self) -> Box<dyn RootCompositor> {
            Box::new(Self {
                focused: self.focused,
            })
        }
    }

    fn test_manager() -> TuiFocusManager<MockCompositor> {
        let compositor = Arc::new(Mutex::new(MockCompositor::new(WindowId::from_raw(1))));
        TuiFocusManager::new(compositor, 1)
    }

    #[test]
    fn test_focus_manager_new() {
        let manager = test_manager();
        assert!(manager.current().is_panel());
        assert_eq!(manager.current().viewport_id(), Some(1));
    }

    #[test]
    fn test_focus_manager_focus_panel() {
        let mut manager = test_manager();
        manager.focus_panel(2);

        assert!(manager.is_panel_focused(2));
        assert!(!manager.is_panel_focused(1));

        // Check compositor was updated
        let compositor = manager.compositor.lock().unwrap();
        assert_eq!(compositor.focused(), Some(WindowId::from_raw(2)));
    }

    #[test]
    fn test_focus_manager_focus_overlay() {
        let mut manager = test_manager();
        manager.focus_overlay("completion");

        assert!(manager.is_overlay_focused("completion"));
        assert!(manager.has_overlay_focus());
        assert!(!manager.is_panel_focused(1));

        // Compositor should still have focus on original panel
        // (overlays are rendered on top, but panel keeps compositor focus)
        let compositor = manager.compositor.lock().unwrap();
        assert_eq!(compositor.focused(), Some(WindowId::from_raw(1)));
    }

    #[test]
    fn test_focus_manager_return_to_panel() {
        let mut manager = test_manager();
        manager.focus_overlay("completion");
        assert!(manager.has_overlay_focus());

        manager.return_to_panel();
        assert!(manager.is_panel_focused(1));
        assert!(!manager.has_overlay_focus());

        // Check compositor was updated
        let compositor = manager.compositor.lock().unwrap();
        assert_eq!(compositor.focused(), Some(WindowId::from_raw(1)));
    }

    #[test]
    fn test_focus_manager_remembers_last_panel() {
        let mut manager = test_manager();
        manager.focus_panel(2);
        manager.focus_overlay("completion");
        manager.return_to_panel();

        // Should return to panel 2, not 1
        assert!(manager.is_panel_focused(2));
    }

    #[test]
    fn test_focus_manager_multiple_overlay_transitions() {
        let mut manager = test_manager();

        // Panel 1 -> Overlay -> Panel 1
        manager.focus_overlay("completion");
        manager.return_to_panel();
        assert!(manager.is_panel_focused(1));

        // Panel 1 -> Panel 2 -> Overlay -> Panel 2
        manager.focus_panel(2);
        manager.focus_overlay("hover");
        manager.return_to_panel();
        assert!(manager.is_panel_focused(2));
    }

    #[test]
    fn test_focus_manager_compositor_sync() {
        let mut manager = test_manager();

        // Focus panel 2
        manager.focus_panel(2);
        {
            let compositor = manager.compositor.lock().unwrap();
            assert_eq!(compositor.focused(), Some(WindowId::from_raw(2)));
        }

        // Focus panel 3
        manager.focus_panel(3);
        {
            let compositor = manager.compositor.lock().unwrap();
            assert_eq!(compositor.focused(), Some(WindowId::from_raw(3)));
        }
    }

    #[test]
    fn test_is_panel_focused() {
        let mut manager = test_manager();

        assert!(manager.is_panel_focused(1));
        assert!(!manager.is_panel_focused(2));

        manager.focus_panel(2);
        assert!(!manager.is_panel_focused(1));
        assert!(manager.is_panel_focused(2));
    }

    #[test]
    fn test_is_overlay_focused() {
        let mut manager = test_manager();

        assert!(!manager.is_overlay_focused("completion"));

        manager.focus_overlay("completion");
        assert!(manager.is_overlay_focused("completion"));
        assert!(!manager.is_overlay_focused("hover"));

        manager.focus_overlay("hover");
        assert!(!manager.is_overlay_focused("completion"));
        assert!(manager.is_overlay_focused("hover"));
    }

    #[test]
    fn test_has_overlay_focus() {
        let mut manager = test_manager();

        assert!(!manager.has_overlay_focus());

        manager.focus_overlay("completion");
        assert!(manager.has_overlay_focus());

        manager.return_to_panel();
        assert!(!manager.has_overlay_focus());
    }
}
