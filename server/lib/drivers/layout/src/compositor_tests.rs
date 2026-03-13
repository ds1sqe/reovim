use super::*;

#[test]
fn test_composite_result_empty() {
    let result = CompositeResult::empty(Rect::new(0, 0, 80, 24));
    assert!(result.placements.is_empty());
    assert!(result.focused.is_none());
    assert!(result.active_layer.is_none());
}

#[test]
fn test_window_error_vim_codes() {
    assert_eq!(WindowError::CannotCloseLastWindow.vim_code(), "E444");
    assert_eq!(WindowError::NoNeighbor(NavigateDirection::Left).vim_code(), "E36");
    assert_eq!(WindowError::NotEnoughRoom.vim_code(), "E94");
    assert_eq!(WindowError::CannotResizeAtEdge.vim_code(), "E36");
}

#[test]
fn test_window_error_display() {
    let error = WindowError::CannotCloseLastWindow;
    assert!(error.to_string().contains("E444"));
    assert!(error.to_string().contains("Cannot close last window"));
}

#[test]
fn test_composite_result_get_placement() {
    use super::super::layer::ZOrder;

    let result = CompositeResult {
        placements: vec![
            WindowPlacement {
                window_id: WindowId::from_raw(1),
                bounds: Rect::new(0, 0, 40, 24),
                z_order: ZOrder::new(0),
                zone: Zone::Tiled,
                layer_id: LayerId::new(0),
                visible: true,
                focusable: true,
                opacity: 1.0,
            },
            WindowPlacement {
                window_id: WindowId::from_raw(2),
                bounds: Rect::new(40, 0, 40, 24),
                z_order: ZOrder::new(0),
                zone: Zone::Tiled,
                layer_id: LayerId::new(0),
                visible: true,
                focusable: true,
                opacity: 1.0,
            },
        ],
        focused: Some(WindowId::from_raw(1)),
        active_layer: Some(LayerId::new(0)),
        screen: Rect::new(0, 0, 80, 24),
    };

    // Found
    let placement = result.get_placement(WindowId::from_raw(1));
    assert!(placement.is_some());
    assert_eq!(placement.unwrap().bounds.width, 40);

    // Not found
    let placement = result.get_placement(WindowId::from_raw(99));
    assert!(placement.is_none());
}

#[test]
fn test_window_error_all_variants() {
    let errors = [
        WindowError::CannotCloseLastWindow,
        WindowError::NoNeighbor(NavigateDirection::Right),
        WindowError::NotEnoughRoom,
        WindowError::CannotResizeAtEdge,
        WindowError::WindowNotFound(WindowId::from_raw(1)),
        WindowError::LayerNotFound(LayerId::new(0)),
    ];

    for error in &errors {
        // All should produce non-empty vim code and message
        assert!(!error.vim_code().is_empty());
        assert!(!error.message().is_empty());
        // Display impl should work
        let display = error.to_string();
        assert!(!display.is_empty());
    }
}

#[test]
fn test_window_error_messages() {
    assert!(
        WindowError::NoNeighbor(NavigateDirection::Left)
            .message()
            .contains("Left")
    );
    assert!(
        WindowError::NotEnoughRoom
            .message()
            .contains("Not enough room")
    );
    assert!(
        WindowError::CannotResizeAtEdge
            .message()
            .contains("Cannot resize")
    );
    assert!(
        WindowError::WindowNotFound(WindowId::from_raw(42))
            .message()
            .contains("42")
    );
    assert!(
        WindowError::LayerNotFound(LayerId::new(7))
            .message()
            .contains('7')
    );
}

#[test]
fn test_window_error_is_std_error() {
    let error = WindowError::CannotCloseLastWindow;
    let _: &dyn std::error::Error = &error;
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_root_compositor_is_object_safe() {
    fn _accepts_ref(_: &dyn RootCompositor) {}
    fn _accepts_box(_: Box<dyn RootCompositor>) {}
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_window_layer_compositor_is_object_safe() {
    fn _accepts_ref(_: &dyn WindowLayerCompositor) {}
    fn _accepts_box(_: Box<dyn WindowLayerCompositor>) {}
}
