//! Completion state management
//!
//! State is managed by the runtime's event handler via EventBus events.

#![allow(dead_code)] // State methods are used by runtime event handlers

use super::item::CompletionItem;

/// State of the completion popup
#[derive(Debug, Clone, Default)]
pub struct CompletionState {
    /// Whether completion is currently active/visible
    pub active: bool,
    /// Current list of filtered completion items
    pub items: Vec<CompletionItem>,
    /// Currently selected item index
    pub selected_index: usize,
    /// The prefix used for filtering
    pub prefix: String,
    /// Column where completion started
    pub start_col: u16,
    /// Row where completion started
    pub start_row: u16,
}

impl CompletionState {
    /// Create a new empty completion state
    #[must_use]
    pub const fn new() -> Self {
        Self {
            active: false,
            items: Vec::new(),
            selected_index: 0,
            prefix: String::new(),
            start_col: 0,
            start_row: 0,
        }
    }

    /// Activate completion with items
    pub fn activate(
        &mut self,
        items: Vec<CompletionItem>,
        prefix: String,
        start_col: u16,
        start_row: u16,
    ) {
        self.active = !items.is_empty();
        self.items = items;
        self.selected_index = 0;
        self.prefix = prefix;
        self.start_col = start_col;
        self.start_row = start_row;
    }

    /// Dismiss completion popup
    pub fn dismiss(&mut self) {
        self.active = false;
        self.items.clear();
        self.selected_index = 0;
        self.prefix.clear();
    }

    /// Select next item in the list
    #[allow(clippy::missing_const_for_fn)] // Vec::is_empty/len not const-stable
    pub fn select_next(&mut self) {
        if !self.items.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.items.len();
        }
    }

    /// Select previous item in the list
    pub fn select_prev(&mut self) {
        if !self.items.is_empty() {
            self.selected_index = self
                .selected_index
                .checked_sub(1)
                .unwrap_or(self.items.len() - 1);
        }
    }

    /// Get currently selected item
    #[must_use]
    pub fn selected_item(&self) -> Option<&CompletionItem> {
        self.items.get(self.selected_index)
    }

    /// Check if completion is active and has items
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec::is_empty not const-stable
    pub fn is_visible(&self) -> bool {
        self.active && !self.items.is_empty()
    }

    /// Get the number of items
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec::len not const-stable
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Update filter prefix and re-filter items from original set
    pub fn update_prefix(&mut self, new_prefix: &str, original_items: &[CompletionItem]) {
        self.prefix.clear();
        self.prefix.push_str(new_prefix);

        // Re-filter from original items
        self.items = original_items
            .iter()
            .filter(|item| {
                item.filter_text()
                    .to_lowercase()
                    .starts_with(&new_prefix.to_lowercase())
            })
            .cloned()
            .collect();

        // Clamp selected index
        if self.selected_index >= self.items.len() {
            self.selected_index = self.items.len().saturating_sub(1);
        }

        // Dismiss if no items match
        if self.items.is_empty() {
            self.active = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_items() -> Vec<CompletionItem> {
        vec![
            CompletionItem::new("apple", "buffer"),
            CompletionItem::new("application", "buffer"),
            CompletionItem::new("banana", "buffer"),
        ]
    }

    #[test]
    fn test_new_state() {
        let state = CompletionState::new();
        assert!(!state.active);
        assert!(state.items.is_empty());
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_activate() {
        let mut state = CompletionState::new();
        let items = sample_items();
        state.activate(items, "app".to_string(), 5, 10);

        assert!(state.active);
        assert_eq!(state.items.len(), 3);
        assert_eq!(state.prefix, "app");
        assert_eq!(state.start_col, 5);
        assert_eq!(state.start_row, 10);
    }

    #[test]
    fn test_activate_empty_items() {
        let mut state = CompletionState::new();
        state.activate(vec![], "test".to_string(), 0, 0);
        assert!(!state.active); // Should not activate with empty items
    }

    #[test]
    fn test_dismiss() {
        let mut state = CompletionState::new();
        state.activate(sample_items(), "test".to_string(), 0, 0);
        state.dismiss();

        assert!(!state.active);
        assert!(state.items.is_empty());
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_select_next() {
        let mut state = CompletionState::new();
        state.activate(sample_items(), String::new(), 0, 0);

        assert_eq!(state.selected_index, 0);
        state.select_next();
        assert_eq!(state.selected_index, 1);
        state.select_next();
        assert_eq!(state.selected_index, 2);
        state.select_next();
        assert_eq!(state.selected_index, 0); // Wraps around
    }

    #[test]
    fn test_select_prev() {
        let mut state = CompletionState::new();
        state.activate(sample_items(), String::new(), 0, 0);

        assert_eq!(state.selected_index, 0);
        state.select_prev();
        assert_eq!(state.selected_index, 2); // Wraps around
        state.select_prev();
        assert_eq!(state.selected_index, 1);
    }

    #[test]
    fn test_selected_item() {
        let mut state = CompletionState::new();
        state.activate(sample_items(), String::new(), 0, 0);

        assert_eq!(state.selected_item().map(|i| &i.label), Some(&"apple".to_string()));
        state.select_next();
        assert_eq!(state.selected_item().map(|i| &i.label), Some(&"application".to_string()));
    }

    #[test]
    fn test_update_prefix() {
        let mut state = CompletionState::new();
        let items = sample_items();
        state.activate(items.clone(), String::new(), 0, 0);

        state.update_prefix("app", &items);
        assert_eq!(state.items.len(), 2); // "apple" and "application"
        assert!(state.active);

        state.update_prefix("appl", &items);
        assert_eq!(state.items.len(), 2); // Still both

        state.update_prefix("appli", &items);
        assert_eq!(state.items.len(), 1); // Only "application"

        state.update_prefix("xyz", &items);
        assert!(state.items.is_empty());
        assert!(!state.active); // Dismissed when no matches
    }
}
