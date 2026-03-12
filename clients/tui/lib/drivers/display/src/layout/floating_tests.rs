use super::*;

#[test]
fn test_floating_window_struct() {
    let fw = FloatingWindow {
        id: WindowId::from_raw(1),
        bounds: Rect::new(10, 20, 30, 40),
        z_order: ZOrder::new(5),
    };
    assert_eq!(fw.id, WindowId::from_raw(1));
    assert_eq!(fw.bounds.x, 10);
    assert_eq!(fw.bounds.y, 20);
    assert_eq!(fw.bounds.width, 30);
    assert_eq!(fw.bounds.height, 40);
    assert_eq!(fw.z_order, ZOrder::new(5));
}

#[test]
fn test_floating_window_clone() {
    let fw = FloatingWindow {
        id: WindowId::from_raw(2),
        bounds: Rect::new(0, 0, 80, 24),
        z_order: ZOrder::new(0),
    };
    let cloned = fw.clone();
    assert_eq!(fw.id, cloned.id);
    assert_eq!(fw.bounds, cloned.bounds);
    assert_eq!(fw.z_order, cloned.z_order);
}

#[test]
fn test_floating_window_debug() {
    let fw = FloatingWindow {
        id: WindowId::from_raw(3),
        bounds: Rect::new(0, 0, 10, 10),
        z_order: ZOrder::new(1),
    };
    let debug = format!("{fw:?}");
    assert!(debug.contains("FloatingWindow"));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_floating_layer_is_object_safe() {
    fn _accepts_ref(_: &dyn FloatingLayer) {}
    fn _accepts_box(_: Box<dyn FloatingLayer>) {}
}
