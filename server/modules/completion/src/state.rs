//! Per-client completion popup state.
//!
//! Stored in the session's `ExtensionMap` as a `SessionExtension`.
//! Unlike microscope, the completion popup does NOT use `TextInputSink`
//! because it operates within insert mode where the buffer receives input.

use {
    reovim_driver_session::SessionExtension,
    reovim_subsys_completion::{CompletionItem, CompletionKind},
};

/// Snapshot of a completion item for bridge serialization.
///
/// Lightweight copy of driver `CompletionItem` fields needed by the UI
/// and the confirm handler.
#[derive(Debug, Clone)]
pub struct CompletionItemSnapshot {
    /// Primary display text.
    pub label: String,
    /// Text to insert on confirmation (may differ from label).
    pub insert_text: String,
    /// Whether `insert_text` uses snippet syntax (`$1`, `${2:default}`).
    pub is_snippet: bool,
    /// Kind abbreviation (e.g., "fn", "va", "kw").
    pub kind_abbrev: String,
    /// Kind icon (Nerd Font glyph).
    pub kind_icon: String,
    /// Kind for styling.
    pub kind: CompletionKind,
    /// Optional detail text (type signature, etc.).
    pub detail: Option<String>,
    /// Source that provided this item (e.g., "buffer", "lsp").
    pub source_id: String,
}

impl CompletionItemSnapshot {
    /// Create a snapshot from a driver `CompletionItem`.
    #[must_use]
    pub fn from_item(item: &CompletionItem) -> Self {
        Self {
            label: item.label.clone(),
            insert_text: item.insert_text.clone(),
            is_snippet: item.is_snippet,
            kind_abbrev: item.kind.abbreviation().to_owned(),
            kind_icon: item.kind.icon().to_owned(),
            kind: item.kind,
            detail: item.detail.clone(),
            source_id: item.source_id.to_owned(),
        }
    }
}

/// Per-client completion popup state.
///
/// Tracks whether the popup is visible, the filtered items, selection,
/// and the prefix that triggered completion.
#[derive(Debug)]
pub struct CompletionState {
    /// Whether the completion popup is visible.
    pub active: bool,
    /// Filtered and scored completion items.
    pub items: Vec<CompletionItemSnapshot>,
    /// Index of the selected item.
    pub selected: usize,
    /// The prefix text that triggered completion.
    pub prefix: String,
    /// Scroll offset for the item list.
    pub scroll_offset: usize,
}

impl CompletionState {
    /// Open the completion popup with items.
    pub fn open(&mut self, items: Vec<CompletionItemSnapshot>, prefix: &str) {
        self.active = true;
        self.items = items;
        self.selected = 0;
        self.scroll_offset = 0;
        prefix.clone_into(&mut self.prefix);
    }

    /// Close the completion popup.
    pub fn close(&mut self) {
        self.active = false;
        self.items.clear();
        self.selected = 0;
        self.scroll_offset = 0;
        self.prefix.clear();
    }

    /// Move to the next item (wrapping).
    pub const fn next(&mut self) {
        if !self.items.is_empty() {
            self.selected = (self.selected + 1) % self.items.len();
        }
    }

    /// Move to the previous item (wrapping).
    pub const fn prev(&mut self) {
        if !self.items.is_empty() {
            let count = self.items.len();
            self.selected = (self.selected + count - 1) % count;
        }
    }

    /// Get the currently selected item, if any.
    #[must_use]
    pub fn selected_item(&self) -> Option<&CompletionItemSnapshot> {
        if self.active {
            self.items.get(self.selected)
        } else {
            None
        }
    }
}

impl SessionExtension for CompletionState {
    fn create() -> Self {
        Self {
            active: false,
            items: Vec::new(),
            selected: 0,
            prefix: String::new(),
            scroll_offset: 0,
        }
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
