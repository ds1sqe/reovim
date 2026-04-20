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
#[path = "window_tests.rs"]
mod tests;
