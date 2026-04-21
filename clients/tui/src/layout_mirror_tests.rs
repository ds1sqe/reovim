use {super::*, reovim_protocol::v3::WindowRect};

fn make_window_info(
    id: u64,
    buffer_id: u64,
    x: u64,
    y: u64,
    w: u64,
    h: u64,
    focused: bool,
) -> WindowInfo {
    WindowInfo {
        window_id: id,
        buffer_id: Some(buffer_id),
        rect: Some(WindowRect {
            x,
            y,
            width: w,
            height: h,
        }),
        focused,
        opacity: None,
        primary_domain_id: 0,
        embedded_domain_ids: vec![],
        spatial_placement: None,
    }
}

#[test]
fn test_new_mirror() {
    let mirror = ServerLayoutMirror::new(80, 24);
    assert_eq!(mirror.window_count(), 0);
    assert!(mirror.focused_id().is_none());
    assert_eq!(mirror.screen_size(), (80, 24));
}

#[test]
fn test_apply_single_window() {
    let mut mirror = ServerLayoutMirror::new(80, 24);
    let windows = vec![make_window_info(1, 100, 0, 0, 80, 23, true)];

    mirror.apply_layout_changed(1, &windows);

    assert_eq!(mirror.window_count(), 1);
    assert_eq!(mirror.focused_id(), Some(1));

    let placement = mirror.placements().first().unwrap();
    assert_eq!(placement.window_id, 1);
    assert_eq!(placement.buffer_id, Some(100));
    assert_eq!(placement.x, 0);
    assert_eq!(placement.y, 0);
    assert_eq!(placement.width, 80);
    assert_eq!(placement.height, 23);
    assert!(placement.focused);
}

#[test]
fn test_apply_multiple_windows() {
    let mut mirror = ServerLayoutMirror::new(80, 24);
    let windows = vec![
        make_window_info(1, 100, 0, 0, 40, 23, true),
        make_window_info(2, 101, 40, 0, 40, 23, false),
    ];

    mirror.apply_layout_changed(1, &windows);

    assert_eq!(mirror.window_count(), 2);
    assert!(mirror.has_multiple_windows());

    // Check focused placement
    let focused = mirror.focused_placement().unwrap();
    assert_eq!(focused.window_id, 1);
    assert!(focused.focused);

    // Check second window
    let second = mirror.get_placement(2).unwrap();
    assert_eq!(second.window_id, 2);
    assert!(!second.focused);
}

#[test]
fn test_focus_tracking() {
    let mut mirror = ServerLayoutMirror::new(80, 24);
    let windows = vec![
        make_window_info(1, 100, 0, 0, 40, 23, false),
        make_window_info(2, 101, 40, 0, 40, 23, true),
    ];

    mirror.apply_layout_changed(2, &windows);

    assert_eq!(mirror.focused_id(), Some(2));
    let focused = mirror.focused_placement().unwrap();
    assert_eq!(focused.window_id, 2);
}

#[test]
fn test_set_screen() {
    let mut mirror = ServerLayoutMirror::new(80, 24);
    assert_eq!(mirror.screen_size(), (80, 24));

    mirror.set_screen(120, 40);
    assert_eq!(mirror.screen_size(), (120, 40));
}

#[test]
fn test_window_without_rect_uses_fallback() {
    let mut mirror = ServerLayoutMirror::new(80, 24);
    // 24 - 1 (statusline) = 23 content height
    // 2 windows → each gets 11 rows (23 / 2 = 11)
    let windows = vec![
        WindowInfo {
            window_id: 1,
            buffer_id: Some(100),
            rect: None, // No rect - uses fallback vertical stacking
            focused: true,
            opacity: None,
            primary_domain_id: 0,
            embedded_domain_ids: vec![],
            spatial_placement: None,
        },
        WindowInfo {
            window_id: 2,
            buffer_id: Some(101),
            rect: None, // No rect - uses fallback vertical stacking
            focused: false,
            opacity: None,
            primary_domain_id: 0,
            embedded_domain_ids: vec![],
            spatial_placement: None,
        },
    ];

    mirror.apply_layout_changed(1, &windows);

    // Both windows should be present with calculated positions
    assert_eq!(mirror.window_count(), 2);

    // First window at top
    let first = mirror.get_placement(1).unwrap();
    assert_eq!(first.x, 0);
    assert_eq!(first.y, 0);
    assert_eq!(first.width, 80);
    assert_eq!(first.height, 11);

    // Second window below first
    let second = mirror.get_placement(2).unwrap();
    assert_eq!(second.x, 0);
    assert_eq!(second.y, 11);
    assert_eq!(second.width, 80);
    assert_eq!(second.height, 11);
}

#[test]
fn test_default_mirror() {
    let mirror = ServerLayoutMirror::default();
    assert_eq!(mirror.window_count(), 0);
    assert!(mirror.focused_id().is_none());
    assert!(!mirror.has_multiple_windows());
    assert!(mirror.placements().is_empty());
}

