use super::*;

#[test]
fn test_interpret_layout_single() {
    let logical = LogicalLayout::single(1, 100);
    let tree = interpret_layout(logical, 80, 24);
    assert!(tree.is_leaf());
    assert_eq!(tree.bounds(), Rect::new(0, 0, 80, 24));
}

#[test]
fn test_interpret_layout_vsplit() {
    let logical =
        LogicalLayout::vsplit(vec![LogicalLayout::single(1, 100), LogicalLayout::single(2, 101)]);
    let tree = interpret_layout(logical, 80, 24);
    assert_eq!(tree.window_count(), 2);
}

#[test]
fn test_rect_contains() {
    let rect = Rect::new(10, 10, 20, 20);
    assert!(rect_contains(rect, 15, 15));
    assert!(!rect_contains(rect, 5, 5));
    assert!(!rect_contains(rect, 30, 30));
}

#[test]
fn test_direction_opposite() {
    assert_eq!(direction_opposite(Direction::Up), Direction::Down);
    assert_eq!(direction_opposite(Direction::Left), Direction::Right);
}

#[test]
fn test_direction_checks() {
    assert!(direction_is_horizontal(Direction::Left));
    assert!(direction_is_horizontal(Direction::Right));
    assert!(direction_is_vertical(Direction::Up));
    assert!(direction_is_vertical(Direction::Down));
}

#[test]
fn test_split_direction() {
    assert!(split_is_horizontal(SplitDirection::Horizontal));
    assert!(split_is_vertical(SplitDirection::Vertical));
    assert_eq!(split_opposite(SplitDirection::Horizontal), SplitDirection::Vertical);
}

#[test]
fn test_layout_helpers() {
    let single = layout_single(1, 100);
    assert!(layout_is_leaf(&single));
    assert_eq!(layout_window_count(&single), 1);

    let split = layout_hsplit(vec![single.clone(), single]);
    assert!(!layout_is_leaf(&split));
    assert_eq!(layout_window_count(&split), 2);
}
