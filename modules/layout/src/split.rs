//! Split tree data structure for window tiling.
//!
//! This module provides a binary tree structure for representing window splits.
//! Each node is either a split (horizontal or vertical) with two children,
//! or a leaf containing a window ID.
//!
//! # Architecture
//!
//! The split tree is a **policy** data structure - it decides HOW windows
//! are arranged. The display driver provides the **mechanism** (rendering
//! windows at given positions).
//!
//! # Example
//!
//! ```text
//! Initial: Single window (W1)
//!
//! After :vsplit (vertical split, creates W2 to the right):
//!     Split(Vertical, 0.5)
//!        /           \
//!      W1            W2
//!
//! After :split on W2 (horizontal split, creates W3 below W2):
//!     Split(Vertical, 0.5)
//!        /              \
//!      W1         Split(Horizontal, 0.5)
//!                    /         \
//!                  W2          W3
//! ```

use reovim_driver_display::{Rect, SplitDirection, WindowId};

/// Split tree node - represents layout hierarchy.
///
/// Each node is either a container with two children (split),
/// or a leaf containing a window.
#[derive(Debug, Clone, PartialEq)]
pub enum SplitNode {
    /// Container node with two children.
    Split {
        /// Direction of the split.
        direction: SplitDirection,
        /// Ratio for the first child (0.0 to 1.0).
        /// First child gets this ratio of the space.
        ratio: f32,
        /// First child (left for vertical, top for horizontal).
        first: Box<SplitNode>,
        /// Second child (right for vertical, bottom for horizontal).
        second: Box<SplitNode>,
    },
    /// Leaf node containing a window.
    Leaf(WindowId),
}

impl SplitNode {
    /// Create a new leaf node.
    #[must_use]
    pub const fn leaf(id: WindowId) -> Self {
        Self::Leaf(id)
    }

