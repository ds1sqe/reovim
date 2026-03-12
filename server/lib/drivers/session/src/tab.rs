//! Tab page management for session driver.
//!
//! Provides [`TabPage`] and [`TabPageSet`] for grouping window layouts.
//! Each tab page owns its own compositor and window layout, allowing
//! users to organize multiple window configurations within a session.
//!
//! # Architecture (#401)
//!
//! ```text
//! TabPageSet
//! ├── TabPage 0 (active)
//! │   ├── compositor: Option<Box<dyn RootCompositor>>
//! │   └── windows: WindowLayout
//! ├── TabPage 1
//! │   ├── compositor: Option<Box<dyn RootCompositor>>
//! │   └── windows: WindowLayout
//! └── ...
//! ```
//!
//! Each tab is a self-contained "workspace" — switching tabs swaps the
//! entire visible layout, compositor state, and window-to-cursor mappings.

use {reovim_driver_display::layout::RootCompositor, reovim_kernel::api::v1::TabId};

use crate::WindowLayout;

/// A single tab page containing its own window layout and compositor.
///
/// Analogous to vim's `:tabnew` — each tab is an independent workspace
/// with its own set of windows and split configuration.
pub struct TabPage {
    /// Unique tab identifier.
    id: TabId,
    /// Human-readable label (e.g., "1", "main", user-assigned name).
    label: String,
    /// Compositor for this tab's window layout (None = no production compositor).
    compositor: Option<Box<dyn RootCompositor>>,
    /// Window-to-cursor mapping for this tab.
    windows: WindowLayout,
}

impl std::fmt::Debug for TabPage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TabPage")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("has_compositor", &self.compositor.is_some())
            .field("windows", &self.windows)
            .finish()
    }
}

impl Clone for TabPage {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            label: self.label.clone(),
            compositor: self.compositor.as_ref().map(|c| c.boxed_clone()),
            windows: self.windows.clone(),
        }
    }
}

impl TabPage {
    /// Create a new tab page with the given label.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            id: TabId::new(),
            label: label.into(),
            compositor: None,
            windows: WindowLayout::empty(),
        }
    }

    /// Create a tab page with a specific ID (for testing or restoration).
    #[must_use]
    pub fn with_id(id: TabId, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            compositor: None,
            windows: WindowLayout::empty(),
        }
    }

    /// Get the tab's unique identifier.
    #[must_use]
    pub const fn id(&self) -> TabId {
        self.id
    }

    /// Get the tab's label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Set the tab's label.
    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
    }

    /// Get a reference to the compositor.
    #[must_use]
    pub const fn compositor(&self) -> &Option<Box<dyn RootCompositor>> {
        &self.compositor
    }

    /// Get a mutable reference to the compositor.
    pub fn compositor_mut(&mut self) -> &mut Option<Box<dyn RootCompositor>> {
        &mut self.compositor
    }

    /// Set the compositor for this tab.
    pub fn set_compositor(&mut self, compositor: Option<Box<dyn RootCompositor>>) {
        self.compositor = compositor;
    }

    /// Get a reference to the window layout.
    #[must_use]
    pub const fn windows(&self) -> &WindowLayout {
        &self.windows
    }

    /// Get a mutable reference to the window layout.
    pub const fn windows_mut(&mut self) -> &mut WindowLayout {
        &mut self.windows
    }
}

/// Ordered collection of tab pages with one active tab.
///
/// Manages the lifecycle of tabs: creation, deletion, navigation.
/// Always maintains at least one tab (the initial tab).
///
/// # Invariants
///
/// - `tabs` is never empty (always >= 1 tab)
/// - `active_index` is always valid (< `tabs.len()`)
#[derive(Debug, Clone)]
pub struct TabPageSet {
    /// All tab pages in order.
    tabs: Vec<TabPage>,
    /// Index of the currently active tab.
    active_index: usize,
}

