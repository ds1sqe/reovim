#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
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
mod tests {
    use super::*;

    #[test]
    fn extension_kind() {
        let ext = MicroscopeExtension::new();
        assert_eq!(ext.kind(), "microscope");
    }

    #[test]
    fn initially_inactive() {
        let ext = MicroscopeExtension::new();
        assert!(!ext.is_active());
    }

    #[test]
    fn default_impl() {
        let ext = MicroscopeExtension::default();
        assert!(!ext.is_active());
    }

    #[test]
    fn apply_notification_active() {
        let mut ext = MicroscopeExtension::new();
        ext.apply_notification(
            r#"{"active":true,"query":"main","cursor":4,"selected":0,"scrollOffset":0,"pickerName":"files","pickerTitle":"Files","prompt":"> ","items":[{"display":"main.rs"}],"totalCount":10,"matchedCount":1}"#,
        );
        assert!(ext.is_active());
        assert_eq!(ext.data.query, "main");
        assert_eq!(ext.data.cursor, 4);
        assert_eq!(ext.data.picker_title, "Files");
        assert_eq!(ext.data.items.len(), 1);
        assert_eq!(ext.data.total_count, 10);
    }

    #[test]
    fn apply_notification_inactive() {
        let mut ext = MicroscopeExtension::new();
        ext.apply_notification(r#"{"active":true,"query":"x","cursor":1,"selected":0,"scrollOffset":0,"pickerName":"f","pickerTitle":"F","prompt":"> ","items":[],"totalCount":0,"matchedCount":0}"#);
        assert!(ext.is_active());

        ext.apply_notification(r#"{"active":false}"#);
        assert!(!ext.is_active());
    }

    #[test]
    fn apply_notification_invalid_json() {
        let mut ext = MicroscopeExtension::new();
        ext.apply_notification("not json");
        assert!(!ext.is_active());
    }

    #[test]
    fn apply_notification_with_preview() {
        let mut ext = MicroscopeExtension::new();
        ext.apply_notification(
            r#"{"active":true,"query":"","cursor":0,"selected":0,"scrollOffset":0,"pickerName":"f","pickerTitle":"F","prompt":"> ","items":[],"totalCount":0,"matchedCount":0,"preview":{"lines":["fn main()","{}"],"highlightLine":0}}"#,
        );
        assert!(ext.data.preview.is_some());
        let preview = ext.data.preview.as_ref().unwrap();
        assert_eq!(preview.lines.len(), 2);
        assert_eq!(preview.highlight_line, Some(0));
    }

    #[test]
    fn apply_notification_with_item_detail() {
        let mut ext = MicroscopeExtension::new();
        ext.apply_notification(
            r#"{"active":true,"query":"","cursor":0,"selected":0,"scrollOffset":0,"pickerName":"f","pickerTitle":"F","prompt":"> ","items":[{"display":"main.rs","detail":"src/main.rs"}],"totalCount":1,"matchedCount":1}"#,
        );
        assert_eq!(ext.data.items.len(), 1);
        assert_eq!(ext.data.items[0].detail.as_deref(), Some("src/main.rs"));
    }

    #[test]
    fn cursor_position_inactive() {
        let ext = MicroscopeExtension::new();
        assert!(ext.cursor_position(80, 24).is_none());
    }

    #[test]
    fn cursor_position_active() {
        let mut ext = MicroscopeExtension::new();
        ext.apply_notification(
            r#"{"active":true,"query":"ab","cursor":2,"selected":0,"scrollOffset":0,"pickerName":"f","pickerTitle":"F","prompt":"> ","items":[],"totalCount":0,"matchedCount":0}"#,
        );
        let pos = ext.cursor_position(80, 24);
        assert!(pos.is_some());
    }

    #[test]
    fn cursor_position_too_small_screen() {
        let mut ext = MicroscopeExtension::new();
        ext.apply_notification(
            r#"{"active":true,"query":"","cursor":0,"selected":0,"scrollOffset":0,"pickerName":"f","pickerTitle":"F","prompt":"> ","items":[],"totalCount":0,"matchedCount":0}"#,
        );
        // Very small screen.
        let pos = ext.cursor_position(10, 3);
        assert!(pos.is_none());
    }
}
