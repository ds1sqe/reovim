use {
    super::*,
    reovim_subsys_layout::{LayerId, NavigateDirection, Rect, SplitDirection, TiledLayer},
};

fn zone() -> TiledZone {
    TiledZone::new(LayerId::new(0))
}

fn bounds() -> Rect {
    Rect::new(0, 0, 80, 24)
}

// =============================================================================
// Basic operations
// =============================================================================

#[test]
fn empty_zone() {
    let z = zone();
    assert!(z.is_empty());
    assert!(z.windows().is_empty());
    assert!(z.arrange(bounds()).is_empty());
}

#[test]
fn add_first_window() {
    let mut z = zone();
    let id = z.add_first();
    assert!(!z.is_empty());
    assert_eq!(z.windows(), vec![id]);
}

#[test]
fn single_window_fills_bounds() {
    let mut z = zone();
    z.add_first();
    let placements = z.arrange(bounds());
    assert_eq!(placements.len(), 1);
    assert_eq!(placements[0].bounds, bounds());
}

// =============================================================================
// Splitting
// =============================================================================

#[test]
fn vertical_split() {
    let mut z = zone();
    let a = z.add_first();
    let b = z.split(a, SplitDirection::Vertical).unwrap();
    assert_ne!(a, b);
    assert_eq!(z.windows().len(), 2);

    let placements = z.arrange(bounds());
    assert_eq!(placements.len(), 2);
    // Vertical split: side by side
    let pa = placements.iter().find(|p| p.window_id == a).unwrap();
    let pb = placements.iter().find(|p| p.window_id == b).unwrap();
    assert_eq!(pa.bounds.x, 0);
    assert_eq!(pa.bounds.width, 40);
    assert_eq!(pb.bounds.x, 40);
    assert_eq!(pb.bounds.width, 40);
}

#[test]
fn horizontal_split() {
    let mut z = zone();
    let a = z.add_first();
    let b = z.split(a, SplitDirection::Horizontal).unwrap();

    let placements = z.arrange(bounds());
    let pa = placements.iter().find(|p| p.window_id == a).unwrap();
    let pb = placements.iter().find(|p| p.window_id == b).unwrap();
    // Horizontal split: stacked vertically
    assert_eq!(pa.bounds.y, 0);
    assert_eq!(pa.bounds.height, 12);
    assert_eq!(pb.bounds.y, 12);
    assert_eq!(pb.bounds.height, 12);
}

#[test]
fn nested_split_three_windows() {
    let mut z = zone();
    let a = z.add_first();
    let b = z.split(a, SplitDirection::Vertical).unwrap();
    let c = z.split(b, SplitDirection::Horizontal).unwrap();
    assert_eq!(z.windows().len(), 3);

    let placements = z.arrange(bounds());
    assert_eq!(placements.len(), 3);
    // A takes left half, B+C share right half
    let pa = placements.iter().find(|p| p.window_id == a).unwrap();
    let pb = placements.iter().find(|p| p.window_id == b).unwrap();
    let pc = placements.iter().find(|p| p.window_id == c).unwrap();
    assert_eq!(pa.bounds.width, 40);
    assert_eq!(pb.bounds.x, 40);
    assert_eq!(pc.bounds.x, 40);
    assert!(pb.bounds.y < pc.bounds.y);
}

#[test]
fn split_nonexistent_returns_none() {
    let mut z = zone();
    z.add_first();
    let fake = WindowId::new();
    assert!(z.split(fake, SplitDirection::Vertical).is_none());
}

// =============================================================================
// Closing
// =============================================================================

#[test]
fn close_last_window_returns_none() {
    let mut z = zone();
    let a = z.add_first();
    assert!(z.close(a).is_none());
}

#[test]
fn close_one_of_two() {
    let mut z = zone();
    let a = z.add_first();
    let b = z.split(a, SplitDirection::Vertical).unwrap();
    let focus = z.close(b).unwrap();
    assert_eq!(focus, a);
    assert_eq!(z.windows(), vec![a]);
}

#[test]
fn close_first_of_two() {
    let mut z = zone();
    let a = z.add_first();
    let b = z.split(a, SplitDirection::Vertical).unwrap();
    let focus = z.close(a).unwrap();
    assert_eq!(focus, b);
    assert_eq!(z.windows(), vec![b]);
}

