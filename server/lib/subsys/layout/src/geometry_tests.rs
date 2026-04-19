use super::*;

#[test]
fn size_new() {
    let size = Size::new(80, 24);
    assert_eq!(size.width, 80);
    assert_eq!(size.height, 24);
}

#[test]
fn size_area() {
    let size = Size::new(80, 24);
    assert_eq!(size.area(), 1920);
}

#[test]
fn size_is_empty() {
    assert!(Size::new(0, 10).is_empty());
    assert!(Size::new(10, 0).is_empty());
    assert!(!Size::new(10, 10).is_empty());
}

#[test]
fn size_default() {
    let size = Size::default();
    assert!(size.is_empty());
}

#[test]
fn rect_new() {
    let rect = Rect::new(10, 20, 30, 40);
    assert_eq!(rect.x, 10);
    assert_eq!(rect.y, 20);
    assert_eq!(rect.width, 30);
    assert_eq!(rect.height, 40);
}

#[test]
fn rect_right_and_bottom() {
    let rect = Rect::new(10, 20, 30, 40);
    assert_eq!(rect.right(), 40);
    assert_eq!(rect.bottom(), 60);
}

#[test]
fn rect_contains_xy() {
    let rect = Rect::new(10, 10, 20, 20);
    assert!(rect.contains_xy(15, 15));
    assert!(rect.contains_xy(10, 10));
    assert!(!rect.contains_xy(5, 15));
    assert!(!rect.contains_xy(30, 15));
    assert!(!rect.contains_xy(15, 30));
}

#[test]
fn rect_intersects_overlapping() {
    let rect1 = Rect::new(0, 0, 20, 20);
    let rect2 = Rect::new(10, 10, 20, 20);
    assert!(rect1.intersects(&rect2));
    assert!(rect2.intersects(&rect1));
}

#[test]
fn rect_intersects_non_overlapping() {
    let rect1 = Rect::new(0, 0, 10, 10);
    let rect2 = Rect::new(20, 20, 10, 10);
    assert!(!rect1.intersects(&rect2));
    assert!(!rect2.intersects(&rect1));
}

#[test]
fn rect_intersects_touching_edges() {
    let rect1 = Rect::new(0, 0, 10, 10);
    let rect2 = Rect::new(10, 0, 10, 10);
    assert!(!rect1.intersects(&rect2));
}

#[test]
fn rect_intersection_overlapping() {
    let rect1 = Rect::new(0, 0, 20, 20);
    let rect2 = Rect::new(10, 10, 20, 20);
    assert_eq!(rect1.intersection(&rect2), Some(Rect::new(10, 10, 10, 10)));
}

#[test]
fn rect_intersection_non_overlapping() {
    let rect1 = Rect::new(0, 0, 10, 10);
    let rect2 = Rect::new(20, 20, 10, 10);
    assert_eq!(rect1.intersection(&rect2), None);
}

#[test]
fn rect_is_empty() {
    assert!(Rect::new(10, 10, 0, 10).is_empty());
    assert!(Rect::new(10, 10, 10, 0).is_empty());
    assert!(!Rect::new(10, 10, 10, 10).is_empty());
}

#[test]
fn rect_area() {
    let rect = Rect::new(0, 0, 30, 40);
    assert_eq!(rect.area(), 1200);
}
