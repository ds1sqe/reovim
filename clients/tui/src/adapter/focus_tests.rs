use {
    super::*,
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
    fn new(initial_focus: WindowId) -> Self {
        Self {
            focused: Some(initial_focus),
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
        None
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
        None
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
#[cfg_attr(coverage_nightly, coverage(off))]
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

#[test]
fn test_focus_manager_compositor_accessor() {
    let manager = test_manager();
    let comp = manager.compositor();
    let guard = comp.lock().unwrap();
    assert_eq!(guard.focused(), Some(WindowId::from_raw(1)));
}

#[test]
fn test_focus_overlay_preserves_last_panel_from_different_panels() {
    let mut manager = test_manager();

    // Focus panel 3, then switch to overlay
    manager.focus_panel(3);
    manager.focus_overlay("menu");

    // Return should go to panel 3
    manager.return_to_panel();
    assert!(manager.is_panel_focused(3));
}

#[test]
fn test_focus_overlay_twice_remembers_first_panel() {
    let mut manager = test_manager();

    // Start at panel 1, go to overlay A, then overlay B
    manager.focus_overlay("a");
    manager.focus_overlay("b");

    // Return should go to panel 1 (the last panel before first overlay)
    manager.return_to_panel();
    assert!(manager.is_panel_focused(1));
}

#[test]
fn test_focus_panel_after_overlay_updates_last_panel() {
    let mut manager = test_manager();

    manager.focus_overlay("completion");
    // Going directly to panel 5 (not via return_to_panel)
    manager.focus_panel(5);
    manager.focus_overlay("hover");
    manager.return_to_panel();

    // Should return to panel 5 (the last panel focused)
    assert!(manager.is_panel_focused(5));
}

#[test]
fn test_u64_to_window_id_conversion() {
    let wid = TuiFocusManager::<MockCompositor>::u64_to_window_id(42);
    assert_eq!(wid, WindowId::from_raw(42));
}
