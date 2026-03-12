use super::*;

fn test_window(id: u64, x: u16, y: u16, w: u16, h: u16) -> Window {
    Window::new(id, id, id, Rect::new(x, y, w, h))
}

#[test]
fn test_window_new() {
    let window = test_window(1, 0, 0, 80, 24);
    assert_eq!(window.id, 1);
    assert_eq!(window.viewport_id, 1);
    assert_eq!(window.buffer_id, 1);
    assert!(!window.focused);
}

#[test]
fn test_window_with_focus() {
    let window = test_window(1, 0, 0, 80, 24).with_focus(true);
    assert!(window.focused);
}

#[test]
fn test_window_area() {
    let window = test_window(1, 0, 0, 80, 24);
    assert_eq!(window.area(), 1920);
}

#[test]
fn test_window_tree_leaf() {
    let tree = WindowTree::leaf(test_window(1, 0, 0, 80, 24));
    assert!(tree.is_leaf());
    assert_eq!(tree.window_count(), 1);
}

#[test]
fn test_window_tree_split() {
    let tree = WindowTree::split(
        SplitDirection::Vertical,
        vec![
            WindowTree::leaf(test_window(1, 0, 0, 40, 24)),
            WindowTree::leaf(test_window(2, 40, 0, 40, 24)),
        ],
        Rect::new(0, 0, 80, 24),
    );
    assert!(!tree.is_leaf());
    assert_eq!(tree.window_count(), 2);
}

#[test]
fn test_window_tree_bounds() {
    let tree = WindowTree::split(
        SplitDirection::Vertical,
        vec![
            WindowTree::leaf(test_window(1, 0, 0, 40, 24)),
            WindowTree::leaf(test_window(2, 40, 0, 40, 24)),
        ],
        Rect::new(0, 0, 80, 24),
    );
    assert_eq!(tree.bounds(), Rect::new(0, 0, 80, 24));
}

#[test]
fn test_window_tree_focused() {
    let tree = WindowTree::split(
        SplitDirection::Vertical,
        vec![
            WindowTree::leaf(test_window(1, 0, 0, 40, 24)),
            WindowTree::leaf(test_window(2, 40, 0, 40, 24).with_focus(true)),
        ],
        Rect::new(0, 0, 80, 24),
    );

    let focused = tree.focused_window().unwrap();
    assert_eq!(focused.id, 2);
}

#[test]
fn test_window_tree_find_by_viewport() {
    let tree = WindowTree::split(
        SplitDirection::Horizontal,
        vec![
            WindowTree::leaf(Window::new(1, 100, 1, Rect::new(0, 0, 80, 12))),
            WindowTree::leaf(Window::new(2, 200, 2, Rect::new(0, 12, 80, 12))),
        ],
        Rect::new(0, 0, 80, 24),
    );

    let found = tree.find_by_viewport(200).unwrap();
    assert_eq!(found.id, 2);

    assert!(tree.find_by_viewport(999).is_none());
}

#[test]
fn test_window_tree_all_windows() {
    let tree = WindowTree::split(
        SplitDirection::Vertical,
        vec![
            WindowTree::split(
                SplitDirection::Horizontal,
                vec![
                    WindowTree::leaf(test_window(1, 0, 0, 40, 12)),
                    WindowTree::leaf(test_window(2, 0, 12, 40, 12)),
                ],
                Rect::new(0, 0, 40, 24),
            ),
            WindowTree::leaf(test_window(3, 40, 0, 40, 24)),
        ],
        Rect::new(0, 0, 80, 24),
    );

    let windows = tree.all_windows();
    assert_eq!(windows.len(), 3);
}

#[test]
fn test_window_tree_focused_none() {
    // No focused window
    let tree = WindowTree::leaf(test_window(1, 0, 0, 80, 24));
    assert!(tree.focused_window().is_none());
}

#[test]
fn test_window_tree_tabs_empty() {
    let tree = WindowTree::tabs(Vec::new(), 0, Rect::new(0, 0, 80, 24));
    assert!(tree.focused_window().is_none());
    assert_eq!(tree.window_count(), 0);
}

#[test]
fn test_window_tree_tabs_out_of_bounds_active() {
    // Active index beyond tabs length
    let tree = WindowTree::tabs(
        vec![WindowTree::leaf(
            test_window(1, 0, 0, 80, 24).with_focus(true),
        )],
        5, // out of bounds
        Rect::new(0, 0, 80, 24),
    );
    // focused_window should return None since tabs.get(5) returns None
    assert!(tree.focused_window().is_none());
}

#[test]
fn test_window_tree_find_by_viewport_tabs() {
    let tree = WindowTree::tabs(
        vec![
            WindowTree::leaf(Window::new(1, 100, 1, Rect::new(0, 0, 80, 24))),
            WindowTree::leaf(Window::new(2, 200, 2, Rect::new(0, 0, 80, 24))),
        ],
        0,
        Rect::new(0, 0, 80, 24),
    );
    assert!(tree.find_by_viewport(200).is_some());
    assert!(tree.find_by_viewport(999).is_none());
}

#[test]
fn test_window_tree_all_windows_tabs() {
    let tree = WindowTree::tabs(
        vec![
            WindowTree::leaf(test_window(1, 0, 0, 80, 24)),
            WindowTree::leaf(test_window(2, 0, 0, 80, 24)),
        ],
        0,
        Rect::new(0, 0, 80, 24),
    );
    assert_eq!(tree.all_windows().len(), 2);
}

#[test]
fn test_window_tree_tabs_bounds() {
    let bounds = Rect::new(0, 0, 80, 24);
    let tree =
        WindowTree::tabs(vec![WindowTree::leaf(test_window(1, 0, 0, 80, 24))], 0, bounds);
    assert_eq!(tree.bounds(), bounds);
}

#[test]
fn test_window_tree_tabs() {
    let tree = WindowTree::tabs(
        vec![
            WindowTree::leaf(test_window(1, 0, 0, 80, 24).with_focus(true)),
            WindowTree::leaf(test_window(2, 0, 0, 80, 24)),
        ],
        0,
        Rect::new(0, 0, 80, 24),
    );

    assert_eq!(tree.window_count(), 2);

    // Focused window is in the active tab
    let focused = tree.focused_window().unwrap();
    assert_eq!(focused.id, 1);
}
