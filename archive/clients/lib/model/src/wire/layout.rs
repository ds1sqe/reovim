//! Layout types representing window arrangement.
//!
//! These types describe the logical structure of window layouts
//! as sent from the server. Clients interpret these into their
//! platform-specific window trees.

use serde::{Deserialize, Serialize};

use crate::SplitDirection;

/// Logical layout structure from the server.
///
/// Represents how windows are arranged without specifying exact
/// screen coordinates. The client calculates positions based on
/// available screen space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum LogicalLayout {
    /// A single window showing a buffer.
    Single {
        /// Buffer being displayed.
        buffer_id: u64,
        /// Viewport identifier (for scroll state).
        viewport_id: u64,
    },

    /// Split layout with multiple children.
    Split {
        /// Direction of the split.
        direction: SplitDirection,
        /// Child layouts.
        children: Vec<Self>,
        /// Relative sizes (should sum to 1.0).
        ratios: Vec<f32>,
    },

    /// Tabbed layout with multiple children.
    Tabs {
        /// Tab layouts.
        tabs: Vec<Self>,
        /// Active tab index.
        active: usize,
    },
}

impl LogicalLayout {
    /// Create a single window layout.
    #[must_use]
    pub const fn single(buffer_id: u64, viewport_id: u64) -> Self {
        Self::Single {
            buffer_id,
            viewport_id,
        }
    }

    /// Create a horizontal split (windows stacked vertically).
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // Window count is always small
    pub fn hsplit(children: Vec<Self>) -> Self {
        let count = children.len();
        let ratio = 1.0 / count as f32;
        Self::Split {
            direction: SplitDirection::Horizontal,
            children,
            ratios: vec![ratio; count],
        }
    }

    /// Create a vertical split (windows side by side).
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // Window count is always small
    pub fn vsplit(children: Vec<Self>) -> Self {
        let count = children.len();
        let ratio = 1.0 / count as f32;
        Self::Split {
            direction: SplitDirection::Vertical,
            children,
            ratios: vec![ratio; count],
        }
    }

    /// Create a split with custom ratios.
    #[must_use]
    pub fn split_with_ratios(
        direction: SplitDirection,
        children: Vec<Self>,
        ratios: Vec<f32>,
    ) -> Self {
        debug_assert_eq!(children.len(), ratios.len());
        Self::Split {
            direction,
            children,
            ratios,
        }
    }

    /// Create a tabbed layout.
    #[must_use]
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn tabs(tabs: Vec<Self>, active: usize) -> Self {
        debug_assert!(active < tabs.len() || tabs.is_empty());
        Self::Tabs { tabs, active }
    }

    /// Get the total number of leaf windows.
    #[must_use]
    pub fn window_count(&self) -> usize {
        match self {
            Self::Single { .. } => 1,
            Self::Split { children, .. } => children.iter().map(Self::window_count).sum(),
            Self::Tabs { tabs, .. } => tabs.iter().map(Self::window_count).sum(),
        }
    }

    /// Check if this is a leaf (single window).
    #[must_use]
    pub const fn is_leaf(&self) -> bool {
        matches!(self, Self::Single { .. })
    }

    /// Get the buffer ID if this is a single window.
    #[must_use]
    pub const fn buffer_id(&self) -> Option<u64> {
        match self {
            Self::Single { buffer_id, .. } => Some(*buffer_id),
            _ => None,
        }
    }

    /// Find a viewport by ID and return its path.
    ///
    /// Returns indices through the tree to reach the viewport.
    #[must_use]
    pub fn find_viewport(&self, target_viewport: u64) -> Option<Vec<usize>> {
        self.find_viewport_inner(target_viewport, Vec::new())
    }

    fn find_viewport_inner(&self, target: u64, path: Vec<usize>) -> Option<Vec<usize>> {
        match self {
            Self::Single { viewport_id, .. } => {
                if *viewport_id == target {
                    Some(path)
                } else {
                    None
                }
            }
            Self::Split { children, .. } | Self::Tabs { tabs: children, .. } => {
                for (i, child) in children.iter().enumerate() {
                    let mut child_path = path.clone();
                    child_path.push(i);
                    if let Some(found) = child.find_viewport_inner(target, child_path) {
                        return Some(found);
                    }
                }
                None
            }
        }
    }
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
