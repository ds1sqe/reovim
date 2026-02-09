//! Adapter module for common client model integration.
//!
//! This module provides adapters between the common client model types
//! (`reovim-client-model`) and TUI-specific types (`reovim-driver-display`).
//!
//! # Architecture
//!
//! The adapter module follows the "extension, not replacement" philosophy:
//! - Common model types are used for data interchange (wire format)
//! - TUI keeps its sophisticated compositor for rendering
//! - Adapters convert between wire format and TUI types
//!
//! # Usage
//!
//! The [`TuiAdapterFactory`] provides a convenient way to create all adapters:
//!
//! ```ignore
//! let compositor = Arc::new(Mutex::new(my_compositor));
//! let factory = TuiAdapterFactory::new(compositor, active_layer);
//!
//! // Create adapters
//! let layout = factory.create_layout();
//! let focus = factory.create_focus_manager(initial_viewport);
//! let overlay = TuiOverlayManager::new();
//! ```
//!
//! # Modules
//!
//! - [`anchor`] - Convert wire anchors to TUI anchors
//! - [`layout`] - Layout trait implementation wrapping TUI compositor
//! - [`panel`] - Panel trait implementation wrapping TUI View
//! - [`overlay`] - Overlay manager wrapping TUI `OverlayLayer`
//! - [`focus`] - Focus manager for panel/overlay focus tracking

use std::sync::{Arc, Mutex};

use reovim_driver_display::layout::{LayerId, RootCompositor};

pub mod anchor;
pub mod focus;
pub mod layout;
pub mod overlay;
pub mod panel;

// Re-export key types for convenience
pub use {
    anchor::{AnchorContext, convert_anchor},
    focus::TuiFocusManager,
    layout::TuiLayoutAdapter,
    overlay::TuiOverlayManager,
    panel::TuiPanel,
};

/// Factory for creating TUI adapters from a compositor.
///
/// Provides a convenient way to create multiple adapters that share
/// the same underlying compositor.
///
/// # Type Parameter
///
/// * `C` - The compositor type (must implement `RootCompositor + 'static`)
pub struct TuiAdapterFactory<C: RootCompositor + 'static> {
    /// The underlying root compositor.
    compositor: Arc<Mutex<C>>,
    /// The active layer for tiled operations.
    active_layer: LayerId,
}

