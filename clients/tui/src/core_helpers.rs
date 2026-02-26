//! Shared helper functions for TUI layout handling.
//!
//! This module extracts common utility functions that `TuiApp<O: TuiOutput>`
//! uses for layout handling and buffer management.
//!
//! # Functions
//!
//! - `collect_windows` - Recursively flatten window tree to list
//! - `apply_layout` - Apply layout response to core state
//! - `create_default_window` - Create a default window for empty layouts
//! - `fetch_buffer_contents` - Fetch buffer content for visible windows

use reovim_protocol::v2::{GetLayoutResponse, WindowInfo, WindowNode, WindowRect};

use crate::{
    TuiCoreState,
    grpc_client::{TuiGrpcClient, TuiGrpcError},
};

/// Recursively collect windows from a layout tree node.
///
/// Flattens the hierarchical window tree into a linear list, marking
/// the focused window based on `focused_id`.
///
/// # Arguments
///
/// * `node` - Current window tree node
/// * `focused_id` - ID of the focused window
/// * `out` - Output vector to collect windows into
pub fn collect_windows(node: &WindowNode, focused_id: u64, out: &mut Vec<WindowInfo>) {
    if let Some(n) = &node.node {
        match n {
            reovim_protocol::v2::window_node::Node::Leaf(leaf) => {
                // Skip windows with no buffer (they can't be displayed)
                if leaf.buffer_id.is_some() {
                    out.push(WindowInfo {
                        window_id: leaf.window_id,
                        buffer_id: leaf.buffer_id,
                        rect: leaf.rect,
                        focused: leaf.window_id == focused_id,
                    });
                } else {
                    tracing::debug!(window_id = leaf.window_id, "Skipping window with no buffer");
                }
            }
            reovim_protocol::v2::window_node::Node::Split(split) => {
                for child in &split.children {
                    collect_windows(child, focused_id, out);
                }
            }
        }
    }
}

/// Apply a layout response to the core state.
///
/// Updates focused window ID and flattens the window tree. If the layout
/// is empty, marks that a default window needs to be created.
///
/// # Arguments
///
/// * `state` - Mutable reference to core state
/// * `layout` - Layout response from server
pub fn apply_layout(state: &mut TuiCoreState, layout: &GetLayoutResponse) {
    // focused_window_id is Option<u64> - None means "no windows" or "no focus"
    state.focused_window_id = layout.focused_window_id.unwrap_or(0);

    // Flatten window tree to list
    state.windows.clear();
    if let Some(root) = &layout.root {
        collect_windows(root, state.focused_window_id, &mut state.windows);
    }

    // Mark if we need to create a default window
    state.set_needs_default_window(state.windows.is_empty());
}

/// Apply a layout changed notification to the core state.
///
/// Similar to `apply_layout` but uses the notification format which has
/// a flat window list instead of a tree.
///
/// # Arguments
///
/// * `state` - Mutable reference to core state
/// * `focused_window_id` - Optional focused window ID
/// * `windows` - Flat list of windows from notification
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn apply_layout_notification(
    state: &mut TuiCoreState,
    focused_window_id: Option<u64>,
    windows: Vec<WindowInfo>,
) {
    // Determine effective focused ID
    let effective_focused_id = match focused_window_id {
        Some(id) => id,
        None if !windows.is_empty() => {
            tracing::debug!(
                "Server sent no focused_window_id with {} windows, using first",
                windows.len()
            );
            windows.first().map_or(0, |w| w.window_id)
        }
        None => 0,
    };

    state.focused_window_id = effective_focused_id;

    // Set focused flag on the matching window
    state.windows = windows
        .into_iter()
        .map(|mut w| {
            w.focused = w.window_id == effective_focused_id;
            w
        })
        .collect();

    // Clean up stale cursor entries
    state.cleanup_stale_cursors();
}

/// Create a default window view for empty layout.
///
/// Called when server returns no windows. Creates a local window
/// viewing the specified buffer with full viewport dimensions.
///
/// # Arguments
///
/// * `state` - Mutable reference to core state
/// * `buffer_id` - Buffer ID to display in the window
/// * `width` - Viewport width
/// * `height` - Viewport height
pub fn create_default_window(state: &mut TuiCoreState, buffer_id: u64, width: u16, height: u16) {
    let content_height = height.saturating_sub(1); // Reserve statusline

    let window = WindowInfo {
        window_id: 1, // Local ID
        buffer_id: Some(buffer_id),
        rect: Some(WindowRect {
            x: 0,
            y: 0,
            width: u64::from(width),
            height: u64::from(content_height),
        }),
        focused: true,
    };

    state.windows.push(window);
    state.focused_window_id = 1;
    state.set_needs_default_window(false);

    tracing::debug!(buffer_id, "Created default window for empty layout");
}

/// Fetch buffer content for all visible windows.
///
/// Iterates through windows and fetches content for any buffers not
/// already in the cache.
///
/// # Arguments
///
/// * `client` - gRPC client for server communication
/// * `state` - Mutable reference to core state
///
/// # Errors
///
/// Returns an error if buffer content fetch fails.
#[cfg_attr(coverage_nightly, coverage(off))]
pub async fn fetch_buffer_contents(
    client: &mut TuiGrpcClient,
    state: &mut TuiCoreState,
) -> Result<(), TuiGrpcError> {
    // Collect buffer IDs to fetch (filter out None values)
    let buffer_ids: Vec<u64> = state.windows.iter().filter_map(|w| w.buffer_id).collect();

    for buffer_id in buffer_ids {
        if let std::collections::hash_map::Entry::Vacant(e) = state.buffer_cache.entry(buffer_id) {
            let content = client
                .get_buffer_content(Some(buffer_id), None, None)
                .await?;
            e.insert(content.lines);
        }
    }

    Ok(())
}

/// Invalidate and refetch a specific buffer's content.
///
/// Called when a buffer is modified to refresh the cached content.
///
/// # Arguments
///
/// * `client` - gRPC client for server communication
/// * `state` - Mutable reference to core state
/// * `buffer_id` - ID of the modified buffer
///
/// # Errors
///
/// Returns an error if buffer content fetch fails.
#[cfg_attr(coverage_nightly, coverage(off))]
pub async fn refetch_buffer(
    client: &mut TuiGrpcClient,
    state: &mut TuiCoreState,
    buffer_id: u64,
) -> Result<(), TuiGrpcError> {
    state.buffer_cache.remove(&buffer_id);
    let content = client
        .get_buffer_content(Some(buffer_id), None, None)
        .await?;
    state.buffer_cache.insert(buffer_id, content.lines);
    Ok(())
}

#[cfg(test)]
mod tests {
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
            },
            WindowInfo {
                window_id: 2,
                buffer_id: Some(20),
                rect: None,
                focused: false,
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
            },
            WindowInfo {
                window_id: 20,
                buffer_id: Some(200),
                rect: None,
                focused: false,
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
        };
        apply_layout(&mut state, &layout1);
        assert_eq!(state.windows.len(), 1);

        // Second layout with different windows
        let layout2 = GetLayoutResponse {
            focused_window_id: Some(2),
            root: Some(make_leaf_node(2, Some(20))),
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
}
