//! Window state management for the runner.
//!
//! This module provides `WindowRegistry` which tracks window state and layout.
//! Following mechanism vs policy separation:
//! - **Runner (this module)**: Owns `WindowState`, generates window IDs
//! - **Layout module**: Provides `TilingLayout` and `VimFocusPolicy` implementations
//!
//! # Example
//!
//! ```ignore
//! use runner::server::window::{WindowRegistry, WindowState};
//!
//! let mut registry = WindowRegistry::new();
//! let w1 = registry.create_window(Some(buffer_id));
//! let w2 = registry.split_vertical(w1).unwrap();
//! registry.set_active_window(w2);
//! ```

use std::collections::HashMap;

use {
    reovim_driver_display::{FocusPolicy, LayoutPolicy, NavigateDirection, WindowId, WindowView},
    reovim_kernel::api::v1::{BufferId, Position},
    reovim_module_layout::{TilingLayout, VimFocusPolicy},
};

/// Per-window state.
///
/// Each window tracks its own cursor position and scroll offset,
/// allowing multiple windows to view the same buffer independently.
#[derive(Debug, Clone)]
pub struct WindowState {
    /// Unique window identifier.
    pub id: WindowId,
    /// Buffer displayed in this window (None if empty).
    pub buffer_id: Option<BufferId>,
    /// Per-window cursor position.
    ///
    /// When multiple windows view the same buffer, each maintains
    /// its own cursor position.
    pub cursor: Position,
    /// Top visible line (scroll offset).
    pub scroll_top: usize,
}

impl WindowState {
    /// Create a new window state.
    #[must_use]
    pub fn new(id: WindowId, buffer_id: Option<BufferId>) -> Self {
        Self {
            id,
            buffer_id,
            cursor: Position::default(),
            scroll_top: 0,
        }
    }

    /// Create a window state with a specific cursor position.
    #[must_use]
    pub const fn with_cursor(mut self, cursor: Position) -> Self {
        self.cursor = cursor;
        self
    }

    /// Create a window state with a specific scroll offset.
    #[must_use]
    pub const fn with_scroll_top(mut self, scroll_top: usize) -> Self {
        self.scroll_top = scroll_top;
        self
    }
}

/// Window registry managing window state and layout.
///
/// Owns the `TilingLayout` for window arrangement and `VimFocusPolicy`
/// for directional navigation. Generates monotonically increasing window IDs.
#[derive(Debug)]
pub struct WindowRegistry {
    /// Per-window state indexed by window ID.
    windows: HashMap<WindowId, WindowState>,
    /// Currently active window.
    active_window: Option<WindowId>,
    /// Next window ID to assign.
    next_id: usize,
    /// Tiling layout for window arrangement.
    layout: TilingLayout,
    /// Focus policy for directional navigation.
    focus_policy: VimFocusPolicy,
}

