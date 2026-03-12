use super::*;

#[test]
fn test_window_view_new() {
    let view = WindowView::new(WindowId::from_raw(1), Rect::new(10, 20, 30, 40));
    assert_eq!(view.window_id, WindowId::from_raw(1));
    assert_eq!(view.bounds.x, 10);
    assert_eq!(view.bounds.y, 20);
}

#[test]
fn test_window_view_contains() {
    let view = WindowView::new(WindowId::from_raw(1), Rect::new(10, 10, 20, 20));
    assert!(view.contains(15, 15));
    assert!(!view.contains(5, 15));
}

#[test]
fn test_single_window_layout_empty() {
    let layout = SingleWindowLayout;
    let views = layout.arrange((80, 24), &[]);
    assert!(views.is_empty());
}

#[test]
fn test_single_window_layout_one() {
    let layout = SingleWindowLayout;
    let views = layout.arrange((80, 24), &[WindowId::from_raw(1)]);
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].window_id, WindowId::from_raw(1));
    assert_eq!(views[0].bounds, Rect::new(0, 0, 80, 24));
}

#[test]
fn test_single_window_layout_multiple() {
    let layout = SingleWindowLayout;
    let views = layout.arrange((80, 24), &[WindowId::from_raw(1), WindowId::from_raw(2)]);
    // Only the first window is shown
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].window_id, WindowId::from_raw(1));
}

#[test]
fn test_default_focus_left() {
    let policy = DefaultFocusPolicy;
    let views = vec![
        WindowView::new(WindowId::from_raw(1), Rect::new(0, 0, 40, 24)),
        WindowView::new(WindowId::from_raw(2), Rect::new(40, 0, 40, 24)),
    ];

    // From right window, go left
    let next = policy.next(NavigateDirection::Left, WindowId::from_raw(2), &views);
    assert_eq!(next, Some(WindowId::from_raw(1)));

    // From left window, go left (nothing there)
    let next = policy.next(NavigateDirection::Left, WindowId::from_raw(1), &views);
    assert_eq!(next, None);
}

#[test]
fn test_default_focus_right() {
    let policy = DefaultFocusPolicy;
    let views = vec![
        WindowView::new(WindowId::from_raw(1), Rect::new(0, 0, 40, 24)),
        WindowView::new(WindowId::from_raw(2), Rect::new(40, 0, 40, 24)),
    ];

    // From left window, go right
    let next = policy.next(NavigateDirection::Right, WindowId::from_raw(1), &views);
    assert_eq!(next, Some(WindowId::from_raw(2)));
}

#[test]
fn test_default_focus_vertical() {
    let policy = DefaultFocusPolicy;
    let views = vec![
        WindowView::new(WindowId::from_raw(1), Rect::new(0, 0, 80, 12)),
        WindowView::new(WindowId::from_raw(2), Rect::new(0, 12, 80, 12)),
    ];

    // From top, go down
    let next = policy.next(NavigateDirection::Down, WindowId::from_raw(1), &views);
    assert_eq!(next, Some(WindowId::from_raw(2)));

    // From bottom, go up
    let next = policy.next(NavigateDirection::Up, WindowId::from_raw(2), &views);
    assert_eq!(next, Some(WindowId::from_raw(1)));
}

#[test]
fn test_default_focus_unknown_current() {
    let policy = DefaultFocusPolicy;
    let views = vec![WindowView::new(
        WindowId::from_raw(1),
        Rect::new(0, 0, 80, 24),
    )];

    // Current window not in views
    let next = policy.next(NavigateDirection::Left, WindowId::from_raw(999), &views);
    assert_eq!(next, None);
}

#[test]
fn test_default_focus_best_skips_farther_window() {
    // Exercise line 183: best is Some with dist >= d (farther window is skipped)
    let policy = DefaultFocusPolicy;
    let views = vec![
        WindowView::new(WindowId::from_raw(1), Rect::new(0, 0, 40, 24)),
        // Window 2 is closer to the right
        WindowView::new(WindowId::from_raw(2), Rect::new(40, 0, 20, 24)),
        // Window 3 is farther to the right
        WindowView::new(WindowId::from_raw(3), Rect::new(80, 0, 20, 24)),
    ];

    // From window 1, going right: window 2 (closer) should win over window 3 (farther)
    let next = policy.next(NavigateDirection::Right, WindowId::from_raw(1), &views);
    assert_eq!(next, Some(WindowId::from_raw(2)));
}

#[test]
fn test_center() {
    assert_eq!(center(Rect::new(0, 0, 80, 24)), (40, 12));
    assert_eq!(center(Rect::new(10, 10, 20, 20)), (20, 20));
}
