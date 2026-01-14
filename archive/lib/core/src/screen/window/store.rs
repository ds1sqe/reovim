//! Window store - single source of truth for window management
//!
//! This module provides a centralized store for all window state, eliminating
//! the previous redundant state between `Screen::windows`, `Screen::window_buffers`,
//! and the tab manager's split tree.

use std::collections::BTreeMap;

use crate::buffer::Buffer;

use super::{
    super::layout::{
        split::{NavigateDirection, SplitDirection, WindowRect},
        tab::{TabManager, TabPage},
    },
    Anchor, Viewport, Window, WindowConfig, WindowContentSource, WindowId,
};

/// Result of closing a window
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseResult {
    /// Window closed, editor continues
    Closed,
    /// Last window in tab closed, tab closed, editor continues
    TabClosed,
    /// Last window in last tab closed, editor should quit
    ShouldQuit,
}

/// Centralized window store - single source of truth for all window state
///
/// This replaces the previous redundant state:
/// - `Screen::windows` (`Vec<Window>`)
/// - `Screen::window_buffers` (`BTreeMap<usize, usize>`)
/// - Tab manager's `active_window_id`
///
/// Now windows are stored in a `BTreeMap` with their buffer assignments integrated
/// into the Window struct itself via `WindowContentSource`.
pub struct WindowStore {
    /// All windows indexed by ID
    windows: BTreeMap<WindowId, Window>,
    /// Tab manager for split tree structure
    tab_manager: TabManager,
    /// Next available window ID
    next_id: WindowId,
}

impl WindowStore {
    /// Create a new window store with an initial window
    #[must_use]
    pub fn new(initial_buffer_id: usize, width: u16, height: u16) -> Self {
        let initial_id = WindowId::new(0);

        let initial_window = Window {
            id: initial_id,
            source: WindowContentSource::FileBuffer {
                buffer_id: initial_buffer_id,
            },
            bounds: WindowRect::new(0, 0, width, height),
            z_order: 100,
            is_active: true,
            is_floating: false,
            viewport: Viewport::default(),
            config: WindowConfig::default(),
        };

        let mut windows = BTreeMap::new();
        windows.insert(initial_id, initial_window);

        let mut tab_manager = TabManager::new();
        tab_manager.init_with_window(initial_id.raw());

        Self {
            windows,
            tab_manager,
            next_id: WindowId::new(1),
        }
    }

    /// Get a reference to the active window
    #[must_use]
    pub fn active_window(&self) -> Option<&Window> {
        self.tab_manager
            .active_window_id()
            .map(WindowId::new)
            .and_then(|id| self.windows.get(&id))
    }

    /// Get a mutable reference to the active window
    pub fn active_window_mut(&mut self) -> Option<&mut Window> {
        self.tab_manager
            .active_window_id()
            .map(WindowId::new)
            .and_then(|id| self.windows.get_mut(&id))
    }

    /// Get the active window ID
    #[must_use]
    pub fn active_window_id(&self) -> Option<WindowId> {
        self.tab_manager.active_window_id().map(WindowId::new)
    }

    /// Get the active buffer ID
    #[must_use]
    pub fn active_buffer_id(&self) -> Option<usize> {
        self.active_window().and_then(Window::buffer_id)
    }

    /// Get a reference to a window by ID
    #[must_use]
    pub fn get(&self, id: WindowId) -> Option<&Window> {
        self.windows.get(&id)
    }

    /// Get a mutable reference to a window by ID
    pub fn get_mut(&mut self, id: WindowId) -> Option<&mut Window> {
        self.windows.get_mut(&id)
    }

    /// Iterate over all windows
    pub fn iter(&self) -> impl Iterator<Item = &Window> {
        self.windows.values()
    }

    /// Iterate over all windows mutably
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Window> {
        self.windows.values_mut()
    }

    /// Get all windows as a slice (sorted by z-order for rendering)
    #[must_use]
    pub fn windows_sorted_by_z_order(&self) -> Vec<&Window> {
        let mut windows: Vec<_> = self.windows.values().collect();
        windows.sort_by_key(|w| w.z_order);
        windows
    }

    /// Get all windows as a Vec (for compatibility with code expecting slices)
    ///
    /// This clones all windows. Prefer `iter()` when possible.
    #[must_use]
    pub fn windows_vec(&self) -> Vec<Window> {
        self.windows.values().cloned().collect()
    }

    /// Get the number of windows in the active tab
    #[must_use]
    pub fn window_count(&self) -> usize {
        self.tab_manager
            .active_tab()
            .map_or(0, TabPage::window_count)
    }

