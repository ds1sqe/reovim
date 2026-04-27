use super::*;

// ---------------------------------------------------------------------------
// Permil::new — valid boundary values
// ---------------------------------------------------------------------------

#[test]
fn permil_new_zero() {
    let p = Permil::new(0).expect("0 is valid");
    assert_eq!(p.value(), 0);
}

#[test]
fn permil_new_half() {
    let p = Permil::new(500).expect("500 is valid");
    assert_eq!(p.value(), 500);
}

#[test]
fn permil_new_full() {
    let p = Permil::new(1000).expect("1000 is valid");
    assert_eq!(p.value(), 1000);
}

// ---------------------------------------------------------------------------
// Permil::new — invalid values
// ---------------------------------------------------------------------------

#[test]
fn permil_new_invalid_1001() {
    assert_eq!(Permil::new(1001), Err(SplitError::InvalidRatio(1001)));
}

#[test]
fn permil_new_invalid_max() {
    assert_eq!(Permil::new(u16::MAX), Err(SplitError::InvalidRatio(u16::MAX)));
}

// ---------------------------------------------------------------------------
// Permil constants
// ---------------------------------------------------------------------------

#[test]
fn permil_full_constant() {
    assert_eq!(Permil::FULL.value(), 1000);
}

#[test]
fn permil_half_constant() {
    assert_eq!(Permil::HALF.value(), 500);
}

// ---------------------------------------------------------------------------
// TiledTree::Window
// ---------------------------------------------------------------------------

#[test]
fn tiled_tree_window() {
    let id = WindowId::from_raw(1);
    let topo = TiledTree::Window(id);
    let TiledTree::Window(got) = topo else {
        panic!("expected Window variant");
    };
    assert_eq!(got, id);
}

// ---------------------------------------------------------------------------
// TiledTree::Split nested
// ---------------------------------------------------------------------------

#[test]
fn tiled_tree_split_nested() {
    use crate::SplitDirection;

    let id_a = WindowId::from_raw(101);
    let id_b = WindowId::from_raw(102);
    let id_c = WindowId::from_raw(103);

    // Build: Vertical split with A on the left and (Horizontal B|C) on the right.
    let inner = TiledTree::Split {
        direction: SplitDirection::Horizontal,
        ratio: Permil::HALF,
        first: Box::new(TiledTree::Window(id_b)),
        second: Box::new(TiledTree::Window(id_c)),
    };

    let outer = TiledTree::Split {
        direction: SplitDirection::Vertical,
        ratio: Permil::new(400).expect("valid"),
        first: Box::new(TiledTree::Window(id_a)),
        second: Box::new(inner),
    };

    // Verify outer variant and ratio.
    let TiledTree::Split {
        direction,
        ratio,
        first,
        second,
    } = outer
    else {
        panic!("expected Split variant");
    };

    assert_eq!(direction, SplitDirection::Vertical);
    assert_eq!(ratio.value(), 400);

    // Verify first leaf.
    let TiledTree::Window(got_a) = *first else {
        panic!("expected Window in first");
    };
    assert_eq!(got_a, id_a);

    // Verify inner split.
    let TiledTree::Split {
        direction: inner_dir,
        first: inner_first,
        second: inner_second,
        ..
    } = *second
    else {
        panic!("expected nested Split in second");
    };
    assert_eq!(inner_dir, SplitDirection::Horizontal);

    let TiledTree::Window(got_b) = *inner_first else {
        panic!("expected Window b");
    };
    let TiledTree::Window(got_c) = *inner_second else {
        panic!("expected Window c");
    };
    assert_eq!(got_b, id_b);
    assert_eq!(got_c, id_c);
}

#[test]
fn layer_tree_holds_layer_and_tree() {
    let layer_id = LayerId::new(7);
    let window_id = WindowId::from_raw(2);
    let tree = TiledTree::Window(window_id);
    let layer_tree = LayerTree { layer_id, tree };
    assert_eq!(layer_tree.layer_id, layer_id);
    match layer_tree.tree {
        TiledTree::Window(got) => assert_eq!(got, window_id),
        TiledTree::Split { .. } => panic!("expected Window variant"),
    }
}

#[test]
fn layout_topology_struct_fields() {
    let focused = Some(WindowId::from_raw(10));
    let active_layer = Some(LayerId::new(3));
    let tiled_trees = vec![LayerTree {
        layer_id: LayerId::new(3),
        tree: TiledTree::Window(WindowId::from_raw(10)),
    }];
    let overlay_anchors = vec![(WindowId::from_raw(20), crate::Anchor::Center)];

    let topology = LayoutTopology {
        focused,
        active_layer,
        tiled_trees: tiled_trees.clone(),
        overlay_anchors: overlay_anchors.clone(),
    };

    assert_eq!(topology.focused, focused);
    assert_eq!(topology.active_layer, active_layer);
    assert_eq!(topology.tiled_trees, tiled_trees);
    assert_eq!(topology.overlay_anchors, overlay_anchors);
}

#[test]
fn layout_topology_default_is_empty() {
    let topology = LayoutTopology::default();
    assert_eq!(topology.focused, None);
    assert_eq!(topology.active_layer, None);
    assert!(topology.tiled_trees.is_empty());
    assert!(topology.overlay_anchors.is_empty());
}

// ---------------------------------------------------------------------------
// SplitError Display
// ---------------------------------------------------------------------------

#[test]
fn split_error_display_invalid_ratio() {
    let msg = SplitError::InvalidRatio(1234).to_string();
    assert!(msg.contains("1234"), "message was: {msg}");
    assert!(msg.contains("1000"), "message was: {msg}");
}

#[test]
fn split_error_display_too_small() {
    let msg = SplitError::TooSmall.to_string();
    assert!(!msg.is_empty());
}
