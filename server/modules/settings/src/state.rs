//! Per-client settings panel state.
//!
//! `SettingsState` is a `SessionExtension` that stores the list of
//! settings visible in the interactive settings panel.

use reovim_driver_text_session::SessionExtension;

/// A flat item in the settings panel display list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlatItem {
    /// Section header showing a module/group name.
    SectionHeader {
        /// Display text for the section.
        title: String,
    },
    /// An individual setting entry.
    Setting {
        /// Option name (e.g., "tabstop").
        name: String,
        /// Human-readable description.
        description: String,
        /// Current value as a display string.
        value: String,
        /// The kind of value (bool, int, string, choice).
        kind: SettingKind,
    },
}

/// The kind of a setting value, for UI rendering hints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingKind {
    /// Boolean toggle.
    Bool,
    /// Integer value.
    Int,
    /// String value.
    String,
    /// Choice from a set of values.
    Choice,
}

/// Per-client settings panel state stored as a session extension.
#[derive(Debug, Default)]
pub struct SettingsState {
    /// Whether the panel is currently open.
    pub open: bool,
    /// Flat list of items (headers and settings interleaved).
    pub items: Vec<FlatItem>,
    /// Currently selected item index.
    pub selected_index: usize,
    /// Scroll offset for viewport.
    pub scroll_offset: usize,
}

impl SessionExtension for SettingsState {
    fn create() -> Self {
        Self::default()
    }
}

impl SettingsState {
    /// Move selection down by one item, skipping section headers.
    pub fn select_next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let mut next = self.selected_index + 1;
        while next < self.items.len() {
            if matches!(self.items[next], FlatItem::Setting { .. }) {
                self.selected_index = next;
                return;
            }
            next += 1;
        }
    }

    /// Move selection up by one item, skipping section headers.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn select_prev(&mut self) {
        if self.selected_index == 0 {
            return;
        }
        let mut prev = self.selected_index - 1;
        loop {
            if matches!(self.items[prev], FlatItem::Setting { .. }) {
                self.selected_index = prev;
                return;
            }
            if prev == 0 {
                break;
            }
            prev -= 1;
        }
    }

    /// Get the currently selected setting name, if a setting is selected.
    #[must_use]
    pub fn selected_setting_name(&self) -> Option<&str> {
        self.items.get(self.selected_index).and_then(|item| {
            if let FlatItem::Setting { name, .. } = item {
                Some(name.as_str())
            } else {
                None
            }
        })
    }

    /// Get the number of setting items (excluding headers).
    #[must_use]
    pub fn setting_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| matches!(item, FlatItem::Setting { .. }))
            .count()
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