#[test]
fn close_in_nested_tree() {
    let mut z = zone();
    let a = z.add_first();
    let b = z.split(a, SplitDirection::Vertical).unwrap();
    let c = z.split(b, SplitDirection::Horizontal).unwrap();
    // Close b, c should remain
    let focus = z.close(b).unwrap();
    assert_eq!(focus, c);
    assert_eq!(z.windows().len(), 2);
    assert!(z.windows().contains(&a));
    assert!(z.windows().contains(&c));
}

// =============================================================================
// Navigation
// =============================================================================

#[test]
fn navigate_right() {
    let mut z = zone();
    let a = z.add_first();
    let b = z.split(a, SplitDirection::Vertical).unwrap();
    let views = z.arrange(bounds());
    let target = z.navigate(a, NavigateDirection::Right, &views);
    assert_eq!(target, Some(b));
}

#[test]
fn navigate_left() {
    let mut z = zone();
    let a = z.add_first();
    let b = z.split(a, SplitDirection::Vertical).unwrap();
    let views = z.arrange(bounds());
    let target = z.navigate(b, NavigateDirection::Left, &views);
    assert_eq!(target, Some(a));
}

#[test]
fn navigate_at_boundary_returns_none() {
    let mut z = zone();
    let a = z.add_first();
    z.split(a, SplitDirection::Vertical);
    let views = z.arrange(bounds());
    assert!(z.navigate(a, NavigateDirection::Left, &views).is_none());
}

#[test]
fn navigate_down() {
    let mut z = zone();
    let a = z.add_first();
    let b = z.split(a, SplitDirection::Horizontal).unwrap();
    let views = z.arrange(bounds());
    assert_eq!(z.navigate(a, NavigateDirection::Down, &views), Some(b));
}

// =============================================================================
// Equalize
// =============================================================================

#[test]
fn equalize_resets_ratios() {
    let mut z = zone();
    let a = z.add_first();
    z.split(a, SplitDirection::Vertical);
    // Resize to make unequal
    z.resize(a, NavigateDirection::Right, 10);
    z.equalize();
    // After equalize, both halves should be equal
    let placements = z.arrange(bounds());
    assert_eq!(placements[0].bounds.width, 40);
    assert_eq!(placements[1].bounds.width, 40);
}

// =============================================================================
// Cycle
// =============================================================================

#[test]
fn cycle_forward() {
    let mut z = zone();
    let a = z.add_first();
    let b = z.split(a, SplitDirection::Vertical).unwrap();
    let views = z.arrange(bounds());
    assert_eq!(z.cycle(a, true, &views), Some(b));
    assert_eq!(z.cycle(b, true, &views), Some(a)); // wraps
}

#[test]
fn cycle_backward() {
    let mut z = zone();
    let a = z.add_first();
    let b = z.split(a, SplitDirection::Vertical).unwrap();
    let views = z.arrange(bounds());
    assert_eq!(z.cycle(b, false, &views), Some(a));
}

#[test]
fn cycle_single_window_returns_none() {
    let mut z = zone();
    let a = z.add_first();
    let views = z.arrange(bounds());
    assert!(z.cycle(a, true, &views).is_none());
}

// =============================================================================
// Resize
// =============================================================================

#[test]
fn resize_changes_proportions() {
    let mut z = zone();
    let a = z.add_first();
    z.split(a, SplitDirection::Vertical);
    // Resize uses internal 1000-wide bounds; delta of 100 = 10% shift
    z.resize(a, NavigateDirection::Right, 100);
    let placements = z.arrange(bounds());
    let pa = placements.iter().find(|p| p.window_id == a).unwrap();
    // Ratio shifted from 0.5 → 0.6, so width ≈ 48 out of 80
    assert!(pa.bounds.width > 40, "Expected wider after resize, got {}", pa.bounds.width);
}

// =============================================================================
// Geometry helpers
// =============================================================================

#[test]
fn split_rect_vertical() {
    let r = Rect::new(0, 0, 80, 24);
    let (left, right) = super::split_rect(r, SplitDirection::Vertical, 0.5);
    assert_eq!(left.width, 40);
    assert_eq!(right.width, 40);
    assert_eq!(right.x, 40);
}

