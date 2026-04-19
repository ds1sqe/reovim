use {
    super::*,
    reovim_subsys_layout::{LayerConfig, Rect, RootCompositor, SplitDirection, TiledTree},
};

fn screen() -> Rect {
    Rect::new(0, 0, 80, 24)
}

// =============================================================================
// Creation
// =============================================================================

#[test]
fn new_has_main_layer() {
    let c = HybridCompositor::new();
    assert_eq!(c.layers().len(), 1);
    assert_eq!(c.layers()[0].label, "main");
    assert_eq!(c.active_layer(), Some(LayerId::new(0)));
}

#[test]
fn default_matches_new() {
    let c = HybridCompositor::default();
    assert_eq!(c.layers().len(), 1);
}

// =============================================================================
// Composite
// =============================================================================

#[test]
fn composite_empty_layer() {
    let c = HybridCompositor::new();
    let result = c.composite(screen());
    assert!(result.placements.is_empty());
    assert_eq!(result.screen, screen());
}

#[test]
fn composite_with_window() {
    let mut c = HybridCompositor::new();
    let layer_id = c.active_layer().unwrap();
    let layer = c.layer_compositor_mut(layer_id).unwrap();
    layer.add_tiled();
    let result = c.composite(screen());
    assert_eq!(result.placements.len(), 1);
    assert_eq!(result.placements[0].bounds, screen());
}

#[test]
fn composite_with_split() {
    let mut c = HybridCompositor::new();
    let layer_id = c.active_layer().unwrap();
    let layer = c.layer_compositor_mut(layer_id).unwrap();
    let a = layer.add_tiled();
    layer.split_tiled(a, SplitDirection::Vertical);
    let result = c.composite(screen());
    assert_eq!(result.placements.len(), 2);
}

// =============================================================================
// Layer management
// =============================================================================

#[test]
fn create_layer() {
    let mut c = HybridCompositor::new();
    let id = c.create_layer(LayerConfig::fullscreen("secondary"));
    assert_eq!(c.layers().len(), 2);
    assert!(c.layer_by_label("secondary").is_some());
    assert_eq!(c.layer_by_label("secondary"), Some(id));
}

#[test]
fn remove_layer() {
    let mut c = HybridCompositor::new();
    let id = c.create_layer(LayerConfig::fullscreen("temp"));
    c.remove_layer(id);
    assert_eq!(c.layers().len(), 1);
}

#[test]
fn remove_active_layer_switches() {
    let mut c = HybridCompositor::new();
    let second = c.create_layer(LayerConfig::fullscreen("second"));
    c.set_active_layer(second);
    c.remove_layer(second);
    // Falls back to first layer
    assert_eq!(c.active_layer(), Some(LayerId::new(0)));
}

#[test]
fn set_layer_visible() {
    let mut c = HybridCompositor::new();
    let id = c.active_layer().unwrap();
    c.set_layer_visible(id, false);
    assert!(!c.layers()[0].visible);
    // Hidden layer produces no placements
    let layer = c.layer_compositor_mut(id).unwrap();
    layer.add_tiled();
    let result = c.composite(screen());
    assert!(result.placements.is_empty());
}

#[test]
fn set_layer_opacity() {
    let mut c = HybridCompositor::new();
    let id = c.active_layer().unwrap();
    c.set_layer_opacity(id, 0.5);
    let layer = c.layer_compositor_mut(id).unwrap();
    layer.add_tiled();
    let result = c.composite(screen());
    let eps = f32::EPSILON;
    assert!((result.placements[0].opacity - 0.5).abs() < eps);
}

// =============================================================================
// Focus
// =============================================================================

#[test]
fn set_focus_activates_layer() {
    let mut c = HybridCompositor::new();
    let layer_id = c.active_layer().unwrap();
    let layer = c.layer_compositor_mut(layer_id).unwrap();
    let win = layer.add_tiled();

    c.set_focus(win);
    assert_eq!(c.focused(), Some(win));
    assert_eq!(c.active_layer(), Some(layer_id));
}

