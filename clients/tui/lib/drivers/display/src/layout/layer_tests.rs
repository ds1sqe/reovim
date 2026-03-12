use super::*;

#[test]
fn test_zone_z_offset() {
    assert_eq!(Zone::Tiled.z_offset(), 0);
    assert_eq!(Zone::Float.z_offset(), 10);
    assert_eq!(Zone::Overlay.z_offset(), 50);
}

#[test]
fn test_zone_ordering() {
    // Zones should be ordered: Tiled < Float < Overlay
    assert!(Zone::Tiled < Zone::Float);
    assert!(Zone::Float < Zone::Overlay);
}

#[test]
fn test_layer_z_for() {
    let layer = Layer::new(LayerId::new(1), "main", ZOrder::new(100));

    // Tiled windows: 100, 101, 102...
    assert_eq!(layer.z_for(Zone::Tiled, 0), ZOrder::new(100));
    assert_eq!(layer.z_for(Zone::Tiled, 5), ZOrder::new(105));

    // Float windows: 110, 111, 112...
    assert_eq!(layer.z_for(Zone::Float, 0), ZOrder::new(110));
    assert_eq!(layer.z_for(Zone::Float, 3), ZOrder::new(113));

    // Overlay windows: 150, 151, 152...
    assert_eq!(layer.z_for(Zone::Overlay, 0), ZOrder::new(150));
    assert_eq!(layer.z_for(Zone::Overlay, 2), ZOrder::new(152));
}

#[test]
fn test_layer_id() {
    let id = LayerId::new(42);
    assert_eq!(id.as_u16(), 42);
}

#[test]
fn test_window_placement_new() {
    let window_id = WindowId::from_raw(1);
    let placement = WindowPlacement::new(
        window_id,
        LayerId::new(0),
        Zone::Tiled,
        Rect::new(0, 0, 80, 24),
        ZOrder::new(100),
    );

    assert_eq!(placement.window_id, window_id);
    assert_eq!(placement.zone, Zone::Tiled);
    assert_eq!(placement.z_order, ZOrder::new(100));
    assert!(placement.visible);
    assert!(placement.focusable);
    assert!((placement.opacity - 1.0).abs() < f32::EPSILON);
}

#[test]
fn test_click_through_threshold() {
    assert!((CLICK_THROUGH_THRESHOLD - 0.1).abs() < f32::EPSILON);
    // Layers below threshold should be click-through
    const { assert!(0.05 < CLICK_THROUGH_THRESHOLD) };
    // Layers at threshold should NOT be click-through
    assert!((CLICK_THROUGH_THRESHOLD - 0.1).abs() < f32::EPSILON);
}

#[test]
fn test_z_order_layer_base() {
    assert_eq!(ZOrder::layer_base(LayerId::new(0)), ZOrder::new(0));
    assert_eq!(ZOrder::layer_base(LayerId::new(1)), ZOrder::new(100));
    assert_eq!(ZOrder::layer_base(LayerId::new(2)), ZOrder::new(200));
}

#[test]
fn test_z_order_for_window() {
    let base = ZOrder::layer_base(LayerId::new(1));

    // Tiled: base + 0 + index
    assert_eq!(ZOrder::for_window(base, Zone::Tiled, 0), ZOrder::new(100));
    assert_eq!(ZOrder::for_window(base, Zone::Tiled, 5), ZOrder::new(105));

    // Float: base + 10 + index
    assert_eq!(ZOrder::for_window(base, Zone::Float, 0), ZOrder::new(110));
    assert_eq!(ZOrder::for_window(base, Zone::Float, 3), ZOrder::new(113));

    // Overlay: base + 50 + index
    assert_eq!(ZOrder::for_window(base, Zone::Overlay, 0), ZOrder::new(150));
}

#[test]
fn test_z_order_comparison() {
    let z1 = ZOrder::new(100);
    let z2 = ZOrder::new(150);

    assert!(z1 < z2);
    assert!(z2 > z1);
    assert_eq!(z1, ZOrder::new(100));
}

#[test]
fn test_overlay_constraints_builder() {
    let constraints = OverlayConstraints::centered()
        .with_size(40, 10)
        .with_max_size(60, 20);

    assert!(matches!(constraints.anchor, Anchor::Center));
    assert_eq!(constraints.preferred_width, Some(40));
    assert_eq!(constraints.preferred_height, Some(10));
    assert_eq!(constraints.max_width, Some(60));
    assert_eq!(constraints.max_height, Some(20));
}

#[test]
fn test_layer_config_fullscreen() {
    let config = LayerConfig::fullscreen("main");
    assert_eq!(config.label, "main");
    assert!(config.bounds.is_none());
    assert!((config.opacity - 1.0).abs() < f32::EPSILON);
}