#[test]
fn split_rect_horizontal() {
    let r = Rect::new(0, 0, 80, 24);
    let (top, bottom) = super::split_rect(r, SplitDirection::Horizontal, 0.5);
    assert_eq!(top.height, 12);
    assert_eq!(bottom.height, 12);
    assert_eq!(bottom.y, 12);
}

#[test]
fn meets_minimum_size_check() {
    assert!(super::meets_minimum_size(Rect::new(0, 0, 10, 3)));
    assert!(!super::meets_minimum_size(Rect::new(0, 0, 9, 3)));
    assert!(!super::meets_minimum_size(Rect::new(0, 0, 10, 2)));
}

// =============================================================================
// Coverage: empty zone edge cases
// =============================================================================

#[test]
fn split_on_empty_zone_returns_none() {
    let mut z = zone();
    let fake = WindowId::new();
    assert!(z.split(fake, SplitDirection::Vertical).is_none());
}

#[test]
fn close_on_empty_zone_returns_none() {
    let mut z = zone();
    let fake = WindowId::new();
    assert!(z.close(fake).is_none());
}

#[test]
fn close_nonexistent_window_in_single_leaf() {
    let mut z = zone();
    z.add_first();
    let fake = WindowId::new();
    // Root is a leaf but doesn't match target -> None (line 283)
    assert!(z.close(fake).is_none());
}

#[test]
fn resize_on_empty_zone_is_noop() {
    let mut z = zone();
    let fake = WindowId::new();
    // Should not panic; the `if let Some(root)` is false
    z.resize(fake, NavigateDirection::Right, 10);
    assert!(z.is_empty());
}

#[test]
fn equalize_on_empty_zone_is_noop() {
    let mut z = zone();
    z.equalize();
    assert!(z.is_empty());
}

#[test]
fn equalize_on_single_leaf_is_noop() {
    let mut z = zone();
    let a = z.add_first();
    z.equalize();
    assert_eq!(z.windows(), vec![a]);
}

// =============================================================================
// Coverage: navigate edge cases
// =============================================================================

#[test]
fn navigate_from_nonexistent_window() {
    let mut z = zone();
    z.add_first();
    let views = z.arrange(bounds());
    let fake = WindowId::new();
    assert!(z.navigate(fake, NavigateDirection::Right, &views).is_none());
}

#[test]
fn navigate_up() {
    let mut z = zone();
    let a = z.add_first();
    let b = z.split(a, SplitDirection::Horizontal).unwrap();
    let views = z.arrange(bounds());
    assert_eq!(z.navigate(b, NavigateDirection::Up, &views), Some(a));
}

#[test]
fn navigate_up_at_boundary_returns_none() {
    let mut z = zone();
    let a = z.add_first();
    z.split(a, SplitDirection::Horizontal);
    let views = z.arrange(bounds());
    assert!(z.navigate(a, NavigateDirection::Up, &views).is_none());
}

#[test]
fn navigate_down_at_boundary_returns_none() {
    let mut z = zone();
    let a = z.add_first();
    let b = z.split(a, SplitDirection::Horizontal).unwrap();
    let views = z.arrange(bounds());
    assert!(z.navigate(b, NavigateDirection::Down, &views).is_none());
}

#[test]
fn navigate_right_at_boundary_returns_none() {
    let mut z = zone();
    let a = z.add_first();
    let b = z.split(a, SplitDirection::Vertical).unwrap();
    let views = z.arrange(bounds());
    assert!(z.navigate(b, NavigateDirection::Right, &views).is_none());
}

#[test]
fn navigate_picks_closest_among_multiple() {
    // Create 3 windows: A | B over C (B and C stacked on the right)
    let mut z = zone();
    let a = z.add_first();
    let b = z.split(a, SplitDirection::Vertical).unwrap();
    let c = z.split(b, SplitDirection::Horizontal).unwrap();
    let views = z.arrange(bounds());
    // Navigate right from A: should pick closest (B, since B is above C)
    let target = z.navigate(a, NavigateDirection::Right, &views).unwrap();
    // Both B and C are to the right; the closest by center distance should be chosen
    assert!(target == b || target == c);
}

#[test]
fn navigate_with_empty_views() {
    let z = zone();
    let fake = WindowId::new();
    assert!(z.navigate(fake, NavigateDirection::Left, &[]).is_none());
}

