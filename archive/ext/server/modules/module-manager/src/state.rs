//! Per-client module manager state (#622).
//!
//! Stored in the session's `ExtensionMap` as a `SessionExtension`.

use reovim_driver_text_session::SessionExtension;

/// Module status indicator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleStatus {
    /// Module is loaded and running.
    Loaded,
    /// Module is disabled by config.
    Disabled,
    /// Module failed to initialize.
    Failed,
}

impl ModuleStatus {
    /// Display indicator for the status.
    #[must_use]
    pub const fn indicator(self) -> &'static str {
        match self {
            Self::Loaded => "[*]",
            Self::Disabled => "[-]",
            Self::Failed => "[!]",
        }
    }

    /// Label for the status.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Loaded => "loaded",
            Self::Disabled => "disabled",
            Self::Failed => "failed",
        }
    }
}

/// Filter view for the module list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleFilter {
    /// Show all modules.
    All,
    /// Show only loaded modules.
    Loaded,
    /// Show only disabled modules.
    Disabled,
    /// Show only failed modules.
    Failed,
}

impl ModuleFilter {
    /// Cycle to the next filter.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::All => Self::Loaded,
            Self::Loaded => Self::Disabled,
            Self::Disabled => Self::Failed,
            Self::Failed => Self::All,
        }
    }

    /// Display label for the filter.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Loaded => "Loaded",
            Self::Disabled => "Disabled",
            Self::Failed => "Failed",
        }
    }

    /// Check if a module status matches this filter.
    #[must_use]
    pub const fn matches(self, status: ModuleStatus) -> bool {
        match self {
            Self::All => true,
            Self::Loaded => matches!(status, ModuleStatus::Loaded),
            Self::Disabled => matches!(status, ModuleStatus::Disabled),
            Self::Failed => matches!(status, ModuleStatus::Failed),
        }
    }
}

/// Entry for a single module in the manager list.
#[derive(Debug, Clone)]
pub struct ModuleEntry {
    /// Module ID.
    pub id: String,
    /// Module version string.
    pub version: String,
    /// Current status.
    pub status: ModuleStatus,
    /// Optional failure reason.
    pub reason: Option<String>,
}

/// Per-client module manager state.
pub struct ModuleManagerState {
    /// Whether the module manager panel is active/visible.
    pub active: bool,
    /// All modules (unfiltered).
    pub modules: Vec<ModuleEntry>,
    /// Current selection index (within filtered view).
    pub selected: usize,
    /// Current filter.
    pub filter: ModuleFilter,
    /// Whether the detail sidebar is visible.
    pub detail_visible: bool,
}

impl ModuleManagerState {
    /// Create an inactive state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            active: false,
            modules: Vec::new(),
            selected: 0,
            filter: ModuleFilter::All,
            detail_visible: false,
        }
    }

    /// Get the filtered module list.
    #[must_use]
    pub fn filtered(&self) -> Vec<&ModuleEntry> {
        self.modules
            .iter()
            .filter(|m| self.filter.matches(m.status))
            .collect()
    }

    /// Move selection to the next item.
    pub fn next(&mut self) {
        let count = self.filtered().len();
        if count > 0 {
            self.selected = (self.selected + 1) % count;
        }
    }

    /// Move selection to the previous item.
    pub fn prev(&mut self) {
        let count = self.filtered().len();
        if count > 0 {
            self.selected = if self.selected == 0 {
                count - 1
            } else {
                self.selected - 1
            };
        }
    }

    /// Cycle to the next filter view.
    pub const fn toggle_filter(&mut self) {
        self.filter = self.filter.next();
        // Reset selection when filter changes
        self.selected = 0;
    }

    /// Toggle detail sidebar.
    pub const fn toggle_detail(&mut self) {
        self.detail_visible = !self.detail_visible;
    }

    /// Get the currently selected module entry (in filtered view).
    #[must_use]
    pub fn selected_entry(&self) -> Option<&ModuleEntry> {
        self.filtered().get(self.selected).copied()
    }
}

impl Default for ModuleManagerState {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionExtension for ModuleManagerState {
    fn create() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
