#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Microscope fuzzy finder native `ClientModule`.
//!
//! Renders a Helix-style bottom-anchored fuzzy finder overlay.
//! Receives state from the server via `on_notification()` and
//! renders through `RenderSurface`.
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

use reovim_client_driver::{
    ChromePosition, ClientModule, ClientModuleError, ModuleContext, PlatformCapabilities,
    ProbeResult, Rect, RenderSurface, Version,
};

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
    icon: Option<String>,
}

/// Deserialized preview content.
#[derive(Debug)]
struct PreviewData {
    lines: Vec<String>,
    highlight_line: Option<usize>,
    highlights: Vec<PreviewHighlightData>,
}

/// A syntax highlight span in preview content.
#[derive(Debug)]
struct PreviewHighlightData {
    line: u16,
    col_start: u16,
    col_end: u16,
    category: String,
}

/// Module kind identifier.
const KIND: &str = "microscope";

/// Microscope fuzzy finder chrome module.
///
/// Renders the fuzzy finder overlay when active.
pub struct MicroscopeModule {
    data: MicroscopeData,
}

impl MicroscopeModule {
    /// Create a new inactive module.
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: MicroscopeData::default(),
        }
    }
}

impl Default for MicroscopeModule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl ClientModule for MicroscopeModule {
    fn id(&self) -> &'static str {
        KIND
    }

    fn kind(&self) -> &'static str {
        KIND
    }

    fn name(&self) -> &'static str {
        "Microscope"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ClientModuleError> {
        Ok(())
    }

    fn has_chrome(&self) -> bool {
        true
    }

    fn chrome_position(&self) -> ChromePosition {
        ChromePosition::Overlay
    }

    fn chrome_priority(&self) -> u16 {
        80
    }

    #[allow(clippy::cast_possible_truncation)]
    fn on_notification(&mut self, data: &str) {
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
                        icon: item["icon"].as_str().map(str::to_owned),
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
            let highlights = p
                .get("highlights")
                .and_then(serde_json::Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|h| {
                            Some(PreviewHighlightData {
                                line: u16::try_from(h["line"].as_u64()?).ok()?,
                                col_start: u16::try_from(h["colStart"].as_u64()?).ok()?,
                                col_end: u16::try_from(h["colEnd"].as_u64()?).ok()?,
                                category: h["category"].as_str()?.to_owned(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            Some(PreviewData {
                lines,
                highlight_line: p["highlightLine"].as_u64().map(|n| n as usize),
                highlights,
            })
        });
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

    fn chrome_render(
        &self,
        surface: &mut dyn RenderSurface,
        _bounds: Rect,
        _caps: &dyn PlatformCapabilities,
    ) {
        if !self.data.active {
            return;
        }

        let (width, height) = surface.size();
        let bounds = LayoutBounds::calculate(width, height);

        if bounds.total_height < layout::MIN_HEIGHT {
            return;
        }

        render::render_microscope(surface, &self.data, &bounds);
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