// =============================================================================
// Coverage: close edge cases
// =============================================================================

#[test]
fn close_nonexistent_in_split_tree() {
    let mut z = zone();
    let a = z.add_first();
    z.split(a, SplitDirection::Vertical);
    let fake = WindowId::new();
    // Target not found in either child of the split
    assert!(z.close(fake).is_none());
}

#[test]
fn close_deep_nested_second_child() {
    // A | (B / C), close C -> focus should be B
    let mut z = zone();
    let a = z.add_first();
    let b = z.split(a, SplitDirection::Vertical).unwrap();
    let c = z.split(b, SplitDirection::Horizontal).unwrap();
    let focus = z.close(c).unwrap();
    assert_eq!(focus, b);
    assert_eq!(z.windows().len(), 2);
    assert!(z.windows().contains(&a));
    assert!(z.windows().contains(&b));
}

#[test]
fn close_deep_nested_first_child_of_subtree() {
    // (A / B) | C — close A, focus should be B
    let mut z = zone();
    let root = z.add_first();
    let right = z.split(root, SplitDirection::Vertical).unwrap();
    let bottom_left = z.split(root, SplitDirection::Horizontal).unwrap();
    // Tree: (root / bottom_left) | right
    let focus = z.close(root).unwrap();
    assert_eq!(focus, bottom_left);
    assert_eq!(z.windows().len(), 2);
    assert!(z.windows().contains(&bottom_left));
    assert!(z.windows().contains(&right));
}

// =============================================================================
// Coverage: resize edge cases
// =============================================================================

#[test]
fn resize_single_leaf_is_noop() {
    let mut z = zone();
    let a = z.add_first();
    z.resize(a, NavigateDirection::Right, 10);
    // Single leaf, resize_at hits the else { return; } path
    assert_eq!(z.windows(), vec![a]);
}

#[test]
fn resize_horizontal_split_down() {
    // Horizontal split: first child (a) on top, second child (b) on bottom.
    // Ratio controls proportion given to first (a).
    let mut z = zone();
    let a = z.add_first();
    let _b = z.split(a, SplitDirection::Horizontal).unwrap();
    // Resize first child (a) downward: sign=+1, ratio increases -> a taller
    z.resize(a, NavigateDirection::Down, 100);
    let placements = z.arrange(bounds());
    let pa = placements.iter().find(|p| p.window_id == a).unwrap();
    assert!(
        pa.bounds.height > 12,
        "Expected taller after resize down, got {}",
        pa.bounds.height
    );

    // Resize second child (b) upward: sign=+1, ratio increases -> a gets MORE height
    let mut z2 = zone();
    let a2 = z2.add_first();
    let b2 = z2.split(a2, SplitDirection::Horizontal).unwrap();
    z2.resize(b2, NavigateDirection::Up, 100);
    let placements2 = z2.arrange(bounds());
    let pa2 = placements2.iter().find(|p| p.window_id == a2).unwrap();
    // Second child Up => sign=+1, ratio goes up => first (a2) gets MORE height
    assert!(
        pa2.bounds.height > 12,
        "Expected a2 taller after b2 resized up, got {}",
        pa2.bounds.height
    );
    // Also verify b2 got smaller
    let pb2 = placements2.iter().find(|p| p.window_id == b2).unwrap();
    assert!(
        pb2.bounds.height < 12,
        "Expected b2 shorter after resize up, got {}",
        pb2.bounds.height
    );
}

#[test]
fn resize_second_child_left() {
    // Vertical split: a (first/left), b (second/right). Ratio = proportion of first (a).
    // Second child Left: sign=+1, ratio increases => a gets MORE width.
    let mut z = zone();
    let a = z.add_first();
    let b = z.split(a, SplitDirection::Vertical).unwrap();
    z.resize(b, NavigateDirection::Left, 100);
    let placements = z.arrange(bounds());
    let pa = placements.iter().find(|p| p.window_id == a).unwrap();
    let pb = placements.iter().find(|p| p.window_id == b).unwrap();
    // ratio went from 0.5 to 0.6 -> a is wider
    assert!(
        pa.bounds.width > 40,
        "Expected a wider after b resized left, got {}",
        pa.bounds.width
    );
    assert!(
        pb.bounds.width < 40,
        "Expected b narrower after b resized left, got {}",
        pb.bounds.width
    );
}