    /// Create a new split node with equal ratio.
    #[must_use]
    pub fn split(direction: SplitDirection, first: Self, second: Self) -> Self {
        Self::Split {
            direction,
            ratio: 0.5,
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    /// Create a new split node with custom ratio.
    #[must_use]
    pub fn split_with_ratio(
        direction: SplitDirection,
        ratio: f32,
        first: Self,
        second: Self,
    ) -> Self {
        Self::Split {
            direction,
            ratio: ratio.clamp(0.1, 0.9),
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    /// Check if this node is a leaf.
    #[must_use]
    pub const fn is_leaf(&self) -> bool {
        matches!(self, Self::Leaf(_))
    }

    /// Get the window ID if this is a leaf node.
    #[must_use]
    pub const fn window_id(&self) -> Option<WindowId> {
        match self {
            Self::Leaf(id) => Some(*id),
            Self::Split { .. } => None,
        }
    }

    /// Count the number of windows in this subtree.
    #[must_use]
    pub fn window_count(&self) -> usize {
        match self {
            Self::Leaf(_) => 1,
            Self::Split { first, second, .. } => first.window_count() + second.window_count(),
        }
    }

    /// Find a window by ID and return true if found.
    #[must_use]
    pub fn contains(&self, id: WindowId) -> bool {
        match self {
            Self::Leaf(leaf_id) => *leaf_id == id,
            Self::Split { first, second, .. } => first.contains(id) || second.contains(id),
        }
    }

    /// Calculate bounds for all windows in this subtree.
    ///
    /// Returns a vector of (WindowId, Rect) pairs with the calculated bounds.
    #[must_use]
    pub fn calculate_bounds(&self, bounds: Rect) -> Vec<(WindowId, Rect)> {
        match self {
            Self::Leaf(id) => vec![(*id, bounds)],
            Self::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let (first_bounds, second_bounds) = split_rect(bounds, *direction, *ratio);
                let mut result = first.calculate_bounds(first_bounds);
                result.extend(second.calculate_bounds(second_bounds));
                result
            }
        }
    }

    /// Split a window in this tree, creating a new window.
    ///
    /// Returns the modified tree (or None if the target wasn't found).
    #[must_use]
    pub fn split_window(
        self,
        target: WindowId,
        new_window: WindowId,
        direction: SplitDirection,
    ) -> Option<Self> {
        match self {
            Self::Leaf(id) if id == target => {
                // Found the target - split it
                Some(Self::split(direction, Self::Leaf(id), Self::Leaf(new_window)))
            }
            Self::Leaf(_) => {
                // Not the target
                None
            }
            Self::Split {
                direction: split_dir,
                ratio,
                first,
                second,
            } => {
                // Try to split in first child
                if let Some(new_first) = first.clone().split_window(target, new_window, direction) {
                    return Some(Self::Split {
                        direction: split_dir,
                        ratio,
                        first: Box::new(new_first),
                        second,
                    });
                }
                // Try to split in second child
                if let Some(new_second) = (*second)
                    .clone()
                    .split_window(target, new_window, direction)
                {
                    return Some(Self::Split {
                        direction: split_dir,
                        ratio,
                        first,
                        second: Box::new(new_second),
                    });
                }
                None
            }
        }
    }

    /// Remove a window from this tree.
    ///
    /// Returns the modified tree (or None if the tree becomes empty).
    #[must_use]
    pub fn remove_window(self, target: WindowId) -> Option<Self> {
        match self {
            Self::Leaf(id) if id == target => {
                // Found the target - remove it (tree becomes empty at this level)
                None
            }
            Self::Leaf(_) => {
                // Not the target, keep it
                Some(self)
            }
            Self::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let first_result = first.clone().remove_window(target);
                let second_result = (*second).clone().remove_window(target);

                match (first_result, second_result) {
                    // Target was in first child, and first became empty
                    (None, second_opt) => second_opt,
                    // Target was in second child, and second became empty
                    (first_opt, None) => first_opt,
                    // Both children still exist - rebuild the split
                    (Some(new_first), Some(new_second)) => Some(Self::Split {
                        direction,
                        ratio,
                        first: Box::new(new_first),
                        second: Box::new(new_second),
                    }),
                }
            }
        }
    }

    /// Collect all window IDs in traversal order.
    #[must_use]
    pub fn collect_windows(&self) -> Vec<WindowId> {
        match self {
            Self::Leaf(id) => vec![*id],
            Self::Split { first, second, .. } => {
                let mut result = first.collect_windows();
                result.extend(second.collect_windows());
                result
            }
        }
    }

    /// Adjust the split ratio for a window's parent split.
    ///
    /// `delta` is the change in ratio (-1.0 to 1.0).
    #[must_use]
    pub fn adjust_ratio(self, target: WindowId, delta: f32) -> Self {
        match self {
            Self::Leaf(_) => self,
            Self::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                // Check if target is a direct child
                let target_in_first = first.contains(target);
                let target_in_second = second.contains(target);

                if target_in_first || target_in_second {
                    // Adjust this split's ratio
                    let new_ratio = if target_in_first {
                        (ratio + delta).clamp(0.1, 0.9)
                    } else {
                        (ratio - delta).clamp(0.1, 0.9)
                    };
                    Self::Split {
                        direction,
                        ratio: new_ratio,
                        first,
                        second,
                    }
                } else {
                    // Recurse into children
                    Self::Split {
                        direction,
                        ratio,
                        first: Box::new(first.adjust_ratio(target, delta)),
                        second: Box::new(second.adjust_ratio(target, delta)),
                    }
                }
            }
        }
    }
}

/// Split a rectangle according to direction and ratio.
fn split_rect(bounds: Rect, direction: SplitDirection, ratio: f32) -> (Rect, Rect) {
    match direction {
        SplitDirection::Vertical => {
            // Left/right split
            let first_width = (f32::from(bounds.width) * ratio) as u16;
            let second_width = bounds.width.saturating_sub(first_width);
            (
                Rect::new(bounds.x, bounds.y, first_width, bounds.height),
                Rect::new(
                    bounds.x.saturating_add(first_width),
                    bounds.y,
                    second_width,
                    bounds.height,
                ),
            )
        }
        SplitDirection::Horizontal => {
            // Top/bottom split
            let first_height = (f32::from(bounds.height) * ratio) as u16;
            let second_height = bounds.height.saturating_sub(first_height);
            (
                Rect::new(bounds.x, bounds.y, bounds.width, first_height),
                Rect::new(
                    bounds.x,
                    bounds.y.saturating_add(first_height),
                    bounds.width,
                    second_height,
                ),
            )
        }
    }
}

/// Split tree manager.
///
/// Manages the window split tree and provides operations for
/// adding, removing, and arranging windows.
#[derive(Debug, Clone, Default)]
pub struct SplitTree {
    /// Root of the split tree (None if no windows).
    root: Option<SplitNode>,
    /// Next window ID to assign.
    next_id: usize,
}

impl SplitTree {
    /// Create a new empty split tree.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            root: None,
            next_id: 1,
        }
    }

    /// Check if the tree is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    /// Get the number of windows.
    #[must_use]
    pub fn window_count(&self) -> usize {
        self.root.as_ref().map_or(0, SplitNode::window_count)
    }

    /// Get all window IDs in the tree.
    #[must_use]
    pub fn windows(&self) -> Vec<WindowId> {
        self.root
            .as_ref()
            .map_or_else(Vec::new, SplitNode::collect_windows)
    }

    /// Add the first window to an empty tree.
    ///
    /// Returns the new window ID.
    pub fn add_first_window(&mut self) -> WindowId {
        let id = WindowId::from_raw(self.next_id);
        self.next_id += 1;
        self.root = Some(SplitNode::leaf(id));
        id
    }

    /// Add a window by splitting the target window.
    ///
    /// Returns the new window ID, or None if target wasn't found.
    pub fn split_window(
        &mut self,
        target: WindowId,
        direction: SplitDirection,
    ) -> Option<WindowId> {
        let root = self.root.take()?;

        let new_id = WindowId::from_raw(self.next_id);
        self.next_id += 1;

        // Clone root before split_window since it takes ownership
        let root_clone = root.clone();
        if let Some(new_root) = root.split_window(target, new_id, direction) {
            self.root = Some(new_root);
            Some(new_id)
        } else {
            // Target not found, restore original root
            self.root = Some(root_clone);
            None
        }
    }

    /// Remove a window from the tree.
    ///
    /// Returns true if the window was found and removed.
    pub fn remove_window(&mut self, target: WindowId) -> bool {
        if let Some(root) = self.root.take() {
            self.root = root.remove_window(target);
            true
        } else {
            false
        }
    }

    /// Calculate bounds for all windows.
    #[must_use]
    pub fn calculate_bounds(&self, screen_size: (u16, u16)) -> Vec<(WindowId, Rect)> {
        let (width, height) = screen_size;
        let bounds = Rect::new(0, 0, width, height);
        self.root
            .as_ref()
            .map_or_else(Vec::new, |node| node.calculate_bounds(bounds))
    }

    /// Check if a window exists in the tree.
    #[must_use]
    pub fn contains(&self, id: WindowId) -> bool {
        self.root.as_ref().is_some_and(|node| node.contains(id))
    }

    /// Adjust the split ratio for a window.
    pub fn adjust_ratio(&mut self, target: WindowId, delta: f32) {
        if let Some(root) = self.root.take() {
            self.root = Some(root.adjust_ratio(target, delta));
        }
    }

    /// Get the first window in the tree.
    #[must_use]
    pub fn first_window(&self) -> Option<WindowId> {
        self.windows().first().copied()
    }

    /// Add an existing window ID to an empty tree (for external ID management).
    pub fn add_existing_window(&mut self, id: WindowId) {
        if self.root.is_none() {
            self.root = Some(SplitNode::leaf(id));
            // Update next_id if needed
            if id.as_usize() >= self.next_id {
                self.next_id = id.as_usize() + 1;
            }
        }
    }

    /// Split with an existing window ID (for external ID management).
    pub fn split_with_existing(
        &mut self,
        target: WindowId,
        new_id: WindowId,
        direction: SplitDirection,
    ) -> bool {
        if let Some(root) = self.root.take() {
            // Clone root before split_window since it takes ownership
            let root_clone = root.clone();
            if let Some(new_root) = root.split_window(target, new_id, direction) {
                self.root = Some(new_root);
                // Update next_id if needed
                if new_id.as_usize() >= self.next_id {
                    self.next_id = new_id.as_usize() + 1;
                }
                return true;
            }
            // Target not found, restore original root
            self.root = Some(root_clone);
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_node_leaf() {
        let node = SplitNode::leaf(WindowId::from_raw(1));
        assert!(node.is_leaf());
        assert_eq!(node.window_id(), Some(WindowId::from_raw(1)));
        assert_eq!(node.window_count(), 1);
    }

    #[test]
    fn test_split_node_split() {
        let node = SplitNode::split(
            SplitDirection::Vertical,
            SplitNode::leaf(WindowId::from_raw(1)),
            SplitNode::leaf(WindowId::from_raw(2)),
        );
        assert!(!node.is_leaf());
        assert_eq!(node.window_id(), None);
        assert_eq!(node.window_count(), 2);
    }

    #[test]
    fn test_split_node_contains() {
        let node = SplitNode::split(
            SplitDirection::Vertical,
            SplitNode::leaf(WindowId::from_raw(1)),
            SplitNode::leaf(WindowId::from_raw(2)),
        );
        assert!(node.contains(WindowId::from_raw(1)));
        assert!(node.contains(WindowId::from_raw(2)));
        assert!(!node.contains(WindowId::from_raw(3)));
    }

    #[test]
    fn test_split_node_calculate_bounds() {
        let node = SplitNode::split(
            SplitDirection::Vertical,
            SplitNode::leaf(WindowId::from_raw(1)),
            SplitNode::leaf(WindowId::from_raw(2)),
        );
        let bounds = node.calculate_bounds(Rect::new(0, 0, 80, 24));
        assert_eq!(bounds.len(), 2);

        // First window gets left half
        let (id1, rect1) = bounds[0];
        assert_eq!(id1, WindowId::from_raw(1));
        assert_eq!(rect1.x, 0);
        assert_eq!(rect1.width, 40);

        // Second window gets right half
        let (id2, rect2) = bounds[1];
        assert_eq!(id2, WindowId::from_raw(2));
        assert_eq!(rect2.x, 40);
        assert_eq!(rect2.width, 40);
    }

    #[test]
    fn test_split_node_horizontal_bounds() {
        let node = SplitNode::split(
            SplitDirection::Horizontal,
            SplitNode::leaf(WindowId::from_raw(1)),
            SplitNode::leaf(WindowId::from_raw(2)),
        );
        let bounds = node.calculate_bounds(Rect::new(0, 0, 80, 24));

        // First window gets top half
        let (_, rect1) = bounds[0];
        assert_eq!(rect1.y, 0);
        assert_eq!(rect1.height, 12);

        // Second window gets bottom half
        let (_, rect2) = bounds[1];
        assert_eq!(rect2.y, 12);
        assert_eq!(rect2.height, 12);
    }

    #[test]
    fn test_split_node_split_window() {
        let node = SplitNode::leaf(WindowId::from_raw(1));
        let result = node.split_window(
            WindowId::from_raw(1),
            WindowId::from_raw(2),
            SplitDirection::Vertical,
        );

        assert!(result.is_some());
        let new_node = result.unwrap();
        assert!(!new_node.is_leaf());
        assert_eq!(new_node.window_count(), 2);
    }

    #[test]
    fn test_split_node_remove_window() {
        let node = SplitNode::split(
            SplitDirection::Vertical,
            SplitNode::leaf(WindowId::from_raw(1)),
            SplitNode::leaf(WindowId::from_raw(2)),
        );

        // Remove window 1 - should leave just window 2
        let result = node.remove_window(WindowId::from_raw(1));
        assert!(result.is_some());
        let new_node = result.unwrap();
        assert!(new_node.is_leaf());
        assert_eq!(new_node.window_id(), Some(WindowId::from_raw(2)));
    }

    #[test]
    fn test_split_node_collect_windows() {
        let node = SplitNode::split(
            SplitDirection::Vertical,
            SplitNode::leaf(WindowId::from_raw(1)),
            SplitNode::split(
                SplitDirection::Horizontal,
                SplitNode::leaf(WindowId::from_raw(2)),
                SplitNode::leaf(WindowId::from_raw(3)),
            ),
        );
        let windows = node.collect_windows();
        assert_eq!(windows.len(), 3);
        assert!(windows.contains(&WindowId::from_raw(1)));
        assert!(windows.contains(&WindowId::from_raw(2)));
        assert!(windows.contains(&WindowId::from_raw(3)));
    }

    #[test]
    fn test_split_tree_new() {
        let tree = SplitTree::new();
        assert!(tree.is_empty());
        assert_eq!(tree.window_count(), 0);
    }

    #[test]
    fn test_split_tree_add_first_window() {
        let mut tree = SplitTree::new();
        let id = tree.add_first_window();

        assert!(!tree.is_empty());
        assert_eq!(tree.window_count(), 1);
        assert!(tree.contains(id));
    }

    #[test]
    fn test_split_tree_split_window() {
        let mut tree = SplitTree::new();
        let first = tree.add_first_window();
        let second = tree.split_window(first, SplitDirection::Vertical);

        assert!(second.is_some());
        assert_eq!(tree.window_count(), 2);
    }

    #[test]
    fn test_split_tree_remove_window() {
        let mut tree = SplitTree::new();
        let first = tree.add_first_window();
        let second = tree.split_window(first, SplitDirection::Vertical).unwrap();

        assert!(tree.remove_window(first));
        assert_eq!(tree.window_count(), 1);
        assert!(tree.contains(second));
        assert!(!tree.contains(first));
    }

    #[test]
    fn test_split_tree_calculate_bounds() {
        let mut tree = SplitTree::new();
        let first = tree.add_first_window();
        tree.split_window(first, SplitDirection::Vertical);

        let bounds = tree.calculate_bounds((80, 24));
        assert_eq!(bounds.len(), 2);
    }

    #[test]
    fn test_split_rect_vertical() {
        let bounds = Rect::new(0, 0, 100, 50);
        let (left, right) = split_rect(bounds, SplitDirection::Vertical, 0.5);

        assert_eq!(left.x, 0);
        assert_eq!(left.width, 50);
        assert_eq!(right.x, 50);
        assert_eq!(right.width, 50);

        // Height should be unchanged
        assert_eq!(left.height, 50);
        assert_eq!(right.height, 50);
    }

    #[test]
    fn test_split_rect_horizontal() {
        let bounds = Rect::new(0, 0, 100, 50);
        let (top, bottom) = split_rect(bounds, SplitDirection::Horizontal, 0.5);

        assert_eq!(top.y, 0);
        assert_eq!(top.height, 25);
        assert_eq!(bottom.y, 25);
        assert_eq!(bottom.height, 25);

        // Width should be unchanged
        assert_eq!(top.width, 100);
        assert_eq!(bottom.width, 100);
    }

    #[test]
    fn test_split_rect_custom_ratio() {
        let bounds = Rect::new(0, 0, 100, 50);
        let (left, right) = split_rect(bounds, SplitDirection::Vertical, 0.3);

        assert_eq!(left.width, 30);
        assert_eq!(right.width, 70);
    }

    #[test]
    fn test_split_tree_windows() {
        let mut tree = SplitTree::new();
        let first = tree.add_first_window();
        let second = tree.split_window(first, SplitDirection::Vertical).unwrap();
        let third = tree
            .split_window(second, SplitDirection::Horizontal)
            .unwrap();

        let windows = tree.windows();
        assert_eq!(windows.len(), 3);
        assert!(windows.contains(&first));
        assert!(windows.contains(&second));
        assert!(windows.contains(&third));
    }

    #[test]
    fn test_split_node_adjust_ratio() {
        let node = SplitNode::split_with_ratio(
            SplitDirection::Vertical,
            0.5,
            SplitNode::leaf(WindowId::from_raw(1)),
            SplitNode::leaf(WindowId::from_raw(2)),
        );

        // Adjust ratio for window 1 (in first child)
        let adjusted = node.adjust_ratio(WindowId::from_raw(1), 0.1);
        if let SplitNode::Split { ratio, .. } = adjusted {
            assert!((ratio - 0.6).abs() < 0.001);
        } else {
            panic!("Expected Split node");
        }
    }
}
