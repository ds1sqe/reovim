//! Layout trait for window management.
//!
//! The Layout trait defines operations for splitting, closing, and
//! navigating between windows.

use crate::{Direction, LogicalLayout, SplitDirection};

/// Trait for window layout management.
///
/// Implementations manage the arrangement of editor windows,
/// supporting splits, tabs, and focus navigation.
pub trait Layout {
    /// Split the current window in the given direction.
    ///
    /// Returns the viewport ID of the new window.
    fn split(&mut self, direction: SplitDirection) -> u64;

    /// Close a window by viewport ID.
    ///
    /// Returns `true` if the window was found and closed.
    fn close(&mut self, viewport_id: u64) -> bool;

    /// Focus a specific window by viewport ID.
    ///
    /// Returns `true` if the window was found and focused.
    fn focus(&mut self, viewport_id: u64) -> bool;

    /// Move focus in the given direction.
    ///
    /// Returns `true` if focus was moved to another window.
    fn focus_direction(&mut self, direction: Direction) -> bool;

    /// Get the currently focused viewport ID.
    fn focused_viewport(&self) -> u64;

    /// Convert the current layout to logical format.
    ///
    /// Used for serialization and synchronization with the server.
    fn to_logical(&self) -> LogicalLayout;

    /// Apply a logical layout from the server.
    ///
    /// Reconstructs the window arrangement from the server's layout.
    fn apply_layout(&mut self, layout: &LogicalLayout);

    /// Get the total number of windows.
    fn window_count(&self) -> usize;

    /// Check if the layout has only one window.
    fn is_single(&self) -> bool {
        self.window_count() == 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockLayout {
        windows: Vec<u64>,
        focused: usize,
        next_id: u64,
    }

    impl MockLayout {
        fn new() -> Self {
            Self {
                windows: vec![1],
                focused: 0,
                next_id: 2,
            }
        }
    }

    impl Layout for MockLayout {
        fn split(&mut self, _direction: SplitDirection) -> u64 {
            let id = self.next_id;
            self.next_id += 1;
            self.windows.push(id);
            id
        }

        fn close(&mut self, viewport_id: u64) -> bool {
            if let Some(pos) = self.windows.iter().position(|&id| id == viewport_id) {
                self.windows.remove(pos);
                if self.focused >= self.windows.len() && !self.windows.is_empty() {
                    self.focused = self.windows.len() - 1;
                }
                true
            } else {
                false
            }
        }

        fn focus(&mut self, viewport_id: u64) -> bool {
            if let Some(pos) = self.windows.iter().position(|&id| id == viewport_id) {
                self.focused = pos;
                true
            } else {
                false
            }
        }

        fn focus_direction(&mut self, direction: Direction) -> bool {
            let new_focused = match direction {
                Direction::Right | Direction::Down => {
                    if self.focused + 1 < self.windows.len() {
                        self.focused + 1
                    } else {
                        return false;
                    }
                }
                Direction::Left | Direction::Up => {
                    if self.focused > 0 {
                        self.focused - 1
                    } else {
                        return false;
                    }
                }
            };
            self.focused = new_focused;
            true
        }

        fn focused_viewport(&self) -> u64 {
            self.windows[self.focused]
        }

        fn to_logical(&self) -> LogicalLayout {
            if self.windows.len() == 1 {
                LogicalLayout::single(self.windows[0], self.windows[0])
            } else {
                LogicalLayout::vsplit(
                    self.windows
                        .iter()
                        .map(|&id| LogicalLayout::single(id, id))
                        .collect(),
                )
            }
        }

        fn apply_layout(&mut self, layout: &LogicalLayout) {
            // Simplified: just count windows
            self.windows.clear();
            self.collect_viewports(layout);
            self.focused = 0;
        }

        fn window_count(&self) -> usize {
            self.windows.len()
        }
    }

    impl MockLayout {
        fn collect_viewports(&mut self, layout: &LogicalLayout) {
            match layout {
                LogicalLayout::Single { viewport_id, .. } => {
                    self.windows.push(*viewport_id);
                }
                LogicalLayout::Split { children, .. } => {
                    for child in children {
                        self.collect_viewports(child);
                    }
                }
                LogicalLayout::Tabs { tabs, .. } => {
                    for tab in tabs {
                        self.collect_viewports(tab);
                    }
                }
            }
        }
    }

    #[test]
    fn test_layout_split() {
        let mut layout = MockLayout::new();
        assert_eq!(layout.window_count(), 1);

        let new_id = layout.split(SplitDirection::Vertical);
        assert_eq!(layout.window_count(), 2);
        assert!(layout.windows.contains(&new_id));
    }

    #[test]
    fn test_layout_close() {
        let mut layout = MockLayout::new();
        let new_id = layout.split(SplitDirection::Vertical);

        assert!(layout.close(new_id));
        assert_eq!(layout.window_count(), 1);

        assert!(!layout.close(999)); // Non-existent
    }

    #[test]
    fn test_layout_focus() {
        let mut layout = MockLayout::new();
        let new_id = layout.split(SplitDirection::Vertical);

        assert!(layout.focus(new_id));
        assert_eq!(layout.focused_viewport(), new_id);

        assert!(!layout.focus(999)); // Non-existent
    }

    #[test]
    fn test_layout_focus_direction() {
        let mut layout = MockLayout::new();
        layout.split(SplitDirection::Vertical);

        assert_eq!(layout.focused, 0);
        assert!(layout.focus_direction(Direction::Right));
        assert_eq!(layout.focused, 1);
        assert!(!layout.focus_direction(Direction::Right)); // At end
    }

    #[test]
    fn test_layout_is_single() {
        let mut layout = MockLayout::new();
        assert!(layout.is_single());

        layout.split(SplitDirection::Vertical);
        assert!(!layout.is_single());
    }

    #[test]
    fn test_layout_to_logical() {
        let layout = MockLayout::new();
        let logical = layout.to_logical();
        assert!(logical.is_leaf());
    }
}