#[test]
fn resize_first_child_left() {
    let mut z = zone();
    let a = z.add_first();
    z.split(a, SplitDirection::Vertical);
    // Resize first child (a) to the left -> shrinks a
    z.resize(a, NavigateDirection::Left, 100);
    let placements = z.arrange(bounds());
    let pa = placements.iter().find(|p| p.window_id == a).unwrap();
    assert!(
        pa.bounds.width < 40,
        "Expected a narrower after resize left, got {}",
        pa.bounds.width
    );
}

#[test]
fn resize_second_child_right() {
    // Vertical split: a (first/left), b (second/right). Ratio = proportion of first (a).
    // Second child Right: sign=-1, ratio decreases => a gets LESS width, b gets MORE.
    let mut z = zone();
    let a = z.add_first();
    let b = z.split(a, SplitDirection::Vertical).unwrap();
    z.resize(b, NavigateDirection::Right, 100);
    let placements = z.arrange(bounds());
    let pa = placements.iter().find(|p| p.window_id == a).unwrap();
    let pb = placements.iter().find(|p| p.window_id == b).unwrap();
    // ratio went from 0.5 to 0.4 -> a is narrower, b is wider
    assert!(
        pa.bounds.width < 40,
        "Expected a narrower after b resized right, got {}",
        pa.bounds.width
    );
    assert!(
        pb.bounds.width > 40,
        "Expected b wider after b resized right, got {}",
        pb.bounds.width
    );
}

#[test]
fn resize_axis_mismatch_recurses() {
    // Vertical split but resize with Up/Down -> axis mismatch, should recurse
    let mut z = zone();
    let a = z.add_first();
    let b = z.split(a, SplitDirection::Vertical).unwrap();
    // Split b horizontally to create a nested split
    let c = z.split(b, SplitDirection::Horizontal).unwrap();
    // Resize b with Down on the vertical split -> axis mismatch -> recurse into right child (b/c split)
    z.resize(b, NavigateDirection::Down, 100);
    let placements = z.arrange(bounds());
    let pb = placements.iter().find(|p| p.window_id == b).unwrap();
    let pc = placements.iter().find(|p| p.window_id == c).unwrap();
    // b should be taller, c shorter after resize down
    assert!(
        pb.bounds.height > pc.bounds.height,
        "Expected b taller than c after resize down"
    );
}

#[test]
fn resize_nonexistent_target_is_noop() {
    let mut z = zone();
    let a = z.add_first();
    z.split(a, SplitDirection::Vertical);
    let fake = WindowId::new();
    // Neither child contains fake — both branches of recurse path return
    z.resize(fake, NavigateDirection::Right, 10);
    // Should not panic; tree unchanged
    let placements = z.arrange(bounds());
    assert_eq!(placements[0].bounds.width, 40);
    assert_eq!(placements[1].bounds.width, 40);
}

#[test]
fn resize_clamps_to_min_ratio() {
    let mut z = zone();
    let a = z.add_first();
    z.split(a, SplitDirection::Vertical);
    // Shrink left window way past minimum
    z.resize(a, NavigateDirection::Left, 900);
    let placements = z.arrange(bounds());
    let pa = placements.iter().find(|p| p.window_id == a).unwrap();
    // Ratio clamped to 0.1, so width should be ~8 out of 80
    assert!(
        pa.bounds.width >= 8,
        "Width should be at least 0.1 * 80 = 8, got {}",
        pa.bounds.width
    );
    assert!(pa.bounds.width <= 10, "Width should not exceed ~10, got {}", pa.bounds.width);
}

#[test]
fn resize_clamps_to_max_ratio() {
    let mut z = zone();
    let a = z.add_first();
    z.split(a, SplitDirection::Vertical);
    // Grow left window way past maximum
    z.resize(a, NavigateDirection::Right, 900);
    let placements = z.arrange(bounds());
    let pa = placements.iter().find(|p| p.window_id == a).unwrap();
    // Ratio clamped to 0.9, so width should be ~72 out of 80
    assert!(pa.bounds.width >= 70, "Width should be at least ~70, got {}", pa.bounds.width);
    assert!(pa.bounds.width <= 72, "Width should not exceed ~72, got {}", pa.bounds.width);
}