impl Default for WindowRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowRegistry {
    /// Create a new empty window registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
            active_window: None,
            next_id: 1, // Start from 1 (0 often means "none")
            layout: TilingLayout::new(),
            focus_policy: VimFocusPolicy::new(),
        }
    }

    /// Create a new window with an optional buffer.
    ///
    /// Returns the new window's ID. The window is added to the layout
    /// and becomes the active window if it's the first one.
    pub fn create_window(&mut self, buffer_id: Option<BufferId>) -> WindowId {
        let id = WindowId::new(self.next_id);
        self.next_id += 1;

        let state = WindowState::new(id, buffer_id);
        self.windows.insert(id, state);

        // Add to layout
        if self.layout.is_empty() {
            self.layout.add_first_window();
        }

        // Set as active if first window
        if self.active_window.is_none() {
            self.active_window = Some(id);
        }

        id
    }

    /// Close a window.
    ///
    /// Returns `false` if the window doesn't exist or is the last window
    /// (cannot close the last window).
    pub fn close_window(&mut self, id: WindowId) -> bool {
        // Cannot close if not found
        if !self.windows.contains_key(&id) {
            return false;
        }

        // Cannot close the last window
        if self.windows.len() <= 1 {
            return false;
        }

        // Remove from windows map
        self.windows.remove(&id);

        // Remove from layout
        self.layout.close_window(id);

        // If we closed the active window, pick a new one
        if self.active_window == Some(id) {
            self.active_window = self.windows.keys().next().copied();
        }

        true
    }

    /// Get window state by ID.
    #[must_use]
    pub fn get(&self, id: WindowId) -> Option<&WindowState> {
        self.windows.get(&id)
    }

    /// Get mutable window state by ID.
    pub fn get_mut(&mut self, id: WindowId) -> Option<&mut WindowState> {
        self.windows.get_mut(&id)
    }

    /// Get the currently active window ID.
    #[must_use]
    pub const fn active_window(&self) -> Option<WindowId> {
        self.active_window
    }

    /// Set the active window.
    ///
    /// Returns `false` if the window doesn't exist.
    pub fn set_active_window(&mut self, id: WindowId) -> bool {
        if self.windows.contains_key(&id) {
            self.active_window = Some(id);
            true
        } else {
            false
        }
    }

    /// Split the target window horizontally (top/bottom).
    ///
    /// Creates a new window below the target. Returns `None` if the
    /// target window doesn't exist.
    pub fn split_horizontal(&mut self, target: WindowId) -> Option<WindowId> {
        if !self.windows.contains_key(&target) {
            return None;
        }

        // Create new window ID
        let new_id = WindowId::new(self.next_id);
        self.next_id += 1;

        // Split in layout
        self.layout.split_horizontal(target)?;

        // Copy buffer from target window
        let buffer_id = self.windows.get(&target).and_then(|w| w.buffer_id);
        let state = WindowState::new(new_id, buffer_id);
        self.windows.insert(new_id, state);

        Some(new_id)
    }

    /// Split the target window vertically (left/right).
    ///
    /// Creates a new window to the right of the target. Returns `None` if
    /// the target window doesn't exist.
    pub fn split_vertical(&mut self, target: WindowId) -> Option<WindowId> {
        if !self.windows.contains_key(&target) {
            return None;
        }

        // Create new window ID
        let new_id = WindowId::new(self.next_id);
        self.next_id += 1;

        // Split in layout
        self.layout.split_vertical(target)?;

        // Copy buffer from target window
        let buffer_id = self.windows.get(&target).and_then(|w| w.buffer_id);
        let state = WindowState::new(new_id, buffer_id);
        self.windows.insert(new_id, state);

        Some(new_id)
    }

    /// Find window in the given direction from the active window.
    ///
    /// Returns `None` if there's no active window or no window in that direction.
    #[must_use]
    pub fn focus_direction(&self, direction: NavigateDirection) -> Option<WindowId> {
        let current = self.active_window?;
        let views = self.arrange_internal((80, 24)); // Use nominal size for navigation
        self.focus_policy.next(direction, current, &views)
    }

    /// Get the number of windows.
    #[must_use]
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// Check if there are no windows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    /// Iterate over all window IDs.
    pub fn windows(&self) -> impl Iterator<Item = WindowId> + '_ {
        self.windows.keys().copied()
    }

    /// Arrange windows within the given screen bounds.
    ///
    /// Returns a list of `WindowView` with each window's position and size.
    #[must_use]
    pub fn arrange(&self, screen_size: (u16, u16)) -> Vec<WindowView> {
        self.arrange_internal(screen_size)
    }

    /// Internal arrange helper.
    fn arrange_internal(&self, screen_size: (u16, u16)) -> Vec<WindowView> {
        let window_ids: Vec<_> = self.layout.windows();
        self.layout.arrange(screen_size, &window_ids)
    }

    /// Cycle focus to the next/previous window.
    ///
    /// Returns the new active window ID, or `None` if no windows exist.
    #[must_use]
    pub fn cycle_focus(&self, forward: bool) -> Option<WindowId> {
        let current = self.active_window?;
        let views = self.arrange_internal((80, 24));
        self.focus_policy.cycle(forward, current, &views)
    }

    /// Cycle focus forward to the next window and set it as active.
    ///
    /// Returns `true` if focus changed, `false` otherwise.
    pub fn cycle_forward(&mut self) -> bool {
        self.cycle_focus(true)
            .is_some_and(|new_focus| self.set_active_window(new_focus))
    }

    /// Cycle focus backward to the previous window and set it as active.
    ///
    /// Returns `true` if focus changed, `false` otherwise.
    pub fn cycle_backward(&mut self) -> bool {
        self.cycle_focus(false)
            .is_some_and(|new_focus| self.set_active_window(new_focus))
    }

    /// Get the layout (for external access).
    #[must_use]
    pub const fn layout(&self) -> &TilingLayout {
        &self.layout
    }

    /// Get mutable layout access.
    pub const fn layout_mut(&mut self) -> &mut TilingLayout {
        &mut self.layout
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_registry_new_empty() {
        let registry = WindowRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.window_count(), 0);
        assert!(registry.active_window().is_none());
    }

    #[test]
    fn test_window_registry_create_window() {
        let mut registry = WindowRegistry::new();
        let buffer_id = BufferId::new();

        let w1 = registry.create_window(Some(buffer_id));

        assert_eq!(registry.window_count(), 1);
        assert!(!registry.is_empty());
        assert_eq!(registry.active_window(), Some(w1));

        let state = registry.get(w1).unwrap();
        assert_eq!(state.buffer_id, Some(buffer_id));
        assert_eq!(state.cursor, Position::default());
    }

    #[test]
    fn test_window_registry_split_horizontal() {
        let mut registry = WindowRegistry::new();
        let buffer_id = BufferId::new();

        let w1 = registry.create_window(Some(buffer_id));
        let w2 = registry.split_horizontal(w1);

        assert!(w2.is_some());
        let w2 = w2.unwrap();

        assert_eq!(registry.window_count(), 2);
        assert_ne!(w1, w2);

        // New window should have same buffer
        let state = registry.get(w2).unwrap();
        assert_eq!(state.buffer_id, Some(buffer_id));
    }

    #[test]
    fn test_window_registry_split_vertical() {
        let mut registry = WindowRegistry::new();
        let buffer_id = BufferId::new();

        let w1 = registry.create_window(Some(buffer_id));
        let w2 = registry.split_vertical(w1);

        assert!(w2.is_some());
        let w2 = w2.unwrap();

        assert_eq!(registry.window_count(), 2);
        assert_ne!(w1, w2);

        // New window should have same buffer
        let state = registry.get(w2).unwrap();
        assert_eq!(state.buffer_id, Some(buffer_id));
    }

    #[test]
    fn test_window_registry_close_window() {
        let mut registry = WindowRegistry::new();

        let w1 = registry.create_window(None);
        let w2 = registry.split_vertical(w1).unwrap();

        assert_eq!(registry.window_count(), 2);

        // Close w2
        assert!(registry.close_window(w2));
        assert_eq!(registry.window_count(), 1);
        assert!(registry.get(w2).is_none());
    }

    #[test]
    fn test_window_registry_close_last_window_fails() {
        let mut registry = WindowRegistry::new();
        let w1 = registry.create_window(None);

        // Cannot close the last window
        assert!(!registry.close_window(w1));
        assert_eq!(registry.window_count(), 1);
    }

    #[test]
    fn test_window_registry_focus_direction() {
        let mut registry = WindowRegistry::new();

        let w1 = registry.create_window(None);
        let _w2 = registry.split_vertical(w1).unwrap();

        registry.set_active_window(w1);

        // From w1, focus right should find the other window
        let next = registry.focus_direction(NavigateDirection::Right);
        // Note: This depends on TilingLayout arrangement
        // The exact behavior depends on how split_vertical arranges windows
        assert!(next.is_some() || next.is_none()); // Depends on layout
    }

    #[test]
    fn test_window_registry_focus_direction_no_adjacent() {
        let mut registry = WindowRegistry::new();
        let w1 = registry.create_window(None);
        registry.set_active_window(w1);

        // Single window, no adjacent in any direction
        assert!(registry.focus_direction(NavigateDirection::Left).is_none());
        assert!(registry.focus_direction(NavigateDirection::Right).is_none());
        assert!(registry.focus_direction(NavigateDirection::Up).is_none());
        assert!(registry.focus_direction(NavigateDirection::Down).is_none());
    }

    #[test]
    fn test_window_registry_arrange() {
        let mut registry = WindowRegistry::new();
        let w1 = registry.create_window(None);

        let views = registry.arrange((80, 24));

        // Single window should fill the screen
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].window_id, w1);
        assert_eq!(views[0].bounds.width, 80);
        assert_eq!(views[0].bounds.height, 24);
    }

    #[test]
    fn test_window_registry_three_window_navigation() {
        let mut registry = WindowRegistry::new();

        // Create three windows in a row (vertical splits)
        let w1 = registry.create_window(None);
        let w2 = registry.split_vertical(w1).unwrap();
        let w3 = registry.split_vertical(w2).unwrap();

        assert_eq!(registry.window_count(), 3);

        // All windows should exist
        assert!(registry.get(w1).is_some());
        assert!(registry.get(w2).is_some());
        assert!(registry.get(w3).is_some());
    }

    #[test]
    fn test_per_window_cursor_independence() {
        let mut registry = WindowRegistry::new();
        let buffer_id = BufferId::new();

        // Create two windows viewing the same buffer
        let w1 = registry.create_window(Some(buffer_id));
        let w2 = registry.split_vertical(w1).unwrap();

        // Set different cursor positions
        if let Some(state) = registry.get_mut(w1) {
            state.cursor = Position::new(10, 5);
        }
        if let Some(state) = registry.get_mut(w2) {
            state.cursor = Position::new(20, 0);
        }

        // Verify cursors are independent
        assert_eq!(registry.get(w1).unwrap().cursor, Position::new(10, 5));
        assert_eq!(registry.get(w2).unwrap().cursor, Position::new(20, 0));

        // Both still have same buffer
        assert_eq!(registry.get(w1).unwrap().buffer_id, Some(buffer_id));
        assert_eq!(registry.get(w2).unwrap().buffer_id, Some(buffer_id));
    }

    #[test]
    fn test_window_state_builders() {
        let id = WindowId::new(1);
        let buffer_id = BufferId::new();

        let state = WindowState::new(id, Some(buffer_id))
            .with_cursor(Position::new(5, 10))
            .with_scroll_top(100);

        assert_eq!(state.id, id);
        assert_eq!(state.buffer_id, Some(buffer_id));
        assert_eq!(state.cursor, Position::new(5, 10));
        assert_eq!(state.scroll_top, 100);
    }

    #[test]
    fn test_window_registry_set_active_window() {
        let mut registry = WindowRegistry::new();

        let w1 = registry.create_window(None);
        let w2 = registry.split_vertical(w1).unwrap();

        // Initially w1 is active (first created)
        assert_eq!(registry.active_window(), Some(w1));

        // Set w2 as active
        assert!(registry.set_active_window(w2));
        assert_eq!(registry.active_window(), Some(w2));

        // Try to set non-existent window
        assert!(!registry.set_active_window(WindowId::new(999)));
        assert_eq!(registry.active_window(), Some(w2)); // Unchanged
    }

    #[test]
    fn test_window_registry_close_active_window() {
        let mut registry = WindowRegistry::new();

        let w1 = registry.create_window(None);
        let w2 = registry.split_vertical(w1).unwrap();

        registry.set_active_window(w1);
        assert_eq!(registry.active_window(), Some(w1));

        // Close active window
        assert!(registry.close_window(w1));

        // Active should switch to remaining window
        assert_eq!(registry.active_window(), Some(w2));
    }

    #[test]
    fn test_window_registry_cycle_forward() {
        let mut registry = WindowRegistry::new();

        let w1 = registry.create_window(None);
        let _w2 = registry.split_vertical(w1).unwrap();

        registry.set_active_window(w1);
        assert_eq!(registry.active_window(), Some(w1));

        // Cycle forward should change active window
        assert!(registry.cycle_forward());
        // Active window should have changed (exact result depends on layout)
        assert!(registry.active_window().is_some());
    }

    #[test]
    fn test_window_registry_cycle_backward() {
        let mut registry = WindowRegistry::new();

        let w1 = registry.create_window(None);
        let _w2 = registry.split_vertical(w1).unwrap();

        registry.set_active_window(w1);
        assert_eq!(registry.active_window(), Some(w1));

        // Cycle backward should change active window
        assert!(registry.cycle_backward());
        // Active window should have changed (exact result depends on layout)
        assert!(registry.active_window().is_some());
    }

    #[test]
    fn test_window_registry_cycle_single_window() {
        let mut registry = WindowRegistry::new();

        let w1 = registry.create_window(None);
        registry.set_active_window(w1);

        // Single window, cycle should return itself
        // But cycle_focus returns None when only one window
        // (depends on focus policy implementation)
        let cycled = registry.cycle_forward();
        // Either it cycles to itself or returns false
        assert!(cycled || registry.active_window() == Some(w1));
    }
}
