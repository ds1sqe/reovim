use {
    super::*,
    reovim_subsys_layout::{LayerId, Rect, SplitDirection, WindowLayerCompositor, Zone},
};

fn layer() -> HybridLayerCompositor {
    HybridLayerCompositor::new(LayerId::new(0))
}

#[test]
fn layer_id() {
    let l = layer();
    assert_eq!(l.id(), LayerId::new(0));
}

#[test]
fn add_tiled_creates_window() {
    let mut l = layer();
    let id = l.add_tiled();
    assert_eq!(l.focused(), Some(id));
    assert_eq!(l.windows_in_zone(Zone::Tiled), vec![id]);
}

#[test]
fn split_tiled_updates_focus() {
    let mut l = layer();
    let a = l.add_tiled();
    let b = l.split_tiled(a, SplitDirection::Vertical).unwrap();
    assert_eq!(l.focused(), Some(b));
}

#[test]
fn close_tiled_updates_focus() {
    let mut l = layer();
    let a = l.add_tiled();
    let b = l.split_tiled(a, SplitDirection::Vertical).unwrap();
    let focus = l.close_tiled(b).unwrap();
    assert_eq!(focus, a);
    assert_eq!(l.focused(), Some(a));
}

#[test]
fn arrange_returns_placements() {
    let mut l = layer();
    l.add_tiled();
    let placements = l.arrange(Rect::new(0, 0, 80, 24));
    assert_eq!(placements.len(), 1);
}

#[test]
fn zone_of_tiled() {
    let mut l = layer();
    let id = l.add_tiled();
    assert_eq!(l.zone_of(id), Some(Zone::Tiled));
}

#[test]
fn zone_of_unknown() {
    let l = layer();
    let fake = WindowId::new();
    assert!(l.zone_of(fake).is_none());
}

#[test]
fn float_zone_stubs() {
    let mut l = layer();
    let f = l.create_float(Rect::new(0, 0, 40, 12));
    // Float windows are not tracked in the tiled zone
    assert!(l.windows_in_zone(Zone::Float).is_empty());
    // But we got a valid ID back
    assert_ne!(f.as_usize(), 0);
    // No-op operations don't panic
    l.move_float(f, 10, 10);
    l.resize_float(f, 20, 10);
    l.raise_float(f);
    l.lower_float(f);
    l.close_float(f);
    l.toggle_float(f);
}

#[test]
fn overlay_zone_stubs() {
    let mut l = layer();
    let o = l.show_overlay(OverlayConstraints::centered());
    assert_ne!(o.as_usize(), 0);
    l.resize_overlay(o, 20, 10);
    l.hide_overlay(o);
    l.hide_all_overlays();
    assert!(l.windows_in_zone(Zone::Overlay).is_empty());
}

#[test]
fn set_focus() {
    let mut l = layer();
    let a = l.add_tiled();
    let b = l.split_tiled(a, SplitDirection::Vertical).unwrap();
    l.set_focus(a);
    assert_eq!(l.focused(), Some(a));
    l.set_focus(b);
    assert_eq!(l.focused(), Some(b));
}

#[test]
fn navigate_tiled_delegates() {
    let mut l = layer();
    let a = l.add_tiled();
    let b = l.split_tiled(a, SplitDirection::Vertical).unwrap();
    let target = l.navigate_tiled(a, Direction::Right);
    assert_eq!(target, Some(b));
}

#[test]
fn equalize_tiled() {
    let mut l = layer();
    let a = l.add_tiled();
    l.split_tiled(a, SplitDirection::Vertical);
    l.resize_tiled(a, Direction::Right, 10);
    l.equalize_tiled();
    let placements = l.arrange(Rect::new(0, 0, 80, 24));
    assert_eq!(placements[0].bounds.width, 40);
}

#[test]
fn cycle_tiled() {
    let mut l = layer();
    let a = l.add_tiled();
    let b = l.split_tiled(a, SplitDirection::Vertical).unwrap();
    let next = l.cycle_tiled(a, true);
    assert_eq!(next, Some(b));
}
