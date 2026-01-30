//! Layout types representing window arrangement.
//!
//! These types describe the logical structure of window layouts
//! as sent from the server. Clients interpret these into their
//! platform-specific window trees.

use crate::SplitDirection;

/// Logical layout structure from the server.
///
/// Represents how windows are arranged without specifying exact
/// screen coordinates. The client calculates positions based on
/// available screen space.
#[derive(Debug, Clone, PartialEq)]
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
mod tests {
    use super::*;

    #[test]
    fn test_single_layout() {
        let layout = LogicalLayout::single(1, 100);
        assert!(layout.is_leaf());
        assert_eq!(layout.buffer_id(), Some(1));
        assert_eq!(layout.window_count(), 1);
    }

    #[test]
    fn test_hsplit_layout() {
        let layout = LogicalLayout::hsplit(vec![
            LogicalLayout::single(1, 100),
            LogicalLayout::single(2, 101),
        ]);
        assert!(!layout.is_leaf());
        assert_eq!(layout.buffer_id(), None);
        assert_eq!(layout.window_count(), 2);

        if let LogicalLayout::Split {
            direction, ratios, ..
        } = layout
        {
            assert_eq!(direction, SplitDirection::Horizontal);
            assert_eq!(ratios.len(), 2);
            assert!((ratios[0] - 0.5).abs() < f32::EPSILON);
        } else {
            panic!("Expected Split layout");
        }
    }

    #[test]
    fn test_vsplit_layout() {
        let layout = LogicalLayout::vsplit(vec![
            LogicalLayout::single(1, 100),
            LogicalLayout::single(2, 101),
            LogicalLayout::single(3, 102),
        ]);
        assert_eq!(layout.window_count(), 3);

        if let LogicalLayout::Split {
            direction, ratios, ..
        } = layout
        {
            assert_eq!(direction, SplitDirection::Vertical);
            assert_eq!(ratios.len(), 3);
        } else {
            panic!("Expected Split layout");
        }
    }

    #[test]
    fn test_nested_layout() {
        // Two columns, left column has two rows
        let layout = LogicalLayout::vsplit(vec![
            LogicalLayout::hsplit(vec![
                LogicalLayout::single(1, 100),
                LogicalLayout::single(2, 101),
            ]),
            LogicalLayout::single(3, 102),
        ]);
        assert_eq!(layout.window_count(), 3);
    }

    #[test]
    fn test_tabs_layout() {
        let layout = LogicalLayout::tabs(
            vec![LogicalLayout::single(1, 100), LogicalLayout::single(2, 101)],
            0,
        );
        assert_eq!(layout.window_count(), 2);
        assert!(!layout.is_leaf());
    }

    #[test]
    fn test_find_viewport() {
        let layout = LogicalLayout::vsplit(vec![
            LogicalLayout::hsplit(vec![
                LogicalLayout::single(1, 100),
                LogicalLayout::single(2, 101),
            ]),
            LogicalLayout::single(3, 102),
        ]);

        assert_eq!(layout.find_viewport(100), Some(vec![0, 0]));
        assert_eq!(layout.find_viewport(101), Some(vec![0, 1]));
        assert_eq!(layout.find_viewport(102), Some(vec![1]));
        assert_eq!(layout.find_viewport(999), None);
    }
}