    /// Set the buffer ID for a specific window
    pub fn set_window_buffer(&mut self, window_id: WindowId, buffer_id: usize) {
        if let Some(window) = self.windows.get_mut(&window_id) {
            window.set_buffer_id(buffer_id);
        }
    }

    /// Set the buffer ID for the active window
    pub fn set_active_buffer(&mut self, buffer_id: usize) {
        if let Some(window) = self.active_window_mut() {
            window.set_buffer_id(buffer_id);
        }
    }

    /// Set the active window by ID
    ///
    /// Returns true if the window was found and activated.
    pub fn set_active_window(&mut self, window_id: WindowId) -> bool {
        if !self.windows.contains_key(&window_id) {
            return false;
        }

        // Update is_active flags on all windows
        for (id, window) in &mut self.windows {
            window.is_active = *id == window_id;
        }

        // Update tab manager
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.active_window_id = window_id.raw();
        }

        true
    }

    /// Switch active window with full cursor synchronization
    ///
    /// Performs the complete cursor handoff:
    /// 1. Saves current buffer cursor to old active window
    /// 2. Changes active window
    /// 3. Loads new window's cursor into buffer
    ///
    /// Returns the previous active window ID, or None if same window.
    pub fn switch_active_window(
        &mut self,
        window_id: WindowId,
        buffers: &mut BTreeMap<usize, Buffer>,
    ) -> Option<WindowId> {
        let current_active = self.active_window_id();

        // Early return if same window
        if current_active == Some(window_id) {
            return None;
        }

        // Step 1: Save cursor to old window
        if let Some(old_id) = current_active
            && let Some(old_window) = self.windows.get_mut(&old_id)
            && let Some(buffer_id) = old_window.buffer_id()
            && let Some(buffer) = buffers.get(&buffer_id)
        {
            old_window.viewport.cursor = buffer.cur;
            old_window.viewport.desired_col = buffer.desired_col;
        }

        // Step 2: Update is_active flags
        for (id, window) in &mut self.windows {
            window.is_active = *id == window_id;
        }
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.active_window_id = window_id.raw();
        }

        // Step 3: Load cursor from new window into buffer
        if let Some(new_window) = self.windows.get(&window_id)
            && let Some(buffer_id) = new_window.buffer_id()
            && let Some(buffer) = buffers.get_mut(&buffer_id)
        {
            buffer.cur = new_window.viewport.cursor;
            buffer.desired_col = new_window.viewport.desired_col;
        }

        current_active
    }

    /// Split the active window
    ///
    /// Returns the new window ID if successful.
    pub fn split(&mut self, direction: SplitDirection) -> Option<WindowId> {
        let new_id = self.next_id;
        self.next_id = WindowId::new(self.next_id.raw() + 1);

        let active_tab = self.tab_manager.active_tab_mut()?;
        let active_id = active_tab.active_window_id;

        // Get the current window's buffer and cursor info
        let (buffer_id, cursor, desired_col) = self
            .windows
            .get(&WindowId::new(active_id))
            .map(|w| (w.buffer_id().unwrap_or(0), w.viewport.cursor, w.viewport.desired_col))?;

        // Split in the tree
        active_tab.split(new_id.raw(), direction);

        // Create new window (will be positioned by update_layouts)
        let new_window = Window {
            id: new_id,
            source: WindowContentSource::FileBuffer { buffer_id },
            bounds: WindowRect::new(0, 0, 0, 0), // Will be set by layout
            z_order: 100,
            is_active: true, // New window becomes active
            is_floating: false,
            viewport: Viewport {
                scroll: Anchor { x: 0, y: 0 },
                cursor,
                desired_col,
            },
            config: WindowConfig::default(),
        };

        // Update old window's is_active
        if let Some(old_window) = self.windows.get_mut(&WindowId::new(active_id)) {
            old_window.is_active = false;
        }

        self.windows.insert(new_id, new_window);
        Some(new_id)
    }

    /// Close the active window
    ///
    /// Returns the close result indicating what action was taken.
    pub fn close(&mut self) -> CloseResult {
        let Some(tab) = self.tab_manager.active_tab_mut() else {
            return CloseResult::ShouldQuit;
        };

        let closing_id = WindowId::new(tab.active_window_id);
        let is_last_window = tab.close_window(closing_id.raw());

        // Remove window from store
        self.windows.remove(&closing_id);

        if is_last_window {
            if !self.tab_manager.close_tab() {
                return CloseResult::ShouldQuit;
            }
            return CloseResult::TabClosed;
        }

        // Update is_active flags after close
        if let Some(new_active) = self.tab_manager.active_window_id() {
            let new_id = WindowId::new(new_active);
            for (id, window) in &mut self.windows {
                window.is_active = *id == new_id;
            }
        }

        CloseResult::Closed
    }

    /// Close all windows except the active one
    pub fn close_others(&mut self) {
        let Some(tab) = self.tab_manager.active_tab_mut() else {
            return;
        };

        let active_id = WindowId::new(tab.active_window_id);
        let all_ids: Vec<WindowId> = tab.window_ids().into_iter().map(WindowId::new).collect();

        // Remove all other windows from store
        for id in &all_ids {
            if *id != active_id {
                self.windows.remove(id);
            }
        }

        // Reset the split tree to just the active window
        tab.root = crate::screen::layout::split::SplitNode::leaf(active_id.raw());
    }

    /// Navigate focus to an adjacent window
    ///
    /// Returns Some(true) if navigation occurred.
    pub fn navigate(&mut self, direction: NavigateDirection, editor_rect: WindowRect) -> bool {
        let Some(tab) = self.tab_manager.active_tab() else {
            return false;
        };

        let layouts = tab.calculate_layouts(editor_rect);
        let current_id = tab.active_window_id;

        if let Some(next_id) =
            crate::screen::layout::split::find_adjacent_window(current_id, direction, &layouts)
        {
            if let Some(tab_mut) = self.tab_manager.active_tab_mut() {
                tab_mut.active_window_id = next_id;
            }
            // Update is_active flags
            let next_id = WindowId::new(next_id);
            for (id, window) in &mut self.windows {
                window.is_active = *id == next_id;
            }
            return true;
        }

        false
    }

    /// Equalize window sizes
    pub fn equalize(&mut self) {
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.equalize();
        }
    }

    /// Resize the active window
    pub fn resize(&mut self, direction: SplitDirection, delta: f32) {
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.adjust_ratio_in_direction(direction, delta);
        }
    }

    /// Swap the active window with the window in the given direction
    pub fn swap(&mut self, direction: NavigateDirection, editor_rect: WindowRect) -> bool {
        let Some(tab) = self.tab_manager.active_tab() else {
            return false;
        };

        let layouts = tab.calculate_layouts(editor_rect);
        let current_id = tab.active_window_id;

        let Some(target_id) =
            crate::screen::layout::split::find_adjacent_window(current_id, direction, &layouts)
        else {
            return false;
        };

        if let Some(tab_mut) = self.tab_manager.active_tab_mut() {
            tab_mut.root.swap_windows(current_id, target_id)
        } else {
            false
        }
    }

    /// Update window layouts from the split tree
    ///
    /// This should be called after any operation that changes window positions.
    pub fn update_layouts(&mut self, editor_rect: WindowRect, tab_offset: u16) {
        let Some(tab) = self.tab_manager.active_tab() else {
            return;
        };

        // Adjust rect for tab line
        let adjusted_rect = WindowRect::new(
            editor_rect.x,
            editor_rect.y + tab_offset,
            editor_rect.width,
            editor_rect.height.saturating_sub(tab_offset),
        );

        let layouts = tab.calculate_layouts(adjusted_rect);
        let active_window_id = tab.active_window_id;

        // Track which windows are still in the layout
        let _layout_ids: std::collections::HashSet<WindowId> =
            layouts.iter().map(|l| WindowId::new(l.window_id)).collect();

        // Update existing windows or remove ones not in layout
        let mut to_remove = Vec::new();
        for (id, window) in &mut self.windows {
            if let Some(layout) = layouts.iter().find(|l| l.window_id == id.raw()) {
                window.bounds = WindowRect::new(
                    layout.rect.x,
                    layout.rect.y,
                    layout.rect.width,
                    layout.rect.height,
                );
                window.is_active = layout.window_id == active_window_id;
            } else if !window.is_floating {
                // Non-floating windows not in layout should be removed
                // (unless they're plugin windows which have different management)
                to_remove.push(*id);
            }
        }

        // Remove windows not in layout
        for id in to_remove {
            self.windows.remove(&id);
        }

        // Add any windows that exist in layout but not in store
        // (shouldn't happen normally, but handles edge cases)
        for layout in &layouts {
            let id = WindowId::new(layout.window_id);
            if !self.windows.contains_key(&id) {
                // This shouldn't happen in normal operation
                tracing::warn!(
                    "Window {} in layout but not in store - this is unexpected",
                    layout.window_id
                );
            }
        }
    }

    // === Tab Operations ===

    /// Create a new tab
    pub fn new_tab(&mut self, buffer_id: usize, width: u16, height: u16) -> usize {
        let new_id = self.next_id;
        self.next_id = WindowId::new(self.next_id.raw() + 1);

        let new_window = Window {
            id: new_id,
            source: WindowContentSource::FileBuffer { buffer_id },
            bounds: WindowRect::new(0, 0, width, height),
            z_order: 100,
            is_active: true,
            is_floating: false,
            viewport: Viewport::default(),
            config: WindowConfig::default(),
        };

        // Deactivate all other windows
        for window in self.windows.values_mut() {
            window.is_active = false;
        }

        self.windows.insert(new_id, new_window);
        self.tab_manager.new_tab(new_id.raw())
    }

    /// Close the current tab
    ///
    /// Returns true if there are still tabs remaining, false if editor should quit.
    pub fn close_tab(&mut self) -> bool {
        // Remove all windows from the current tab
        if let Some(tab) = self.tab_manager.active_tab() {
            for window_id in tab.window_ids() {
                self.windows.remove(&WindowId::new(window_id));
            }
        }

        if self.tab_manager.close_tab() {
            // Activate the new active tab's window
            if let Some(new_active) = self.tab_manager.active_window_id() {
                let new_id = WindowId::new(new_active);
                for (id, window) in &mut self.windows {
                    window.is_active = *id == new_id;
                }
            }
            true
        } else {
            false
        }
    }

    /// Switch to next tab
    pub fn next_tab(&mut self) {
        self.tab_manager.next_tab();
        self.update_active_state();
    }

    /// Switch to previous tab
    pub fn prev_tab(&mut self) {
        self.tab_manager.prev_tab();
        self.update_active_state();
    }

    /// Go to a specific tab
    pub fn goto_tab(&mut self, index: usize) {
        self.tab_manager.goto_tab(index);
        self.update_active_state();
    }

    /// Get tab info for display
    #[must_use]
    pub fn tab_info(&self) -> Vec<crate::screen::layout::tab::TabInfo> {
        self.tab_manager.tab_info()
    }

    /// Get the number of tabs
    #[must_use]
    pub fn tab_count(&self) -> usize {
        self.tab_manager.tab_count()
    }

    /// Get a reference to the tab manager
    #[must_use]
    pub const fn tab_manager(&self) -> &TabManager {
        &self.tab_manager
    }

    /// Get a mutable reference to the tab manager
    pub const fn tab_manager_mut(&mut self) -> &mut TabManager {
        &mut self.tab_manager
    }

    /// Update `is_active` state on all windows based on current active window
    fn update_active_state(&mut self) {
        if let Some(active_id) = self.tab_manager.active_window_id() {
            let active_id = WindowId::new(active_id);
            for (id, window) in &mut self.windows {
                window.is_active = *id == active_id;
            }
        }
    }

    /// Find window at a screen position (for mouse clicks)
    ///
    /// Returns the topmost window (highest z-order) containing the position.
    #[must_use]
    pub fn window_at_position(&self, x: u16, y: u16) -> Option<WindowId> {
        self.windows
            .iter()
            .filter(|(_, w)| w.contains_screen_position(x, y))
            .max_by_key(|(_, w)| w.z_order)
            .map(|(id, _)| *id)
    }
}

