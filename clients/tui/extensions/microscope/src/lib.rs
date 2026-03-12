//! Microscope fuzzy finder TUI extension.
//!
//! Renders a Helix-style bottom-anchored fuzzy finder overlay.
//! Receives state from the server via `apply_notification()` and
//! renders through `RenderBackend`.
//!
//! # Layout
//!
//! ```text
//! +----------------------------------------------------------+
//! |                     Editor Content                        |
//! +----------------------------------------------------------+
//! | > query input                                   [3/142]  |
//! |----------------------------------------------------------|
//! | results (40%)              | preview (60%)               |
//! |  > src/main.rs             | 1  fn main() {              |
//! |    src/lib.rs              | 2      println!("hello");   |
//! +----------------------------+-----------------------------+
//! ```

mod layout;
mod render;

use reovim_driver_display::render_backend::{RenderBackend, TuiExtension};

use crate::layout::LayoutBounds;

/// Deserialized microscope state from server notification.
#[derive(Debug, Default)]
struct MicroscopeData {
    active: bool,
    query: String,
    cursor: usize,
    selected: usize,
    scroll_offset: usize,
    picker_title: String,
    prompt: String,
    items: Vec<ItemData>,
    total_count: u32,
    matched_count: u32,
    preview: Option<PreviewData>,
}

/// Deserialized picker item.
#[derive(Debug)]
struct ItemData {
    display: String,
    detail: Option<String>,
}

/// Deserialized preview content.
#[derive(Debug)]
struct PreviewData {
    lines: Vec<String>,
    highlight_line: Option<usize>,
}

/// Microscope TUI extension.
///
/// Renders the fuzzy finder overlay when active.
pub struct MicroscopeExtension {
    data: MicroscopeData,
}

impl MicroscopeExtension {
    /// Create a new inactive extension.
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: MicroscopeData::default(),
        }
    }
}

impl Default for MicroscopeExtension {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiExtension for MicroscopeExtension {
    fn kind(&self) -> &'static str {
        "microscope"
    }

    fn is_active(&self) -> bool {
        self.data.active
    }

    #[allow(clippy::cast_possible_truncation)]
    fn apply_notification(&mut self, data: &str) {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(data) else {
            return;
        };

        let active = json["active"].as_bool().unwrap_or(false);
        if !active {
            self.data.active = false;
            return;
        }

        self.data.active = true;
        json["query"]
            .as_str()
            .unwrap_or("")
            .clone_into(&mut self.data.query);
        self.data.cursor = json["cursor"].as_u64().unwrap_or(0) as usize;
        self.data.selected = json["selected"].as_u64().unwrap_or(0) as usize;
        self.data.scroll_offset = json["scrollOffset"].as_u64().unwrap_or(0) as usize;
        json["pickerTitle"]
            .as_str()
            .unwrap_or("")
            .clone_into(&mut self.data.picker_title);
        json["prompt"]
            .as_str()
            .unwrap_or("> ")
            .clone_into(&mut self.data.prompt);
        self.data.total_count = json["totalCount"].as_u64().unwrap_or(0) as u32;
        self.data.matched_count = json["matchedCount"].as_u64().unwrap_or(0) as u32;

        self.data.items = json["items"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .map(|item| ItemData {
                        display: item["display"].as_str().unwrap_or("").to_owned(),
                        detail: item["detail"].as_str().map(str::to_owned),
                    })
                    .collect()
            })
            .unwrap_or_default();

        self.data.preview = json.get("preview").and_then(|p| {
            let lines = p["lines"]
                .as_array()?
                .iter()
                .filter_map(|l| l.as_str().map(str::to_owned))
                .collect();
            Some(PreviewData {
                lines,
                highlight_line: p["highlightLine"].as_u64().map(|n| n as usize),
            })
        });
    }

    fn render(&self, backend: &mut dyn RenderBackend) {
        let (width, height) = backend.size();
        let bounds = LayoutBounds::calculate(width, height);

        if bounds.total_height < layout::MIN_HEIGHT {
            return;
        }

        render::render_microscope(backend, &self.data, &bounds);
    }

    #[allow(clippy::cast_possible_truncation)]
    fn cursor_position(&self, terminal_width: u16, terminal_height: u16) -> Option<(u16, u16)> {
        if !self.data.active {
            return None;
        }

        let bounds = LayoutBounds::calculate(terminal_width, terminal_height);
        if bounds.total_height < layout::MIN_HEIGHT {
            return None;
        }

        // Cursor is at query input position.
        let prompt_len = self.data.prompt.len() as u16;
        let cursor_x = bounds.x + prompt_len + self.data.cursor as u16;
        let cursor_y = bounds.query_row;

        Some((cursor_x.min(bounds.x + bounds.width - 1), cursor_y))
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