impl<C: RootCompositor + 'static> TuiAdapterFactory<C> {
    /// Create a new adapter factory.
    ///
    /// # Arguments
    ///
    /// * `compositor` - The root compositor to wrap
    /// * `active_layer` - The layer ID for tiled window operations
    #[must_use]
    pub const fn new(compositor: Arc<Mutex<C>>, active_layer: LayerId) -> Self {
        Self {
            compositor,
            active_layer,
        }
    }

    /// Get a reference to the compositor.
    #[must_use]
    pub fn compositor(&self) -> Arc<Mutex<C>> {
        Arc::clone(&self.compositor)
    }

    /// Get the active layer ID.
    #[must_use]
    pub const fn active_layer(&self) -> LayerId {
        self.active_layer
    }

    /// Create a layout adapter.
    #[must_use]
    pub fn create_layout(&self) -> TuiLayoutAdapter<C> {
        TuiLayoutAdapter::new(Arc::clone(&self.compositor), self.active_layer)
    }

    /// Create a focus manager.
    ///
    /// # Arguments
    ///
    /// * `initial_viewport` - The initially focused viewport ID
    #[must_use]
    pub fn create_focus_manager(&self, initial_viewport: u64) -> TuiFocusManager<C> {
        TuiFocusManager::new(Arc::clone(&self.compositor), initial_viewport)
    }

    /// Create an overlay manager.
    ///
    /// Note: The overlay manager is created without an underlying layer.
    /// Call `TuiOverlayManager::with_layer()` directly if you need to
    /// connect it to a TUI overlay layer.
    #[must_use]
    pub fn create_overlay_manager(&self) -> TuiOverlayManager {
        TuiOverlayManager::new()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_client_model::traits::{FocusManager, Layout, OverlayManager},
        reovim_driver_display::{
            Rect, WindowId,
            layout::{CompositeResult, Layer, LayerConfig, WindowLayerCompositor},
        },
    };

    // Mock compositor for testing
    struct MockCompositor {
        focused: Option<WindowId>,
    }

    impl MockCompositor {
        fn new() -> Self {
            Self {
                focused: Some(WindowId::from_raw(1)),
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
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
            Some(LayerId::new(0))
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
            Some(LayerId::new(0))
        }

        fn boxed_clone(&self) -> Box<dyn RootCompositor> {
            Box::new(Self {
                focused: self.focused,
            })
        }
    }

    #[test]
    fn test_factory_new() {
        let compositor = Arc::new(Mutex::new(MockCompositor::new()));
        let factory = TuiAdapterFactory::new(compositor, LayerId::new(0));
        assert_eq!(factory.active_layer(), LayerId::new(0));
    }

    #[test]
    fn test_factory_create_layout() {
        let compositor = Arc::new(Mutex::new(MockCompositor::new()));
        let factory = TuiAdapterFactory::new(compositor, LayerId::new(0));
        let layout = factory.create_layout();
        assert_eq!(layout.active_layer(), LayerId::new(0));
    }

    #[test]
    fn test_factory_create_focus_manager() {
        let compositor = Arc::new(Mutex::new(MockCompositor::new()));
        let factory = TuiAdapterFactory::new(compositor, LayerId::new(0));
        let focus = factory.create_focus_manager(1);
        assert!(focus.is_panel_focused(1));
    }

    #[test]
    fn test_factory_create_overlay_manager() {
        let compositor = Arc::new(Mutex::new(MockCompositor::new()));
        let factory = TuiAdapterFactory::new(compositor, LayerId::new(0));
        let overlay = factory.create_overlay_manager();
        assert!(overlay.active().is_empty());
    }

    #[test]
    fn test_factory_shared_compositor() {
        let compositor = Arc::new(Mutex::new(MockCompositor::new()));
        let factory = TuiAdapterFactory::new(compositor, LayerId::new(0));

        let _layout = factory.create_layout();
        let mut focus = factory.create_focus_manager(1);

        // Focus manager changes focus
        focus.focus_panel(2);

        // Layout adapter should see the change through shared compositor
        let layout = factory.create_layout();
        assert_eq!(layout.focused_viewport(), 2);
    }

    #[test]
    fn test_integration_layout_and_focus() {
        let compositor = Arc::new(Mutex::new(MockCompositor::new()));
        let factory = TuiAdapterFactory::new(compositor, LayerId::new(0));

        let layout = factory.create_layout();
        let mut focus = factory.create_focus_manager(1);

        // Initial state
        assert_eq!(layout.focused_viewport(), 1);
        assert!(focus.is_panel_focused(1));

        // Focus changes through focus manager
        focus.focus_panel(2);
        assert_eq!(layout.focused_viewport(), 2);
        assert!(focus.is_panel_focused(2));
    }

    #[test]
    fn test_factory_compositor_accessor() {
        let compositor = Arc::new(Mutex::new(MockCompositor::new()));
        let factory = TuiAdapterFactory::new(compositor.clone(), LayerId::new(0));

        // compositor() should return a cloned Arc pointing to the same compositor
        let comp_ref = factory.compositor();
        assert_eq!(Arc::strong_count(&compositor), 3); // original + factory + comp_ref
        drop(comp_ref);
        assert_eq!(Arc::strong_count(&compositor), 2);
    }

    #[test]
    fn test_factory_active_layer() {
        let compositor = Arc::new(Mutex::new(MockCompositor::new()));
        let factory = TuiAdapterFactory::new(compositor, LayerId::new(5));
        assert_eq!(factory.active_layer(), LayerId::new(5));
    }

    #[test]
    fn test_multiple_adapters_from_factory() {
        let compositor = Arc::new(Mutex::new(MockCompositor::new()));
        let factory = TuiAdapterFactory::new(compositor, LayerId::new(0));

        let layout1 = factory.create_layout();
        let layout2 = factory.create_layout();
        let focus1 = factory.create_focus_manager(1);
        let focus2 = factory.create_focus_manager(2);
        let overlay = factory.create_overlay_manager();

        // All adapters should be independently usable
        assert_eq!(layout1.focused_viewport(), 1);
        assert_eq!(layout2.focused_viewport(), 1);
        assert!(focus1.is_panel_focused(1));
        assert!(focus2.is_panel_focused(2));
        assert!(overlay.active().is_empty());
    }
}