#[test]
fn test_has_multiple_windows() {
    let mut mirror = ServerLayoutMirror::new(80, 24);
    assert!(!mirror.has_multiple_windows());

    let windows = vec![make_window_info(1, 100, 0, 0, 80, 23, true)];
    mirror.apply_layout_changed(1, &windows);
    assert!(!mirror.has_multiple_windows());

    let windows = vec![
        make_window_info(1, 100, 0, 0, 40, 23, true),
        make_window_info(2, 101, 40, 0, 40, 23, false),
    ];
    mirror.apply_layout_changed(1, &windows);
    assert!(mirror.has_multiple_windows());
}

#[test]
fn test_get_placement_nonexistent() {
    let mirror = ServerLayoutMirror::new(80, 24);
    assert!(mirror.get_placement(999).is_none());
}

#[test]
fn test_focused_placement_none() {
    let mirror = ServerLayoutMirror::new(80, 24);
    assert!(mirror.focused_placement().is_none());
}

#[test]
fn test_focused_placement_wrong_id() {
    let mut mirror = ServerLayoutMirror::new(80, 24);
    let windows = vec![make_window_info(1, 100, 0, 0, 80, 23, true)];
    mirror.apply_layout_changed(999, &windows); // Focus ID doesn't match any window

    assert_eq!(mirror.focused_id(), Some(999));
    assert!(mirror.focused_placement().is_none());
}

#[test]
fn test_apply_replaces_previous_layout() {
    let mut mirror = ServerLayoutMirror::new(80, 24);

    let windows1 = vec![make_window_info(1, 100, 0, 0, 80, 23, true)];
    mirror.apply_layout_changed(1, &windows1);
    assert_eq!(mirror.window_count(), 1);

    let windows2 = vec![
        make_window_info(2, 200, 0, 0, 40, 23, true),
        make_window_info(3, 300, 40, 0, 40, 23, false),
    ];
    mirror.apply_layout_changed(2, &windows2);
    assert_eq!(mirror.window_count(), 2);

    // Old window 1 should no longer be available
    assert!(mirror.get_placement(1).is_none());
    assert!(mirror.get_placement(2).is_some());
    assert!(mirror.get_placement(3).is_some());
}

#[test]
fn test_apply_empty_windows() {
    let mut mirror = ServerLayoutMirror::new(80, 24);
    mirror.apply_layout_changed(0, &[]);
    assert_eq!(mirror.window_count(), 0);
    assert_eq!(mirror.focused_id(), Some(0));
}

#[test]
fn test_single_window_uses_full_screen() {
    let mut mirror = ServerLayoutMirror::new(120, 40);
    // Single window - should use full width, height minus statusline
    let windows = vec![WindowInfo {
        window_id: 1,
        buffer_id: Some(100),
        rect: Some(WindowRect {
            x: 5,
            y: 5,
            width: 30,
            height: 20,
        }),
        focused: true,
        opacity: None,
        primary_domain_id: 0,
        embedded_domain_ids: vec![],
        spatial_placement: None,
    }];

    mirror.apply_layout_changed(1, &windows);

    let placement = mirror.get_placement(1).unwrap();
    // Single window ignores rect and uses full screen
    assert_eq!(placement.x, 0);
    assert_eq!(placement.y, 0);
    assert_eq!(placement.width, 120);
    assert_eq!(placement.height, 39); // 40 - 1 statusline
}

#[test]
fn test_window_placement_buffer_id_none() {
    let mut mirror = ServerLayoutMirror::new(80, 24);
    let windows = vec![WindowInfo {
        window_id: 1,
        buffer_id: None,
        rect: None,
        focused: true,
        opacity: None,
        primary_domain_id: 0,
        embedded_domain_ids: vec![],
        spatial_placement: None,
    }];

    mirror.apply_layout_changed(1, &windows);

    let placement = mirror.get_placement(1).unwrap();
    assert!(placement.buffer_id.is_none());
}

#[test]
fn test_window_placement_clone() {
    let placement = WindowPlacement {
        window_id: 1,
        buffer_id: Some(42),
        x: 10,
        y: 20,
        width: 80,
        height: 24,
        focused: true,
        opacity: 1.0,
    };
    let cloned = placement;
    assert_eq!(cloned.window_id, 1);
    assert_eq!(cloned.buffer_id, Some(42));
    assert_eq!(cloned.x, 10);
    assert_eq!(cloned.y, 20);
    assert!(cloned.focused);
}

#[test]
fn test_window_placement_debug() {
    let placement = WindowPlacement {
        window_id: 1,
        buffer_id: Some(42),
        x: 0,
        y: 0,
        width: 80,
        height: 24,
        focused: true,
        opacity: 1.0,
    };
    let debug = format!("{placement:?}");
    assert!(debug.contains("WindowPlacement"));
}

#[test]
fn test_mirror_debug() {
    let mirror = ServerLayoutMirror::new(80, 24);
    let debug = format!("{mirror:?}");
    assert!(debug.contains("ServerLayoutMirror"));
}
