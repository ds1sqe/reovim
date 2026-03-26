use {
    super::*,
    reovim_driver_layout::{LayerId, Rect, SplitDirection, TiledLayer, NavigateDirection},
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