// =============================================================================
// Queries
// =============================================================================

#[test]
fn window_count() {
    let mut c = HybridCompositor::new();
    assert_eq!(c.window_count(), 0);
    let id = c.active_layer().unwrap();
    let layer = c.layer_compositor_mut(id).unwrap();
    let a = layer.add_tiled();
    assert_eq!(c.window_count(), 1);
    let layer = c.layer_compositor_mut(id).unwrap();
    layer.split_tiled(a, SplitDirection::Vertical);
    assert_eq!(c.window_count(), 2);
}

#[test]
fn layer_of() {
    let mut c = HybridCompositor::new();
    let layer_id = c.active_layer().unwrap();
    let layer = c.layer_compositor_mut(layer_id).unwrap();
    let win = layer.add_tiled();
    assert_eq!(c.layer_of(win), Some(layer_id));
}

#[test]
fn layer_of_unknown_window() {
    let c = HybridCompositor::new();
    assert!(c.layer_of(WindowId::new()).is_none());
}

#[test]
fn layer_by_label_not_found() {
    let c = HybridCompositor::new();
    assert!(c.layer_by_label("nonexistent").is_none());
}

#[test]
fn set_screen() {
    let mut c = HybridCompositor::new();
    c.set_screen(Rect::new(0, 0, 120, 40));
    assert_eq!(c.screen, Rect::new(0, 0, 120, 40));
}

#[test]
fn reorder_layer() {
    let mut c = HybridCompositor::new();
    let id = c.active_layer().unwrap();
    c.reorder_layer(id, 500);
    assert_eq!(c.layers()[0].z_base, ZOrder::new(500));
}

// =============================================================================
// boxed_clone independence
// =============================================================================

#[test]
fn boxed_clone_is_independent() {
    let mut c = HybridCompositor::new();
    let layer_id = c.active_layer().unwrap();
    let layer = c.layer_compositor_mut(layer_id).unwrap();
    let a = layer.add_tiled();

    let mut clone = c.boxed_clone();
    // Mutate the clone
    let clone_layer = clone.layer_compositor_mut(layer_id).unwrap();
    clone_layer.split_tiled(a, SplitDirection::Vertical);

    // Original should be unaffected
    assert_eq!(c.window_count(), 1);
    assert_eq!(clone.window_count(), 2);
}

#[test]
fn layer_compositor_read() {
    let mut c = HybridCompositor::new();
    let id = c.active_layer().unwrap();
    let layer = c.layer_compositor_mut(id).unwrap();
    layer.add_tiled();
    let read = c.layer_compositor(id).unwrap();
    assert!(read.focused().is_some());
}

#[test]
fn topology_empty_has_no_sentinel_windows() {
    let c = HybridCompositor::new();
    let topo = c.topology();
    assert_eq!(topo.focused, None);
    assert_eq!(topo.active_layer, Some(LayerId::new(0)));
    assert!(topo.tiled_trees.is_empty());
}

#[test]
fn topology_includes_active_layer_tree() {
    let mut c = HybridCompositor::new();
    let layer_id = c.active_layer().unwrap();
    let layer = c.layer_compositor_mut(layer_id).unwrap();
    let a = layer.add_tiled();
    let b = layer.split_tiled(a, SplitDirection::Vertical).unwrap();

    let topo = c.topology();
    assert_eq!(topo.active_layer, Some(layer_id));
    assert_eq!(topo.tiled_trees.len(), 1);
    assert_eq!(topo.tiled_trees[0].layer_id, layer_id);
    match &topo.tiled_trees[0].tree {
        TiledTree::Split { first, second, .. } => {
            assert!(matches!(first.as_ref(), TiledTree::Window(id) if *id == a));
            assert!(matches!(second.as_ref(), TiledTree::Window(id) if *id == b));
        }
        TiledTree::Window(_) => panic!("expected split tree"),
    }
}

#[test]
fn tiled_tree_returns_none_for_unknown_layer() {
    let c = HybridCompositor::new();
    assert!(c.tiled_tree(LayerId::new(99)).is_none());
}