#[test]
fn test_z_order_as_u16() {
    let z = ZOrder::new(42);
    assert_eq!(z.as_u16(), 42);

    let z_zero = ZOrder::new(0);
    assert_eq!(z_zero.as_u16(), 0);

    let z_max = ZOrder::new(u16::MAX);
    assert_eq!(z_max.as_u16(), u16::MAX);
}

#[test]
fn test_z_order_offset() {
    let z = ZOrder::new(100);
    let shifted = z.offset(25);
    assert_eq!(shifted.as_u16(), 125);

    let z_zero = ZOrder::new(0);
    assert_eq!(z_zero.offset(0).as_u16(), 0);
    assert_eq!(z_zero.offset(50).as_u16(), 50);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_overlay_constraints_at_cursor() {
    let window = WindowId::from_raw(5);
    let line = LineIndex::new(10);
    let col = ColIndex::new(20);
    let constraints = OverlayConstraints::at_cursor(window, line, col);

    match constraints.anchor {
        Anchor::Cursor {
            window: w,
            line: l,
            col: c,
        } => {
            assert_eq!(w, window);
            assert_eq!(l, line);
            assert_eq!(c, col);
        }
        _ => panic!("Expected Anchor::Cursor"),
    }
    assert_eq!(constraints.preferred_width, None);
    assert_eq!(constraints.preferred_height, None);
    assert_eq!(constraints.max_width, None);
    assert_eq!(constraints.max_height, None);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_overlay_constraints_at_cursor_raw() {
    let window = WindowId::from_raw(3);
    let constraints = OverlayConstraints::at_cursor_raw(window, 7, 15);

    match constraints.anchor {
        Anchor::Cursor {
            window: w,
            line,
            col,
        } => {
            assert_eq!(w, window);
            assert_eq!(line, LineIndex::new(7));
            assert_eq!(col, ColIndex::new(15));
        }
        _ => panic!("Expected Anchor::Cursor"),
    }
    assert_eq!(constraints.preferred_width, None);
    assert_eq!(constraints.preferred_height, None);
    assert_eq!(constraints.max_width, None);
    assert_eq!(constraints.max_height, None);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_overlay_constraints_at_position() {
    let constraints = OverlayConstraints::at_position(30, 10);

    match constraints.anchor {
        Anchor::Screen { x, y } => {
            assert_eq!(x, 30);
            assert_eq!(y, 10);
        }
        _ => panic!("Expected Anchor::Screen"),
    }
    assert_eq!(constraints.preferred_width, None);
    assert_eq!(constraints.preferred_height, None);
    assert_eq!(constraints.max_width, None);
    assert_eq!(constraints.max_height, None);
}

#[test]
fn test_layer_config_with_bounds() {
    let bounds = Rect::new(10, 5, 60, 20);
    let config = LayerConfig::with_bounds("sidebar", bounds);

    assert_eq!(config.label, "sidebar");
    assert_eq!(config.bounds, Some(bounds));
    assert!((config.opacity - 1.0).abs() < f32::EPSILON);
}

#[test]
fn test_layer_is_click_through_fully_opaque() {
    let layer = Layer::new(LayerId::new(0), "main", ZOrder::new(0));
    assert!(!layer.is_click_through());
}

#[test]
fn test_layer_is_click_through_at_threshold() {
    let mut layer = Layer::new(LayerId::new(0), "main", ZOrder::new(0));
    layer.opacity = CLICK_THROUGH_THRESHOLD;
    // At threshold (0.1), NOT click-through (< not <=)
    assert!(!layer.is_click_through());
}

#[test]
fn test_layer_is_click_through_below_threshold() {
    let mut layer = Layer::new(LayerId::new(0), "main", ZOrder::new(0));
    layer.opacity = 0.05;
    assert!(layer.is_click_through());
}

#[test]
fn test_layer_is_click_through_zero_opacity() {
    let mut layer = Layer::new(LayerId::new(0), "main", ZOrder::new(0));
    layer.opacity = 0.0;
    assert!(layer.is_click_through());
}

#[test]
fn test_placement_is_click_through_default() {
    let placement = WindowPlacement::new(
        WindowId::from_raw(1),
        LayerId::new(0),
        Zone::Tiled,
        Rect::new(0, 0, 80, 24),
        ZOrder::new(0),
    );
    // Default opacity is 1.0 — not click-through
    assert!(!placement.is_click_through());
}

#[test]
fn test_placement_is_click_through_transparent() {
    let mut placement = WindowPlacement::new(
        WindowId::from_raw(1),
        LayerId::new(0),
        Zone::Tiled,
        Rect::new(0, 0, 80, 24),
        ZOrder::new(0),
    );
    placement.opacity = 0.05;
    assert!(placement.is_click_through());
}
