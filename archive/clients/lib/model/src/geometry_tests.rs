use super::*;

// ScreenPosition tests
#[test]
fn test_screen_position_new() {
    let pos = ScreenPosition::new(10, 20);
    assert_eq!(pos.x, 10);
    assert_eq!(pos.y, 20);
}

#[test]
fn test_screen_position_default() {
    let pos = ScreenPosition::default();
    assert_eq!(pos, ScreenPosition::new(0, 0));
}

#[test]
fn test_screen_position_offset_positive() {
    let pos = ScreenPosition::new(10, 20);
    let offset = pos.offset(5, 10);
    assert_eq!(offset, ScreenPosition::new(15, 30));
}

#[test]
fn test_screen_position_offset_negative_clamped() {
    let pos = ScreenPosition::new(5, 5);
    let offset = pos.offset(-10, -10);
    assert_eq!(offset, ScreenPosition::new(0, 0));
}

// Size tests
#[test]
fn test_size_new() {
    let size = Size::new(80, 24);
    assert_eq!(size.width, 80);
    assert_eq!(size.height, 24);
}

#[test]
fn test_size_area() {
    let size = Size::new(80, 24);
    assert_eq!(size.area(), 1920);
}

#[test]
fn test_size_is_empty() {
    assert!(Size::new(0, 10).is_empty());
    assert!(Size::new(10, 0).is_empty());
    assert!(!Size::new(10, 10).is_empty());
}

#[test]
fn test_size_default() {
    let size = Size::default();
    assert!(size.is_empty());
}

// Rect tests
#[test]
fn test_rect_new() {
    let rect = Rect::new(10, 20, 30, 40);
    assert_eq!(rect.x, 10);
    assert_eq!(rect.y, 20);
    assert_eq!(rect.width, 30);
    assert_eq!(rect.height, 40);
}

#[test]
fn test_rect_from_position_size() {
    let pos = ScreenPosition::new(10, 20);
    let size = Size::new(30, 40);
    let rect = Rect::from_position_size(pos, size);
    assert_eq!(rect, Rect::new(10, 20, 30, 40));
}

#[test]
fn test_rect_position_and_size() {
    let rect = Rect::new(10, 20, 30, 40);
    assert_eq!(rect.position(), ScreenPosition::new(10, 20));
    assert_eq!(rect.size(), Size::new(30, 40));
}

#[test]
fn test_rect_right_and_bottom() {
    let rect = Rect::new(10, 20, 30, 40);
    assert_eq!(rect.right(), 40);
    assert_eq!(rect.bottom(), 60);
}

#[test]
fn test_rect_contains_inside() {
    let rect = Rect::new(10, 10, 20, 20);
    assert!(rect.contains(ScreenPosition::new(15, 15)));
    assert!(rect.contains(ScreenPosition::new(10, 10))); // Top-left corner
}

#[test]
fn test_rect_contains_outside() {
    let rect = Rect::new(10, 10, 20, 20);
    assert!(!rect.contains(ScreenPosition::new(5, 15))); // Left of rect
    assert!(!rect.contains(ScreenPosition::new(35, 15))); // Right of rect
    assert!(!rect.contains(ScreenPosition::new(15, 5))); // Above rect
    assert!(!rect.contains(ScreenPosition::new(15, 35))); // Below rect
}

#[test]
fn test_rect_contains_on_boundary() {
    let rect = Rect::new(10, 10, 20, 20);
    // Right and bottom edges are exclusive
    assert!(!rect.contains(ScreenPosition::new(30, 15))); // Right edge
    assert!(!rect.contains(ScreenPosition::new(15, 30))); // Bottom edge
}

#[test]
fn test_rect_contains_xy() {
    let rect = Rect::new(10, 10, 20, 20);
    // Inside
    assert!(rect.contains_xy(15, 15));
    assert!(rect.contains_xy(10, 10)); // Top-left corner
    // Outside
    assert!(!rect.contains_xy(5, 15)); // Left
    assert!(!rect.contains_xy(30, 15)); // Right edge (exclusive)
    assert!(!rect.contains_xy(15, 30)); // Bottom edge (exclusive)
}

#[test]
fn test_rect_intersects_overlapping() {
    let rect1 = Rect::new(0, 0, 20, 20);
    let rect2 = Rect::new(10, 10, 20, 20);
    assert!(rect1.intersects(&rect2));
    assert!(rect2.intersects(&rect1));
}

#[test]
fn test_rect_intersects_non_overlapping() {
    let rect1 = Rect::new(0, 0, 10, 10);
    let rect2 = Rect::new(20, 20, 10, 10);
    assert!(!rect1.intersects(&rect2));
    assert!(!rect2.intersects(&rect1));
}

#[test]
fn test_rect_intersects_touching_edges() {
    // Touching edges don't count as intersection
    let rect1 = Rect::new(0, 0, 10, 10);
    let rect2 = Rect::new(10, 0, 10, 10);
    assert!(!rect1.intersects(&rect2));
}

#[test]
fn test_rect_intersection_overlapping() {
    let rect1 = Rect::new(0, 0, 20, 20);
    let rect2 = Rect::new(10, 10, 20, 20);
    let intersection = rect1.intersection(&rect2);
    assert_eq!(intersection, Some(Rect::new(10, 10, 10, 10)));
}

#[test]
fn test_rect_intersection_non_overlapping() {
    let rect1 = Rect::new(0, 0, 10, 10);
    let rect2 = Rect::new(20, 20, 10, 10);
    assert_eq!(rect1.intersection(&rect2), None);
}

#[test]
fn test_rect_is_empty() {
    assert!(Rect::new(10, 10, 0, 10).is_empty());
    assert!(Rect::new(10, 10, 10, 0).is_empty());
    assert!(!Rect::new(10, 10, 10, 10).is_empty());
}

#[test]
fn test_rect_area() {
    let rect = Rect::new(0, 0, 30, 40);
    assert_eq!(rect.area(), 1200);
}
