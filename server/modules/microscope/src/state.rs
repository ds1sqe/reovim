//! Per-client microscope state.
//!
//! Stored in the session's `ExtensionMap` as a `SessionExtension`.
//! Implements `TextInputSink` to receive character input from the
//! input routing system when the microscope is active.

use std::sync::Arc;

use {
    reovim_driver_picker::{PickerContext, PickerEngine, PickerItem, PickerRegistry, PreviewContent, push_items},
    reovim_driver_session::{SessionExtension, TextInputSink},
    reovim_kernel::api::v1::ServiceRegistry,
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
    /// Fuzzy matching engine (persists across keystrokes).
    pub engine: PickerEngine,
    /// Full matched items (with `PickerData` for action dispatch).
    pub full_items: Vec<PickerItem>,
    /// Service registry for dynamic picker re-fetch.
    pub services: Option<Arc<ServiceRegistry>>,
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
            engine: PickerEngine::new(),
            full_items: Vec::new(),
            services: None,
        }
    }

    fn as_text_input_sink(&mut self) -> Option<&mut dyn TextInputSink> {
        if self.active { Some(self) } else { None }
    }
}

impl std::fmt::Debug for MicroscopeState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MicroscopeState")
            .field("active", &self.active)
            .field("query", &self.query)
            .field("cursor", &self.cursor)
            .field("selected", &self.selected)
            .field("picker_name", &self.picker_name)
            .field("items_len", &self.items.len())
            .field("full_items_len", &self.full_items.len())
            .field("total_count", &self.total_count)
            .field("matched_count", &self.matched_count)
            .finish_non_exhaustive()
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
        self.refresh_engine();
    }
}

/// Maximum number of visible items returned from the engine.
const MAX_VISIBLE: usize = 200;

impl MicroscopeState {
    /// Sync engine pattern with current query, tick, and update visible items.
    ///
    /// Called after every query change (character insert, backspace) to
    /// refresh the fuzzy-matched result list from the engine.
    /// For dynamic pickers (e.g. grep), re-fetches items on every query change.
    pub fn refresh_engine(&mut self) {
        // For dynamic pickers, re-fetch items on every query change.
        if let Some(services) = &self.services
            && let Some(registry) = services.get::<PickerRegistry>()
            && let Some(picker) = registry.get(&self.picker_name)
            && !picker.is_static()
        {
            let ctx = PickerContext {
                cwd: std::env::current_dir().unwrap_or_default(),
                query: self.query.clone(),
                buffers: Vec::new(),
                commands: Vec::new(),
            };
            let items = picker.items(&ctx, services);
            self.engine.restart();
            if !items.is_empty() {
                let injector = self.engine.injector();
                push_items(&injector, items);
            }
        }

        self.engine.set_pattern(&self.query);
        // Tick until stable (bounded to avoid infinite loop).
        for _ in 0..100 {
            let status = self.engine.tick(10);
            if !status.running {
                break;
            }
        }
        // Update counts.
        self.total_count = self.engine.total_count();
        self.matched_count = self.engine.matched_count();
        // Update visible items (snapshots for bridge + full items for dispatch).
        let matched = self.engine.matched_items(MAX_VISIBLE);
        self.items = matched
            .iter()
            .map(|item| PickerItemSnapshot {
                display: item.display.clone(),
                detail: item.detail.clone(),
                icon: item.icon,
            })
            .collect();
        self.full_items = matched;
        // Clamp selection.
        if self.matched_count == 0 {
            self.selected = 0;
        } else {
            self.selected = self.selected.min(self.matched_count as usize - 1);
        }
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
        assert!(state.full_items.is_empty());
    }

    #[test]
    fn state_debug() {
        let state = MicroscopeState::create();
        let debug = format!("{state:?}");
        assert!(debug.contains("MicroscopeState"));
    }

    #[test]
    fn refresh_engine_empty() {
        let mut state = MicroscopeState::create();
        state.refresh_engine();
        assert_eq!(state.total_count, 0);
        assert_eq!(state.matched_count, 0);
        assert!(state.items.is_empty());
        assert!(state.full_items.is_empty());
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn refresh_engine_with_items() {
        use reovim_driver_picker::{PickerData, push_items};

        let mut state = MicroscopeState::create();
        let injector = state.engine.injector();
        push_items(
            &injector,
            vec![
                PickerItem {
                    display: "main.rs".to_owned(),
                    detail: None,
                    data: PickerData::Text("a".to_owned()),
                    icon: None,
                },
                PickerItem {
                    display: "lib.rs".to_owned(),
                    detail: Some("src/lib.rs".to_owned()),
                    data: PickerData::Text("b".to_owned()),
                    icon: Some('f'),
                },
            ],
        );
        state.refresh_engine();
        assert_eq!(state.total_count, 2);
        assert_eq!(state.matched_count, 2);
        assert_eq!(state.items.len(), 2);
        assert_eq!(state.full_items.len(), 2);
    }

    #[test]
    fn refresh_engine_clamps_selection() {
        use reovim_driver_picker::{PickerData, push_items};

        let mut state = MicroscopeState::create();
        state.selected = 10; // Out of bounds.
        let injector = state.engine.injector();
        push_items(
            &injector,
            vec![PickerItem {
                display: "only.rs".to_owned(),
                detail: None,
                data: PickerData::Text("x".to_owned()),
                icon: None,
            }],
        );
        state.refresh_engine();
        assert_eq!(state.selected, 0); // Clamped to max valid index.
    }

    #[test]
    fn insert_char_refreshes_engine() {
        use reovim_driver_picker::{PickerData, push_items};

        let mut state = MicroscopeState::create();
        state.active = true;
        let injector = state.engine.injector();
        push_items(
            &injector,
            vec![
                PickerItem {
                    display: "main.rs".to_owned(),
                    detail: None,
                    data: PickerData::Text("a".to_owned()),
                    icon: None,
                },
                PickerItem {
                    display: "lib.rs".to_owned(),
                    detail: None,
                    data: PickerData::Text("b".to_owned()),
                    icon: None,
                },
            ],
        );
        // Initial refresh to populate.
        state.refresh_engine();
        assert_eq!(state.matched_count, 2);

        // Type "main" to filter.
        TextInputSink::insert_char(&mut state, 'm');
        TextInputSink::insert_char(&mut state, 'a');
        TextInputSink::insert_char(&mut state, 'i');
        TextInputSink::insert_char(&mut state, 'n');
        // After typing, engine should have filtered.
        assert_eq!(state.matched_count, 1);
        assert_eq!(state.items[0].display, "main.rs");
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