impl Default for WindowStore {
    fn default() -> Self {
        Self::new(0, 80, 24)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_store_creation() {
        let store = WindowStore::new(0, 100, 50);
        assert_eq!(store.window_count(), 1);
        assert!(store.active_window().is_some());
        assert_eq!(store.active_buffer_id(), Some(0));
    }

    #[test]
    fn test_split_window() {
        let mut store = WindowStore::new(0, 100, 50);
        let new_id = store.split(SplitDirection::Vertical);
        assert!(new_id.is_some());
        assert_eq!(store.window_count(), 2);
    }

    #[test]
    fn test_close_window() {
        let mut store = WindowStore::new(0, 100, 50);
        store.split(SplitDirection::Vertical);

        let result = store.close();
        assert_eq!(result, CloseResult::Closed);
        assert_eq!(store.window_count(), 1);
    }

    #[test]
    fn test_close_last_window() {
        let mut store = WindowStore::new(0, 100, 50);
        let result = store.close();
        assert_eq!(result, CloseResult::ShouldQuit);
    }

    #[test]
    fn test_set_active_window() {
        let mut store = WindowStore::new(0, 100, 50);
        let new_id = store.split(SplitDirection::Vertical).unwrap();

        // New window should be active
        assert_eq!(store.active_window_id(), Some(new_id));

        // Switch back to original
        assert!(store.set_active_window(WindowId::new(0)));
        assert_eq!(store.active_window_id(), Some(WindowId::new(0)));
    }
}