#[test]
fn resize_horizontal_second_child_down() {
    // Horizontal split: a (first/top), b (second/bottom). Ratio = proportion of first (a).
    // Second child Down: sign=-1, ratio decreases => a gets LESS height, b gets MORE.
    let mut z = zone();
    let a = z.add_first();
    let b = z.split(a, SplitDirection::Horizontal).unwrap();
    z.resize(b, NavigateDirection::Down, 100);
    let placements = z.arrange(bounds());
    let pa = placements.iter().find(|p| p.window_id == a).unwrap();
    let pb = placements.iter().find(|p| p.window_id == b).unwrap();
    assert!(
        pa.bounds.height < 12,
        "Expected a shorter after b resized down, got {}",
        pa.bounds.height
    );
    assert!(
        pb.bounds.height > 12,
        "Expected b taller after b resized down, got {}",
        pb.bounds.height
    );
}

// =============================================================================
// Coverage: split edge cases
// =============================================================================

#[test]
fn split_too_small_bounds_returns_none() {
    // The split method uses 1000x1000 internal bounds, but split_at itself
    // checks meets_minimum_size. We test split_at directly via the stored tree.
    // Create a zone, then split_at on a tiny rect that would fail minimum size.
    let mut node = TiledTree::Window(WindowId::new());
    let target = match &node {
        TiledTree::Window(id) => *id,
        TiledTree::Split { .. } => unreachable!(),
    };
    // Tiny rect: 4x2 -> splitting vertically gives 2x2 each, below MIN_WINDOW_WIDTH=10
    let tiny = Rect::new(0, 0, 4, 2);
    assert!(
        node.split_at(target, SplitDirection::Vertical, tiny)
            .is_none()
    );
}

#[test]
fn split_nonexistent_in_nested_tree() {
    // When target is not in `first`, the split_at recurses into `second`
    let mut z = zone();
    let a = z.add_first();
    let b = z.split(a, SplitDirection::Vertical).unwrap();
    // Now split b further (this exercises the second.split_at path in the Split match arm)
    let c = z.split(b, SplitDirection::Horizontal).unwrap();
    assert_eq!(z.windows().len(), 3);
    assert!(z.windows().contains(&c));
}

// =============================================================================
// Coverage: cycle edge cases
// =============================================================================

#[test]
fn cycle_backward_wraps() {
    let mut z = zone();
    let a = z.add_first();
    let b = z.split(a, SplitDirection::Vertical).unwrap();
    let views = z.arrange(bounds());
    // a is first in sorted order (leftmost). Cycling backward should wrap to b.
    assert_eq!(z.cycle(a, false, &views), Some(b));
}

#[test]
fn cycle_nonexistent_window() {
    let mut z = zone();
    let a = z.add_first();
    z.split(a, SplitDirection::Vertical);
    let views = z.arrange(bounds());
    let fake = WindowId::new();
    assert!(z.cycle(fake, true, &views).is_none());
}

#[test]
fn cycle_three_windows_forward_and_backward() {
    let mut z = zone();
    let a = z.add_first();
    let b = z.split(a, SplitDirection::Vertical).unwrap();
    let c = z.split(b, SplitDirection::Horizontal).unwrap();
    let views = z.arrange(bounds());
    // Sorted by (y, x): a at (0,0), b at (0,40), c at (12,40) or similar
    // Forward from a
    let next = z.cycle(a, true, &views).unwrap();
    assert!(next == b || next == c);
    // Backward from a (wraps to last)
    let prev = z.cycle(a, false, &views).unwrap();
    assert!(prev == b || prev == c);
    assert_ne!(next, prev); // forward and backward from a should give different windows
}

// =============================================================================
// Coverage: rect_center
// =============================================================================

#[test]
fn rect_center_calculation() {
    let center = super::rect_center(Rect::new(10, 20, 30, 40));
    // center_x = 10.0 + 30.0/2.0 = 25.0
    // center_y = 20.0 + 40.0/2.0 = 40.0
    assert!((center.0 - 25.0).abs() < f32::EPSILON);
    assert!((center.1 - 40.0).abs() < f32::EPSILON);
}

// =============================================================================
// Coverage: contains
// =============================================================================

