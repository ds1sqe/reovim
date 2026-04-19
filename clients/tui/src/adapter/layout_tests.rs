use {
    super::*,
    reovim_subsys_layout::{
        Layer, LayerConfig, LayerId, LayoutTopology, Rect, RootCompositor, WindowId,
        WindowLayerCompositor,
    },
};

// Mock compositor for testing
struct MockCompositor {
    windows: Vec<WindowId>,
    focused: Option<WindowId>,
    next_id: usize,
}

impl MockCompositor {
    fn new() -> Self {
        let first = WindowId::from_raw(1);
        Self {
            windows: vec![first],
            focused: Some(first),
            next_id: 2,
        }
    }
}

// Minimal RootCompositor implementation for testing
#[cfg_attr(coverage_nightly, coverage(off))]
impl RootCompositor for MockCompositor {
    fn composite(&self, screen: Rect) -> reovim_subsys_layout::CompositeResult {
        reovim_subsys_layout::CompositeResult::empty(screen)
    }

    fn create_layer(&mut self, _config: LayerConfig) -> LayerId {
        LayerId::new(0)
    }

    fn remove_layer(&mut self, _layer: LayerId) {}

    fn layer_by_label(&self, _label: &str) -> Option<LayerId> {
        Some(LayerId::new(0))
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
        if self.windows.contains(&window) {
            self.focused = Some(window);
        }
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
        self.windows.len()
    }

    fn set_screen(&mut self, _screen: Rect) {}

    fn layer_of(&self, _window: WindowId) -> Option<LayerId> {
        Some(LayerId::new(0))
    }

    fn boxed_clone(&self) -> Box<dyn RootCompositor> {
        Box::new(Self {
            windows: self.windows.clone(),
            focused: self.focused,
            next_id: self.next_id,
        })
    }

    fn generation(&self) -> u64 {
        0
    }

    fn topology(&self) -> LayoutTopology {
        LayoutTopology::default()
    }

    fn tiled_tree(&self, _layer: LayerId) -> Option<&reovim_subsys_layout::TiledTree> {
        None
    }
}

#[test]
fn test_layout_adapter_new() {
    let compositor = Arc::new(Mutex::new(MockCompositor::new()));
    let adapter = TuiLayoutAdapter::new(compositor, LayerId::new(0));
    assert_eq!(adapter.active_layer(), LayerId::new(0));
}

#[test]
fn test_layout_adapter_focused_viewport() {
    let compositor = Arc::new(Mutex::new(MockCompositor::new()));
    let adapter = TuiLayoutAdapter::new(compositor, LayerId::new(0));
    assert_eq!(adapter.focused_viewport(), 1); // First window ID
}

#[test]
fn test_layout_adapter_window_count() {
    let compositor = Arc::new(Mutex::new(MockCompositor::new()));
    let adapter = TuiLayoutAdapter::new(compositor, LayerId::new(0));
    assert_eq!(adapter.window_count(), 1);
}

#[test]
fn test_layout_adapter_is_single() {
    let compositor = Arc::new(Mutex::new(MockCompositor::new()));
    let adapter = TuiLayoutAdapter::new(compositor, LayerId::new(0));
    assert!(adapter.is_single());
}

#[test]
fn test_layout_adapter_to_logical() {
    let compositor = Arc::new(Mutex::new(MockCompositor::new()));
    let adapter = TuiLayoutAdapter::new(compositor, LayerId::new(0));
    let logical = adapter.to_logical();
    assert!(logical.is_leaf());
}

#[test]
fn test_layout_adapter_focus() {
    let compositor = Arc::new(Mutex::new(MockCompositor::new()));
    let mut adapter = TuiLayoutAdapter::new(compositor.clone(), LayerId::new(0));

    // Add another window to the mock compositor
    {
        let mut comp = compositor.lock().unwrap();
        let new_id = WindowId::from_raw(2);
        comp.windows.push(new_id);
    }

    // Focus should succeed
    assert!(adapter.focus(2));
    assert_eq!(adapter.focused_viewport(), 2);
}

#[test]
fn test_window_id_conversion() {
    let id = WindowId::from_raw(42);
    let u64_id = TuiLayoutAdapter::<MockCompositor>::window_id_to_u64(id);
    assert_eq!(u64_id, 42);

    let back = TuiLayoutAdapter::<MockCompositor>::u64_to_window_id(u64_id);
    assert_eq!(back, id);
}

