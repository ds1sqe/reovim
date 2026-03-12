use {
    super::*,
    reovim_protocol::v2::{WindowLeaf, WindowSplit, window_node::Node},
};

fn make_leaf_node(window_id: u64, buffer_id: Option<u64>) -> WindowNode {
    WindowNode {
        node: Some(Node::Leaf(WindowLeaf {
            window_id,
            buffer_id,
            rect: Some(WindowRect {
                x: 0,
                y: 0,
                width: 80,
                height: 24,
            }),
        })),
    }
}

#[test]
fn test_collect_windows_single_leaf() {
    let node = make_leaf_node(1, Some(10));
    let mut out = Vec::new();

    collect_windows(&node, 1, &mut out);

    assert_eq!(out.len(), 1);
    assert_eq!(out[0].window_id, 1);
    assert_eq!(out[0].buffer_id, Some(10));
    assert!(out[0].focused);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_collect_windows_skips_no_buffer() {
    let node = make_leaf_node(1, None);
    let mut out = Vec::new();

    collect_windows(&node, 1, &mut out);

    assert!(out.is_empty()); // Window with no buffer is skipped
}

#[test]
fn test_collect_windows_split() {
    let split = WindowNode {
        node: Some(Node::Split(WindowSplit {
            direction: 0,
            children: vec![make_leaf_node(1, Some(10)), make_leaf_node(2, Some(20))],
        })),
    };
    let mut out = Vec::new();

    collect_windows(&split, 2, &mut out);

    assert_eq!(out.len(), 2);
    assert!(!out[0].focused); // Window 1 not focused
    assert!(out[1].focused); // Window 2 is focused
}

#[test]
fn test_apply_layout_empty() {
    let mut state = TuiCoreState::new(1);
    let layout = GetLayoutResponse {
        focused_window_id: None,
        root: None,
        active_tab_id: None,
        tabs: Vec::new(),
    };

    apply_layout(&mut state, &layout);

    assert_eq!(state.focused_window_id, 0);
    assert!(state.windows.is_empty());
    assert!(state.needs_default_window());
}

#[test]
fn test_apply_layout_with_root() {
    let mut state = TuiCoreState::new(1);
    let layout = GetLayoutResponse {
        focused_window_id: Some(5),
        root: Some(make_leaf_node(5, Some(100))),
        active_tab_id: None,
        tabs: Vec::new(),
    };

    apply_layout(&mut state, &layout);

    assert_eq!(state.focused_window_id, 5);
    assert_eq!(state.windows.len(), 1);
    assert!(!state.needs_default_window());
}

#[test]
fn test_create_default_window() {
    let mut state = TuiCoreState::new(1);
    state.set_needs_default_window(true);

    create_default_window(&mut state, 42, 80, 24);

    assert_eq!(state.windows.len(), 1);
    assert_eq!(state.windows[0].window_id, 1);
    assert_eq!(state.windows[0].buffer_id, Some(42));
    assert_eq!(state.focused_window_id, 1);
    assert!(!state.needs_default_window());
}

#[test]
fn test_apply_layout_notification() {
    let mut state = TuiCoreState::new(1);

    let windows = vec![
        WindowInfo {
            window_id: 1,
            buffer_id: Some(10),
            rect: None,
            focused: false,
            opacity: None,
        },
        WindowInfo {
            window_id: 2,
            buffer_id: Some(20),
            rect: None,
            focused: false,
            opacity: None,
        },
    ];

    apply_layout_notification(&mut state, Some(2), windows);

    assert_eq!(state.focused_window_id, 2);
    assert_eq!(state.windows.len(), 2);
    assert!(!state.windows[0].focused);
    assert!(state.windows[1].focused);
}

#[test]
fn test_apply_layout_notification_no_focused_id_uses_first() {
    let mut state = TuiCoreState::new(1);

    let windows = vec![
        WindowInfo {
            window_id: 10,
            buffer_id: Some(100),
            rect: None,
            focused: false,
            opacity: None,
        },
        WindowInfo {
            window_id: 20,
            buffer_id: Some(200),
            rect: None,
            focused: false,
            opacity: None,
        },
    ];

    apply_layout_notification(&mut state, None, windows);

    // Should use first window's ID as focused
    assert_eq!(state.focused_window_id, 10);
    assert!(state.windows[0].focused);
    assert!(!state.windows[1].focused);
}

#[test]
fn test_apply_layout_notification_no_focused_no_windows() {
    let mut state = TuiCoreState::new(1);

    apply_layout_notification(&mut state, None, Vec::new());

    assert_eq!(state.focused_window_id, 0);
    assert!(state.windows.is_empty());
}

#[test]
fn test_apply_layout_notification_cleans_stale_cursors() {
    let mut state = TuiCoreState::new(1);
    // Pre-populate cursors for windows that won't exist after layout change
    state.update_local_cursor(99, 5, 3);
    state
        .window_selections
        .insert(99, crate::SelectionState::default());

    let windows = vec![WindowInfo {
        window_id: 1,
        buffer_id: Some(10),
        rect: None,
        focused: true,
        opacity: None,
    }];

    apply_layout_notification(&mut state, Some(1), windows);

    // Stale cursor for window 99 should be cleaned up
    assert!(!state.window_cursors.contains_key(&99));
    assert!(!state.window_selections.contains_key(&99));
}

#[test]
fn test_collect_windows_empty_node() {
    let node = WindowNode { node: None };
    let mut out = Vec::new();
    collect_windows(&node, 1, &mut out);
    assert!(out.is_empty());
}

#[test]
fn test_collect_windows_nested_split() {
    // Split of splits
    let inner_split = WindowNode {
        node: Some(Node::Split(WindowSplit {
            direction: 0,
            children: vec![make_leaf_node(3, Some(30)), make_leaf_node(4, Some(40))],
        })),
    };
    let outer_split = WindowNode {
        node: Some(Node::Split(WindowSplit {
            direction: 1,
            children: vec![make_leaf_node(1, Some(10)), inner_split],
        })),
    };

    let mut out = Vec::new();
    collect_windows(&outer_split, 3, &mut out);

    assert_eq!(out.len(), 3);
    assert!(!out[0].focused); // window 1
    assert!(out[1].focused); // window 3 (focused)
    assert!(!out[2].focused); // window 4
}

#[test]
fn test_collect_windows_preserves_rect() {
    let node = make_leaf_node(1, Some(10));
    let mut out = Vec::new();
    collect_windows(&node, 1, &mut out);

    assert!(out[0].rect.is_some());
    let rect = out[0].rect.as_ref().unwrap();
    assert_eq!(rect.width, 80);
    assert_eq!(rect.height, 24);
}

#[test]
fn test_apply_layout_clears_previous_windows() {
    let mut state = TuiCoreState::new(1);

    // First layout
    let layout1 = GetLayoutResponse {
        focused_window_id: Some(1),
        root: Some(make_leaf_node(1, Some(10))),
        active_tab_id: None,
        tabs: Vec::new(),
    };
    apply_layout(&mut state, &layout1);
    assert_eq!(state.windows.len(), 1);

    // Second layout with different windows
    let layout2 = GetLayoutResponse {
        focused_window_id: Some(2),
        root: Some(make_leaf_node(2, Some(20))),
        active_tab_id: None,
        tabs: Vec::new(),
    };
    apply_layout(&mut state, &layout2);
    assert_eq!(state.windows.len(), 1);
    assert_eq!(state.windows[0].window_id, 2);
}

#[test]
fn test_create_default_window_reserves_statusline() {
    let mut state = TuiCoreState::new(1);
    state.set_needs_default_window(true);

    create_default_window(&mut state, 42, 80, 24);

    let rect = state.windows[0].rect.as_ref().unwrap();
    assert_eq!(rect.height, 23); // 24 - 1 for statusline
    assert_eq!(rect.width, 80);
    assert_eq!(rect.x, 0);
    assert_eq!(rect.y, 0);
}

#[test]
fn test_create_default_window_small_height() {
    let mut state = TuiCoreState::new(1);
    state.set_needs_default_window(true);

    create_default_window(&mut state, 1, 40, 1);

    let rect = state.windows[0].rect.as_ref().unwrap();
    assert_eq!(rect.height, 0); // 1 - 1 (saturating_sub)
}

#[test]
fn test_collect_windows_split_with_empty_children() {
    let split = WindowNode {
        node: Some(Node::Split(WindowSplit {
            direction: 0,
            children: Vec::new(),
        })),
    };
    let mut out = Vec::new();
    collect_windows(&split, 1, &mut out);
    assert!(out.is_empty());
}
