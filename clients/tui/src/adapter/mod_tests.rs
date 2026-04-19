use {
    super::*,
    reovim_client_model::traits::{FocusManager, Layout},
    reovim_subsys_layout::{
        CompositeResult, Layer, LayerConfig, LayerId, LayoutTopology, Rect, RootCompositor,
        WindowId, WindowLayerCompositor,
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

    fn layer_compositor(&self, _layer: LayerId) -> Option<&dyn WindowLayerCompositor> {
        None
    }

    fn layer_compositor_mut(&mut self, _layer: LayerId) -> Option<&mut dyn WindowLayerCompositor> {
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

    fn generation(&self) -> u64 {
        0
    }

    fn topology(&self) -> LayoutTopology {
        LayoutTopology::Single(WindowId::from_raw(0))
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

    // All adapters should be independently usable
    assert_eq!(layout1.focused_viewport(), 1);
    assert_eq!(layout2.focused_viewport(), 1);
    assert!(focus1.is_panel_focused(1));
    assert!(focus2.is_panel_focused(2));
}
