use super::*;

#[test]
fn test_overlay_window_struct() {
    let ow = OverlayWindow {
        id: WindowId::from_raw(10),
        constraints: OverlayConstraints::centered().with_size(30, 10),
        computed_bounds: Rect::new(5, 5, 30, 10),
        z_order: ZOrder::new(100),
    };
    assert_eq!(ow.id, WindowId::from_raw(10));
    assert_eq!(ow.computed_bounds.width, 30);
    assert_eq!(ow.z_order, ZOrder::new(100));
}

#[test]
fn test_overlay_window_clone() {
    let ow = OverlayWindow {
        id: WindowId::from_raw(5),
        constraints: OverlayConstraints::centered()
            .with_size(20, 8)
            .with_max_size(40, 16),
        computed_bounds: Rect::new(10, 8, 20, 8),
        z_order: ZOrder::new(50),
    };
    let cloned = ow.clone();
    assert_eq!(ow.id, cloned.id);
    assert_eq!(ow.computed_bounds, cloned.computed_bounds);
}

#[test]
fn test_overlay_window_debug() {
    let ow = OverlayWindow {
        id: WindowId::from_raw(1),
        constraints: OverlayConstraints::at_position(0, 0),
        computed_bounds: Rect::new(0, 0, 10, 10),
        z_order: ZOrder::new(0),
    };
    let debug = format!("{ow:?}");
    assert!(debug.contains("OverlayWindow"));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_overlay_layer_is_object_safe() {
    fn _accepts_ref(_: &dyn OverlayLayer) {}
    fn _accepts_box(_: Box<dyn OverlayLayer>) {}
}
