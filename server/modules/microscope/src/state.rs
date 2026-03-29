//! Per-client microscope state.
//!
//! Stored in the session's `ExtensionMap` as a `SessionExtension`.
//! Implements `TextInputSink` to receive character input from the
//! input routing system when the microscope is active.

use std::sync::Arc;

use {
    reovim_driver_picker::{
        PickerContext, PickerEngine, PickerItem, PickerRegistry, PreviewContent, PreviewHighlight,
        push_items,
    },
    reovim_driver_session::{SessionExtension, TextInputSink},
    reovim_driver_syntax::{SyntaxFactoryStore, language_id_from_path},
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
                options: Vec::new(),
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
        let mut ticks = 0;
        while ticks < 100 && self.engine.tick(10).running {
            ticks += 1;
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

        self.update_preview();
    }

    /// Fetch preview content for the currently selected item.
    ///
    /// Called after selection changes (next/prev) and after engine refresh
    /// to keep the preview panel in sync with the highlighted item.
    pub fn update_preview(&mut self) {
        if let Some(services) = &self.services
            && let Some(registry) = services.get::<PickerRegistry>()
            && let Some(picker) = registry.get(&self.picker_name)
            && self.selected < self.full_items.len()
        {
            self.preview = picker.preview(&self.full_items[self.selected], services);
            // Compute syntax highlights if the preview has a file path hint.
            if let Some(preview) = &mut self.preview {
                Self::apply_syntax_highlights(preview, services);
            }
        } else {
            self.preview = None;
        }
    }

    /// Apply syntax highlighting to preview content using tree-sitter.
    ///
    /// Looks up a syntax driver via `SyntaxFactoryStore`, parses the
    /// preview content, and converts byte-range annotations into
    /// line/column `PreviewHighlight` entries.
    fn apply_syntax_highlights(preview: &mut PreviewContent, services: &ServiceRegistry) {
        // Skip if no file path or too many lines (avoid blocking the event loop).
        const MAX_HIGHLIGHT_LINES: usize = 500;
        let Some(ref path) = preview.file_path else {
            return;
        };
        if preview.lines.len() > MAX_HIGHLIGHT_LINES {
            return;
        }
        let Some(lang_id) = language_id_from_path(path) else {
            return;
        };
        let Some(store) = services.get::<SyntaxFactoryStore>() else {
            return;
        };
        let Some(factory) = store.find(lang_id) else {
            return;
        };
        let Some(mut driver) = factory.create(lang_id) else {
            return;
        };

        // Build full content from lines (re-join with newlines).
        let content: String = preview.lines.join("\n");
        driver.parse(&content);

        let annotations = driver.highlights(0..content.len());

        // Build line-start byte offset table.
        let mut line_starts: Vec<usize> = Vec::with_capacity(preview.lines.len());
        let mut offset = 0;
        for line in &preview.lines {
            line_starts.push(offset);
            offset += line.len() + 1; // +1 for the newline
        }

        // Convert byte-range annotations to line/col highlights.
        preview.highlights.reserve(annotations.len());
        for ann in annotations {
            // Find the line containing the annotation start.
            let line_idx = match line_starts.binary_search(&ann.start_byte) {
                Ok(i) => i,
                Err(i) => i.saturating_sub(1),
            };
            if line_idx >= preview.lines.len() {
                continue;
            }
            let line_start = line_starts[line_idx];
            let line_len = preview.lines[line_idx].len();
            let col_start = ann.start_byte.saturating_sub(line_start);
            // Clamp end to the current line (don't span across lines).
            let col_end = ann.end_byte.saturating_sub(line_start).min(line_len);
            if col_start >= col_end {
                continue;
            }
            #[allow(clippy::cast_possible_truncation)]
            preview.highlights.push(PreviewHighlight {
                line: line_idx as u16,
                col_start: col_start as u16,
                col_end: col_end as u16,
                category: ann.category.as_str().to_owned(),
            });
        }
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