impl TabPageSet {
    /// Create a new tab set with one initial tab.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tabs: vec![TabPage::new("1")],
            active_index: 0,
        }
    }

    /// Get the active tab.
    #[must_use]
    pub fn active_tab(&self) -> &TabPage {
        &self.tabs[self.active_index]
    }

    /// Get the active tab mutably.
    pub fn active_tab_mut(&mut self) -> &mut TabPage {
        &mut self.tabs[self.active_index]
    }

    /// Get the active tab's index (0-based).
    #[must_use]
    pub const fn active_index(&self) -> usize {
        self.active_index
    }

    /// Get the active tab's ID.
    #[must_use]
    pub fn active_tab_id(&self) -> TabId {
        self.tabs[self.active_index].id
    }

    /// Get the number of tabs.
    #[must_use]
    pub const fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Get all tabs as a slice.
    #[must_use]
    pub fn tabs(&self) -> &[TabPage] {
        &self.tabs
    }

    /// Create a new tab after the active tab.
    ///
    /// The new tab becomes active. Returns its ID.
    pub fn new_tab(&mut self) -> TabId {
        let label = (self.tabs.len() + 1).to_string();
        let tab = TabPage::new(label);
        let id = tab.id;
        let insert_pos = self.active_index + 1;
        self.tabs.insert(insert_pos, tab);
        self.active_index = insert_pos;
        id
    }

    /// Close the active tab.
    ///
    /// Returns `true` if the tab was closed. Returns `false` if this is
    /// the last tab (cannot close the only tab).
    ///
    /// After closing, the next tab becomes active (or the previous if
    /// the closed tab was the last one).
    pub fn close_tab(&mut self) -> bool {
        if self.tabs.len() <= 1 {
            return false;
        }
        self.tabs.remove(self.active_index);
        // Adjust active_index: if we removed the last tab, move to previous
        if self.active_index >= self.tabs.len() {
            self.active_index = self.tabs.len() - 1;
        }
        true
    }

    /// Switch to the next tab (wraps around).
    ///
    /// Returns the new active tab's ID.
    pub fn next_tab(&mut self) -> TabId {
        self.active_index = (self.active_index + 1) % self.tabs.len();
        self.tabs[self.active_index].id
    }

    /// Switch to the previous tab (wraps around).
    ///
    /// Returns the new active tab's ID.
    pub fn prev_tab(&mut self) -> TabId {
        self.active_index = if self.active_index == 0 {
            self.tabs.len() - 1
        } else {
            self.active_index - 1
        };
        self.tabs[self.active_index].id
    }

    /// Switch to tab at the given 0-based index.
    ///
    /// Returns the tab's ID, or `None` if the index is out of bounds.
    pub fn goto_tab(&mut self, index: usize) -> Option<TabId> {
        if index < self.tabs.len() {
            self.active_index = index;
            Some(self.tabs[self.active_index].id)
        } else {
            None
        }
    }

    /// Find a tab by its ID.
    ///
    /// Returns the tab's index and a reference to it.
    #[must_use]
    pub fn find_tab(&self, id: TabId) -> Option<(usize, &TabPage)> {
        self.tabs.iter().enumerate().find(|(_, tab)| tab.id == id)
    }

    /// Get tab info for protocol serialization.
    ///
    /// Returns (`tab_id`, label, `is_active`) for each tab.
    #[must_use]
    pub fn tab_info(&self) -> Vec<(TabId, &str, bool)> {
        self.tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| (tab.id, tab.label.as_str(), i == self.active_index))
            .collect()
    }

    /// Get the active tab's window layout.
    #[must_use]
    pub fn active_windows(&self) -> &WindowLayout {
        &self.tabs[self.active_index].windows
    }

    /// Get the active tab's window layout mutably.
    pub fn active_windows_mut(&mut self) -> &mut WindowLayout {
        &mut self.tabs[self.active_index].windows
    }

    /// Get the active tab's compositor.
    #[must_use]
    pub fn active_compositor(&self) -> &Option<Box<dyn RootCompositor>> {
        &self.tabs[self.active_index].compositor
    }

    /// Get the active tab's compositor mutably.
    pub fn active_compositor_mut(&mut self) -> &mut Option<Box<dyn RootCompositor>> {
        &mut self.tabs[self.active_index].compositor
    }
}

impl Default for TabPageSet {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(test)]
#[path = "tab_tests.rs"]
mod tests;