#[test]
fn test_direction_conversion() {
    assert!(matches!(
        TuiLayoutAdapter::<MockCompositor>::to_navigate_direction(Direction::Up),
        NavigateDirection::Up
    ));
    assert!(matches!(
        TuiLayoutAdapter::<MockCompositor>::to_navigate_direction(Direction::Down),
        NavigateDirection::Down
    ));
    assert!(matches!(
        TuiLayoutAdapter::<MockCompositor>::to_navigate_direction(Direction::Left),
        NavigateDirection::Left
    ));
    assert!(matches!(
        TuiLayoutAdapter::<MockCompositor>::to_navigate_direction(Direction::Right),
        NavigateDirection::Right
    ));
}

#[test]
fn test_split_direction_conversion() {
    assert!(matches!(
        TuiLayoutAdapter::<MockCompositor>::to_split_direction(SplitDirection::Horizontal),
        reovim_driver_display::SplitDirection::Horizontal
    ));
    assert!(matches!(
        TuiLayoutAdapter::<MockCompositor>::to_split_direction(SplitDirection::Vertical),
        reovim_driver_display::SplitDirection::Vertical
    ));
}

#[test]
fn test_layout_adapter_compositor_accessor() {
    let compositor = Arc::new(Mutex::new(MockCompositor::new()));
    let adapter = TuiLayoutAdapter::new(compositor.clone(), LayerId::new(0));

    let comp_ref = adapter.compositor();
    assert_eq!(Arc::strong_count(&compositor), 3);
    drop(comp_ref);
}

#[test]
fn test_layout_adapter_set_active_layer() {
    let compositor = Arc::new(Mutex::new(MockCompositor::new()));
    let mut adapter = TuiLayoutAdapter::new(compositor, LayerId::new(0));
    assert_eq!(adapter.active_layer(), LayerId::new(0));

    adapter.set_active_layer(LayerId::new(5));
    assert_eq!(adapter.active_layer(), LayerId::new(5));
}

#[test]
fn test_layout_adapter_split_no_layer() {
    // MockCompositor returns None for layer_compositor_mut, so split should return 0
    let compositor = Arc::new(Mutex::new(MockCompositor::new()));
    let mut adapter = TuiLayoutAdapter::new(compositor, LayerId::new(0));

    let result = adapter.split(SplitDirection::Horizontal);
    assert_eq!(result, 0); // No layer compositor -> failure
}

#[test]
fn test_layout_adapter_close_no_layer() {
    let compositor = Arc::new(Mutex::new(MockCompositor::new()));
    let mut adapter = TuiLayoutAdapter::new(compositor, LayerId::new(0));

    let result = adapter.close(1);
    assert!(!result); // No layer compositor -> failure
}

#[test]
fn test_layout_adapter_focus_direction_no_layer() {
    let compositor = Arc::new(Mutex::new(MockCompositor::new()));
    let mut adapter = TuiLayoutAdapter::new(compositor, LayerId::new(0));

    let result = adapter.focus_direction(Direction::Left);
    assert!(!result); // No layer compositor -> failure
}

#[test]
fn test_layout_adapter_focus_returns_true() {
    let compositor = Arc::new(Mutex::new(MockCompositor::new()));
    let mut adapter = TuiLayoutAdapter::new(compositor, LayerId::new(0));

    // Focus should always return true
    assert!(adapter.focus(99));
    // The compositor might not track the window, but focus() unconditionally returns true
}

#[test]
fn test_layout_adapter_apply_layout_noop() {
    let compositor = Arc::new(Mutex::new(MockCompositor::new()));
    let mut adapter = TuiLayoutAdapter::new(compositor, LayerId::new(0));

    let layout = LogicalLayout::single(1, 1);
    // apply_layout is a no-op but should not panic
    adapter.apply_layout(&layout);
}

#[test]
fn test_layout_adapter_to_logical_no_focus() {
    let compositor = Arc::new(Mutex::new(MockCompositor {
        windows: vec![],
        focused: None,
        next_id: 1,
    }));
    let adapter = TuiLayoutAdapter::new(compositor, LayerId::new(0));

    let logical = adapter.to_logical();
    // With no focus, should return single(0, 0)
    assert!(logical.is_leaf());
}

#[test]
fn test_window_id_conversion_zero() {
    let id = WindowId::from_raw(0);
    let u64_id = TuiLayoutAdapter::<MockCompositor>::window_id_to_u64(id);
    assert_eq!(u64_id, 0);

    let back = TuiLayoutAdapter::<MockCompositor>::u64_to_window_id(0);
    assert_eq!(back, id);
}

#[test]
fn test_window_id_conversion_large() {
    let id = WindowId::from_raw(999_999);
    let u64_id = TuiLayoutAdapter::<MockCompositor>::window_id_to_u64(id);
    assert_eq!(u64_id, 999_999);

    let back = TuiLayoutAdapter::<MockCompositor>::u64_to_window_id(u64_id);
    assert_eq!(back, id);
}
