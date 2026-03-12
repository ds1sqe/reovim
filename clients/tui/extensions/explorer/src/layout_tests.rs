use super::*;

#[test]
fn basic_layout() {
    let bounds = SidebarBounds::calculate(30, 24, false);
    assert_eq!(bounds.x, 0);
    assert_eq!(bounds.y, 0);
    assert_eq!(bounds.width, 30);
    assert_eq!(bounds.height, 24);
    assert_eq!(bounds.header_y, 0);
    assert_eq!(bounds.tree_start_y, 1);
    assert_eq!(bounds.tree_height, 23);
    assert!(bounds.input_y.is_none());
}

#[test]
fn layout_with_input() {
    let bounds = SidebarBounds::calculate(30, 24, true);
    assert_eq!(bounds.tree_height, 22);
    assert_eq!(bounds.input_y, Some(23));
}

#[test]
fn layout_small_terminal() {
    let bounds = SidebarBounds::calculate(30, 3, false);
    assert_eq!(bounds.tree_height, 2);
    assert!(bounds.input_y.is_none());
}

#[test]
fn layout_small_terminal_with_input() {
    let bounds = SidebarBounds::calculate(30, 3, true);
    assert_eq!(bounds.tree_height, 1);
    assert_eq!(bounds.input_y, Some(2));
}

#[test]
fn layout_tiny_terminal_no_input() {
    let bounds = SidebarBounds::calculate(30, 1, false);
    assert_eq!(bounds.tree_height, 0);
    assert!(bounds.input_y.is_none());
}

#[test]
fn layout_tiny_terminal_with_input() {
    let bounds = SidebarBounds::calculate(30, 2, false);
    assert_eq!(bounds.tree_height, 1);
    assert!(bounds.input_y.is_none());
}

#[test]
fn layout_too_small_for_input() {
    // height <= 2 means no room for input row
    let bounds = SidebarBounds::calculate(30, 2, true);
    assert_eq!(bounds.tree_height, 1);
    assert!(bounds.input_y.is_none());
}
