use super::*;

#[test]
fn test_bounds_new() {
    let bounds = Bounds::new(10, 20, 30, 40);
    assert_eq!(bounds.x, 10);
    assert_eq!(bounds.y, 20);
    assert_eq!(bounds.width, 30);
    assert_eq!(bounds.height, 40);
}

#[test]
fn test_bounds_full_screen() {
    let bounds = Bounds::full_screen(80, 24);
    assert_eq!(bounds.x, 0);
    assert_eq!(bounds.y, 0);
    assert_eq!(bounds.width, 80);
    assert_eq!(bounds.height, 24);
}

#[test]
fn test_bounds_contains_corners() {
    let bounds = Bounds::new(10, 20, 30, 40);
    // Top-left (inclusive)
    assert!(bounds.contains(10, 20));
    // Just inside bottom-right
    assert!(bounds.contains(39, 59));
    // Bottom-right (exclusive)
    assert!(!bounds.contains(40, 60));
}

#[test]
fn test_bounds_contains_edges() {
    let bounds = Bounds::new(10, 20, 30, 40);
    // Left edge (just outside)
    assert!(!bounds.contains(9, 30));
    // Right edge (just outside)
    assert!(!bounds.contains(40, 30));
    // Top edge (just outside)
    assert!(!bounds.contains(20, 19));
    // Bottom edge (just outside)
    assert!(!bounds.contains(20, 60));
}

#[test]
fn test_bounds_overlaps() {
    let a = Bounds::new(0, 0, 50, 50);
    let b = Bounds::new(25, 25, 50, 50);
    let c = Bounds::new(100, 100, 10, 10);
    assert!(a.overlaps(&b));
    assert!(b.overlaps(&a)); // Symmetric
    assert!(!a.overlaps(&c));
    assert!(!c.overlaps(&a));
}

#[test]
fn test_bounds_overlaps_adjacent() {
    // Adjacent bounds should NOT overlap
    let a = Bounds::new(0, 0, 10, 10);
    let b = Bounds::new(10, 0, 10, 10); // Right adjacent
    assert!(!a.overlaps(&b));
}

#[test]
fn test_bounds_overlaps_vertical_non_overlap() {
    // Horizontally overlapping but vertically non-overlapping (lines 68, 69 false)
    let a = Bounds::new(0, 0, 10, 5); // y: 0..5
    let b = Bounds::new(0, 10, 10, 5); // y: 10..15
    // x ranges overlap (both 0..10), but y ranges don't
    assert!(!a.overlaps(&b));
    assert!(!b.overlaps(&a));

    // Vertically adjacent (a bottom == b top)
    let c = Bounds::new(0, 0, 10, 10); // y: 0..10
    let d = Bounds::new(0, 10, 10, 10); // y: 10..20
    assert!(!c.overlaps(&d));
}

#[test]
fn test_bounds_is_empty() {
    assert!(Bounds::new(0, 0, 0, 10).is_empty());
    assert!(Bounds::new(0, 0, 10, 0).is_empty());
    assert!(!Bounds::new(0, 0, 10, 10).is_empty());
}

#[test]
fn test_bounds_right_bottom() {
    let bounds = Bounds::new(10, 20, 30, 40);
    assert_eq!(bounds.right(), 40);
    assert_eq!(bounds.bottom(), 60);
}