#[test]
fn contains_checks_both_children() {
    let mut z = zone();
    let a = z.add_first();
    let b = z.split(a, SplitDirection::Vertical).unwrap();
    // Extract the root node and test contains
    let root = z.root.as_ref().unwrap();
    assert!(root.contains(a));
    assert!(root.contains(b));
    let fake = WindowId::new();
    assert!(!root.contains(fake));
}

// =============================================================================
// Coverage: first_leaf through split
// =============================================================================

#[test]
fn first_leaf_of_split_tree() {
    let mut z = zone();
    let a = z.add_first();
    z.split(a, SplitDirection::Vertical);
    let root = z.root.as_ref().unwrap();
    // first_leaf should return the leftmost leaf (a)
    assert_eq!(root.first_leaf(), a);
}

// =============================================================================
// Coverage: collect_windows through split
// =============================================================================

#[test]
fn collect_windows_depth_first() {
    let mut z = zone();
    let a = z.add_first();
    let b = z.split(a, SplitDirection::Vertical).unwrap();
    let c = z.split(b, SplitDirection::Horizontal).unwrap();
    let windows = z.windows();
    assert_eq!(windows.len(), 3);
    // Depth-first: a, then b, then c
    assert_eq!(windows[0], a);
    assert_eq!(windows[1], b);
    assert_eq!(windows[2], c);
}

// =============================================================================
// Coverage: meets_minimum_size both-fail case
// =============================================================================

#[test]
fn meets_minimum_size_both_dimensions_fail() {
    assert!(!super::meets_minimum_size(Rect::new(0, 0, 1, 1)));
}

// =============================================================================
// Coverage: resize axis mismatch with no nested split (neither child has target)
// =============================================================================

#[test]
fn resize_axis_mismatch_no_nested_target() {
    // Vertical split, resize with Up (axis mismatch), target only in first child
    // This should recurse into `first.resize_at` which hits Leaf -> return
    let mut z = zone();
    let a = z.add_first();
    z.split(a, SplitDirection::Vertical);
    z.resize(a, NavigateDirection::Up, 10);
    // No change (leaf can't resize), verify no panic
    let placements = z.arrange(bounds());
    assert_eq!(placements[0].bounds.width, 40);
}

#[test]
fn resize_axis_mismatch_second_child() {
    // Vertical split, resize with Down (axis mismatch), target in second child
    let mut z = zone();
    let a = z.add_first();
    let b = z.split(a, SplitDirection::Vertical).unwrap();
    z.resize(b, NavigateDirection::Down, 10);
    // b is a leaf, resize_at returns, no change
    let placements = z.arrange(bounds());
    assert_eq!(placements[0].bounds.width, 40);
}

#[test]
fn resize_at_zero_width_bounds_is_noop() {
    // Test the `total > 0.0` false branch in resize_at:
    // axis matches, target found, but total dimension is zero.
    let id_a = WindowId::new();
    let id_b = WindowId::new();
    let mut node = TiledTree::Split {
        direction: SplitDirection::Vertical,
        ratio: Permil::HALF,
        first: Box::new(TiledTree::Window(id_a)),
        second: Box::new(TiledTree::Window(id_b)),
    };
    // Zero-width bounds: vertical split total = 0.0
    let zero_bounds = Rect::new(0, 0, 0, 10);
    node.resize_at(id_a, NavigateDirection::Right, 10, zero_bounds);
    // Ratio should remain unchanged at 0.5
    if let TiledTree::Split { ratio, .. } = &node {
        assert_eq!(*ratio, Permil::HALF);
    }
}

#[test]
fn resize_at_zero_height_bounds_is_noop() {
    // Same test for horizontal split with zero height.
    let id_a = WindowId::new();
    let id_b = WindowId::new();
    let mut node = TiledTree::Split {
        direction: SplitDirection::Horizontal,
        ratio: Permil::HALF,
        first: Box::new(TiledTree::Window(id_a)),
        second: Box::new(TiledTree::Window(id_b)),
    };
    let zero_bounds = Rect::new(0, 0, 10, 0);
    node.resize_at(id_a, NavigateDirection::Down, 10, zero_bounds);
    if let TiledTree::Split { ratio, .. } = &node {
        assert_eq!(*ratio, Permil::HALF);
    }
}
