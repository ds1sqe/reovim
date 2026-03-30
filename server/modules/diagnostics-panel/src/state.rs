//! Per-client diagnostics panel state.
//!
//! Stored in the session's `ExtensionMap` as a `SessionExtension`.

use {reovim_driver_lsp::DiagnosticSeverity, reovim_driver_session::SessionExtension};

use crate::items::{PanelItem, PanelMode, SeverityFilter, SortOrder, sort_items};

/// Per-client diagnostics panel state.
pub struct DiagnosticsState {
    /// Whether the panel is visible.
    pub active: bool,
    /// Current panel mode.
    pub mode: PanelMode,
    /// Items currently displayed (filtered and sorted).
    pub items: Vec<PanelItem>,
    /// Index of the selected item.
    pub selected: usize,
    /// Scroll offset for the item list.
    pub scroll_offset: usize,
    /// Current sort order.
    pub sort_order: SortOrder,
    /// Current severity filter.
    pub severity_filter: SeverityFilter,
}

impl SessionExtension for DiagnosticsState {
    fn create() -> Self {
        Self {
            active: false,
            mode: PanelMode::Diagnostics,
            items: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            sort_order: SortOrder::BySeverity,
            severity_filter: SeverityFilter::all(),
        }
    }
}

impl DiagnosticsState {
    /// Set items, apply filter and sort, and reset selection.
    pub fn set_items(&mut self, mut items: Vec<PanelItem>) {
        items.retain(|item| self.severity_filter.allows(item.severity));
        sort_items(&mut items, self.sort_order);
        self.items = items;
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Select the next item (wraps around).
    pub const fn select_next(&mut self) {
        if !self.items.is_empty() {
            self.selected = (self.selected + 1) % self.items.len();
        }
    }

    /// Select the previous item (wraps around).
    pub fn select_prev(&mut self) {
        if !self.items.is_empty() {
            self.selected = self.selected.checked_sub(1).unwrap_or(self.items.len() - 1);
        }
    }

    /// Get the currently selected item.
    #[must_use]
    pub fn selected_item(&self) -> Option<&PanelItem> {
        self.items.get(self.selected)
    }

    /// Count items by severity.
    #[must_use]
    pub fn severity_counts(&self) -> SeverityCounts {
        let mut counts = SeverityCounts::default();
        for item in &self.items {
            match item.severity {
                DiagnosticSeverity::Error => counts.errors += 1,
                DiagnosticSeverity::Warning => counts.warnings += 1,
                DiagnosticSeverity::Information => counts.info += 1,
                DiagnosticSeverity::Hint => counts.hints += 1,
            }
        }
        counts
    }

    /// Re-apply filter and sort to existing items.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn refilter_and_sort(&mut self) {
        self.items
            .retain(|item| self.severity_filter.allows(item.severity));
        sort_items(&mut self.items, self.sort_order);
        if self.selected >= self.items.len() {
            self.selected = self.items.len().saturating_sub(1);
        }
    }
}

/// Severity count summary.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SeverityCounts {
    /// Number of errors.
    pub errors: u32,
    /// Number of warnings.
    pub warnings: u32,
    /// Number of informational diagnostics.
    pub info: u32,
    /// Number of hints.
    pub hints: u32,
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
