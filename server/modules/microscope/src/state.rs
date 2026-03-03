//! Per-client microscope state.
//!
//! Stored in the session's `ExtensionMap` as a `SessionExtension`.
//! Implements `TextInputSink` to receive character input from the
//! input routing system when the microscope is active.

use {
    reovim_driver_picker::PreviewContent,
    reovim_driver_session::{SessionExtension, TextInputSink},
};

/// Snapshot of a picker item for bridge serialization.
///
/// Lightweight copy of `PickerItem` fields needed by the UI,
/// without the `PickerData` payload.
#[derive(Debug, Clone)]
pub struct PickerItemSnapshot {
    /// Primary display text.
    pub display: String,
    /// Secondary detail text.
    pub detail: Option<String>,
    /// Icon character.
    pub icon: Option<char>,
}

/// Per-client microscope state.
///
/// Tracks the current picker, query, selection, and visible items.
/// Updated by the module's command handlers and read by the bridge
/// for serialization to clients.
#[derive(Debug)]
pub struct MicroscopeState {
    /// Whether the microscope UI is visible.
    pub active: bool,
    /// Current query text.
    pub query: String,
    /// Cursor position within the query (character index).
    pub cursor: usize,
    /// Index of the selected item in the visible list.
    pub selected: usize,
    /// Scroll offset for the item list.
    pub scroll_offset: usize,
    /// Name of the active picker.
    pub picker_name: String,
    /// Title of the active picker (shown in UI).
    pub picker_title: String,
    /// Prompt prefix (shown before query input).
    pub prompt: String,
    /// Visible items (snapshot for rendering).
    pub items: Vec<PickerItemSnapshot>,
    /// Total number of items (before filtering).
    pub total_count: u32,
    /// Number of items matching the current query.
    pub matched_count: u32,
    /// Preview content for the selected item.
    pub preview: Option<PreviewContent>,
}

impl SessionExtension for MicroscopeState {
    fn create() -> Self {
        Self {
            active: false,
            query: String::new(),
            cursor: 0,
            selected: 0,
            scroll_offset: 0,
            picker_name: String::new(),
            picker_title: String::new(),
            prompt: String::from("> "),
            items: Vec::new(),
            total_count: 0,
            matched_count: 0,
            preview: None,
        }
    }

    fn as_text_input_sink(&mut self) -> Option<&mut dyn TextInputSink> {
        if self.active { Some(self) } else { None }
    }
}

impl TextInputSink for MicroscopeState {
    fn insert_char(&mut self, ch: char) {
        // cursor is a character index, convert to byte offset for String::insert.
        let byte_pos = self
            .query
            .char_indices()
            .nth(self.cursor)
            .map_or(self.query.len(), |(i, _)| i);
        self.query.insert(byte_pos, ch);
        self.cursor += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_create_defaults() {
        let state = MicroscopeState::create();
        assert!(!state.active);
        assert!(state.query.is_empty());
        assert_eq!(state.cursor, 0);
        assert_eq!(state.selected, 0);
        assert_eq!(state.scroll_offset, 0);
        assert!(state.picker_name.is_empty());
        assert!(state.picker_title.is_empty());
        assert_eq!(state.prompt, "> ");
        assert!(state.items.is_empty());
        assert_eq!(state.total_count, 0);
        assert_eq!(state.matched_count, 0);
        assert!(state.preview.is_none());
    }

    #[test]
    fn state_debug() {
        let state = MicroscopeState::create();
        let debug = format!("{state:?}");
        assert!(debug.contains("MicroscopeState"));
    }

    #[test]
    fn text_input_sink_insert_char() {
        let mut state = MicroscopeState::create();
        state.active = true;

        TextInputSink::insert_char(&mut state, 'h');
        TextInputSink::insert_char(&mut state, 'i');
        assert_eq!(state.query, "hi");
        assert_eq!(state.cursor, 2);
    }

    #[test]
    fn text_input_sink_unicode() {
        let mut state = MicroscopeState::create();
        state.active = true;

        TextInputSink::insert_char(&mut state, '日');
        assert_eq!(state.query, "日");
        // Cursor is character index (1), not byte offset.
        assert_eq!(state.cursor, 1);
    }

    #[test]
    fn as_text_input_sink_active() {
        let mut state = MicroscopeState::create();
        state.active = true;
        assert!(SessionExtension::as_text_input_sink(&mut state).is_some());
    }

    #[test]
    fn as_text_input_sink_inactive() {
        let mut state = MicroscopeState::create();
        assert!(SessionExtension::as_text_input_sink(&mut state).is_none());
    }

    #[test]
    fn picker_item_snapshot_clone() {
        let snap = PickerItemSnapshot {
            display: "test.rs".to_owned(),
            detail: Some("src/test.rs".to_owned()),
            icon: Some('f'),
        };
        #[allow(clippy::redundant_clone)]
        let cloned = snap.clone();
        assert_eq!(cloned.display, "test.rs");
        assert_eq!(cloned.detail.as_deref(), Some("src/test.rs"));
        assert_eq!(cloned.icon, Some('f'));
    }

    #[test]
    fn picker_item_snapshot_debug() {
        let snap = PickerItemSnapshot {
            display: "x".to_owned(),
            detail: None,
            icon: None,
        };
        let debug = format!("{snap:?}");
        assert!(debug.contains("PickerItemSnapshot"));
    }

    #[test]
    fn insert_char_at_middle() {
        let mut state = MicroscopeState::create();
        state.active = true;
        state.query = "ac".to_owned();
        state.cursor = 1; // Between 'a' and 'c'.

        TextInputSink::insert_char(&mut state, 'b');
        assert_eq!(state.query, "abc");
        assert_eq!(state.cursor, 2);
    }

    #[test]
    fn insert_unicode_at_middle() {
        let mut state = MicroscopeState::create();
        state.active = true;
        state.query = "ac".to_owned();
        state.cursor = 1;

        TextInputSink::insert_char(&mut state, '日');
        assert_eq!(state.query, "a日c");
        assert_eq!(state.cursor, 2);
    }

    #[test]
    fn insert_multiple_unicode() {
        let mut state = MicroscopeState::create();
        state.active = true;

        TextInputSink::insert_char(&mut state, '日');
        TextInputSink::insert_char(&mut state, '本');
        TextInputSink::insert_char(&mut state, '語');
        assert_eq!(state.query, "日本語");
        assert_eq!(state.cursor, 3);
    }
}
