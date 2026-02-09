//! Window types for client-side window management.
//!
//! These types represent the client's interpretation of logical layout
//! into actual window rectangles on screen.

use serde::{Deserialize, Serialize};

use crate::{Rect, SplitDirection};

/// A rendered window with screen bounds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Window {
    /// Window identifier (matches `viewport_id` from server).
    pub id: u64,
    /// Associated viewport ID.
    pub viewport_id: u64,
    /// Buffer being displayed.
    pub buffer_id: u64,
    /// Screen bounds of this window.
    pub bounds: Rect,
    /// Whether this window has focus.
    pub focused: bool,
}

impl Window {
    /// Create a new window.
    #[must_use]
    pub const fn new(id: u64, viewport_id: u64, buffer_id: u64, bounds: Rect) -> Self {
        Self {
            id,
            viewport_id,
            buffer_id,
            bounds,
            focused: false,
        }
    }

    /// Set the focused state.
    #[must_use]
    pub const fn with_focus(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Get the window's area.
    #[must_use]
    pub const fn area(&self) -> u32 {
        self.bounds.area()
    }
}

/// Tree structure of rendered windows.
///
/// Mirrors the logical layout structure but includes computed bounds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum WindowTree {
    /// A leaf window.
    Leaf(Window),

    /// A split containing multiple children.
    Split {
        /// Direction of the split.
        direction: SplitDirection,
        /// Child window trees.
        children: Vec<Self>,
        /// Bounds of this split container.
        bounds: Rect,
    },

    /// Tabbed windows (only one visible at a time).
    Tabs {
        /// Tab window trees.
        tabs: Vec<Self>,
        /// Active tab index.
        active: usize,
        /// Bounds of this tab container.
        bounds: Rect,
    },
}

impl WindowTree {
    /// Create a leaf window tree.
    #[must_use]
    pub const fn leaf(window: Window) -> Self {
        Self::Leaf(window)
    }

    /// Create a split window tree.
    #[must_use]
    pub const fn split(direction: SplitDirection, children: Vec<Self>, bounds: Rect) -> Self {
        Self::Split {
            direction,
            children,
            bounds,
        }
    }

    /// Create a tabbed window tree.
    #[must_use]
    pub const fn tabs(tabs: Vec<Self>, active: usize, bounds: Rect) -> Self {
        Self::Tabs {
            tabs,
            active,
            bounds,
        }
    }

    /// Get the bounds of this tree node.
    #[must_use]
    pub const fn bounds(&self) -> Rect {
        match self {
            Self::Leaf(window) => window.bounds,
            Self::Split { bounds, .. } | Self::Tabs { bounds, .. } => *bounds,
        }
    }

    /// Count total windows in the tree.
    #[must_use]
    pub fn window_count(&self) -> usize {
        match self {
            Self::Leaf(_) => 1,
            Self::Split { children, .. } => children.iter().map(Self::window_count).sum(),
            Self::Tabs { tabs, .. } => tabs.iter().map(Self::window_count).sum(),
        }
    }

    /// Find the focused window.
    #[must_use]
    pub fn focused_window(&self) -> Option<&Window> {
        match self {
            Self::Leaf(window) if window.focused => Some(window),
            Self::Leaf(_) => None,
            Self::Split { children, .. } => children.iter().find_map(Self::focused_window),
            Self::Tabs { tabs, active, .. } => tabs.get(*active).and_then(Self::focused_window),
        }
    }

    /// Find a window by viewport ID.
    #[must_use]
    pub fn find_by_viewport(&self, viewport_id: u64) -> Option<&Window> {
        match self {
            Self::Leaf(window) if window.viewport_id == viewport_id => Some(window),
            Self::Leaf(_) => None,
            Self::Split { children, .. } => children
                .iter()
                .find_map(|c| c.find_by_viewport(viewport_id)),
            Self::Tabs { tabs, .. } => tabs.iter().find_map(|t| t.find_by_viewport(viewport_id)),
        }
    }

    /// Get all leaf windows.
    #[must_use]
    pub fn all_windows(&self) -> Vec<&Window> {
        match self {
            Self::Leaf(window) => vec![window],
            Self::Split { children, .. } => children.iter().flat_map(Self::all_windows).collect(),
            Self::Tabs { tabs, .. } => tabs.iter().flat_map(Self::all_windows).collect(),
        }
    }

    /// Check if this is a leaf node.
    #[must_use]
    pub const fn is_leaf(&self) -> bool {
        matches!(self, Self::Leaf(_))
    }
}

#[cfg(test)]
mod tests {
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
}
