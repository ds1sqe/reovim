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
#[path = "layout_tests.rs"]
mod tests;
