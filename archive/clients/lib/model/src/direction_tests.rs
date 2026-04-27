use super::*;

// Direction tests
#[test]
fn test_direction_opposite() {
    assert_eq!(Direction::Up.opposite(), Direction::Down);
    assert_eq!(Direction::Down.opposite(), Direction::Up);
    assert_eq!(Direction::Left.opposite(), Direction::Right);
    assert_eq!(Direction::Right.opposite(), Direction::Left);
}

#[test]
fn test_direction_is_horizontal() {
    assert!(Direction::Left.is_horizontal());
    assert!(Direction::Right.is_horizontal());
    assert!(!Direction::Up.is_horizontal());
    assert!(!Direction::Down.is_horizontal());
}

#[test]
fn test_direction_is_vertical() {
    assert!(Direction::Up.is_vertical());
    assert!(Direction::Down.is_vertical());
    assert!(!Direction::Left.is_vertical());
    assert!(!Direction::Right.is_vertical());
}

#[test]
fn test_direction_as_delta() {
    assert_eq!(Direction::Up.as_delta(), (0, -1));
    assert_eq!(Direction::Down.as_delta(), (0, 1));
    assert_eq!(Direction::Left.as_delta(), (-1, 0));
    assert_eq!(Direction::Right.as_delta(), (1, 0));
}

// SplitDirection tests
#[test]
fn test_split_direction_opposite() {
    assert_eq!(SplitDirection::Horizontal.opposite(), SplitDirection::Vertical);
    assert_eq!(SplitDirection::Vertical.opposite(), SplitDirection::Horizontal);
}

#[test]
fn test_split_direction_is_along() {
    // Horizontal split (stacked vertically) - Up/Down navigate between children
    assert!(SplitDirection::Horizontal.is_along(Direction::Up));
    assert!(SplitDirection::Horizontal.is_along(Direction::Down));
    assert!(!SplitDirection::Horizontal.is_along(Direction::Left));
    assert!(!SplitDirection::Horizontal.is_along(Direction::Right));

    // Vertical split (side by side) - Left/Right navigate between children
    assert!(SplitDirection::Vertical.is_along(Direction::Left));
    assert!(SplitDirection::Vertical.is_along(Direction::Right));
    assert!(!SplitDirection::Vertical.is_along(Direction::Up));
    assert!(!SplitDirection::Vertical.is_along(Direction::Down));
}
