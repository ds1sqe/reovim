use super::*;

#[test]
fn test_terminal_size_is_valid() {
    assert!(Size::new(80, 24).is_valid());
    assert!(Size::new(1, 1).is_valid());

    assert!(!Size::new(0, 24).is_valid());
    assert!(!Size::new(80, 0).is_valid());
    assert!(!Size::new(0, 0).is_valid());
}

#[test]
fn test_rect_contains_xy() {
    let rect = Rect::new(10, 20, 30, 40);

    // Inside
    assert!(rect.contains_xy(10, 20)); // Top-left corner
    assert!(rect.contains_xy(25, 40)); // Middle
    assert!(rect.contains_xy(39, 59)); // Bottom-right corner (exclusive bounds)

    // Outside
    assert!(!rect.contains_xy(9, 20)); // Left of rect
    assert!(!rect.contains_xy(10, 19)); // Above rect
    assert!(!rect.contains_xy(40, 20)); // Right of rect (at boundary)
    assert!(!rect.contains_xy(10, 60)); // Below rect (at boundary)
}

#[test]
fn test_direction_variants() {
    // Verify direction variants exist and are distinct
    assert_ne!(Direction::Up, Direction::Down);
    assert_ne!(Direction::Left, Direction::Right);
    assert_ne!(Direction::Up, Direction::Left);

    // Verify NavigateDirection alias works
    let nav: NavigateDirection = Direction::Left;
    assert_eq!(nav, Direction::Left);
}

#[test]
fn test_split_direction_variants() {
    assert_ne!(SplitDirection::Horizontal, SplitDirection::Vertical);
}

#[test]
fn test_size_alias() {
    // TerminalSize is an alias for Size
    let ts: TerminalSize = Size::new(80, 24);
    assert_eq!(ts.width, 80);
    assert_eq!(ts.height, 24);
}
