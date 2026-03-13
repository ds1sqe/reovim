//! Completion popup TUI extension.
//!
//! Displays a floating completion popup below the cursor position.
//! Shows completion items with kind abbreviation, label, and source.
//!
//! This crate is a self-contained TUI extension: it owns its state,
//! parses notifications, and renders through `RenderBackend`.
//! The engine has ZERO knowledge of this crate.

use {
    reovim_arch::Color,
    reovim_driver_display::{
        Style,
        render_backend::{RenderBackend, TuiExtension},
    },
};

/// Maximum number of visible items in the popup.
const MAX_VISIBLE_ITEMS: usize = 10;

/// Completion item for rendering.
#[derive(Debug, Clone)]
struct CompletionRow {
    /// Primary display text.
    label: String,
    /// Kind abbreviation (e.g., "fn", "va", "kw").
    kind_abbrev: String,
    /// Source ID (e.g., "lsp", "buffer").
    source_id: String,
}

/// Completion popup extension.
///
/// Shows a popup with completion items near the cursor. Each row displays
/// `[kind] label  (source)`.
const KIND: &str = "completion";

pub struct CompletionExtension {
    /// Whether the popup is currently visible.
    active: bool,
    /// Completion items to display.
    items: Vec<CompletionRow>,
    /// Index of the selected item.
    selected: usize,
    /// Scroll offset for the item list.
    scroll_offset: usize,
}

impl CompletionExtension {
    /// Create a new completion extension (inactive).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            active: false,
            items: Vec::new(),
            selected: 0,
            scroll_offset: 0,
        }
    }
}

impl Default for CompletionExtension {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiExtension for CompletionExtension {
    fn kind(&self) -> &'static str {
        KIND
    }

    fn is_active(&self) -> bool {
        self.active
    }

    #[allow(clippy::cast_possible_truncation)]
    fn apply_notification(&mut self, data: &str) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
            self.active = json
                .get("active")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);

            if !self.active {
                self.items.clear();
                self.selected = 0;
                self.scroll_offset = 0;
                return;
            }

            self.selected = json
                .get("selected")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0) as usize;

            self.scroll_offset = json
                .get("scrollOffset")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0) as usize;

            self.items = json
                .get("items")
                .and_then(serde_json::Value::as_array)
                .map_or_else(Vec::new, |arr| {
                    arr.iter()
                        .filter_map(|v| {
                            let label = v.get("label")?.as_str()?.to_owned();
                            let kind_abbrev = v
                                .get("kindAbbrev")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("tx")
                                .to_owned();
                            let source_id = v
                                .get("sourceId")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("")
                                .to_owned();
                            Some(CompletionRow {
                                label,
                                kind_abbrev,
                                source_id,
                            })
                        })
                        .collect()
                });
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn render(&self, backend: &mut dyn RenderBackend) {
        if self.items.is_empty() {
            return;
        }

        let (width, height) = backend.size();

        // Calculate visible window.
        let visible_count = self.items.len().min(MAX_VISIBLE_ITEMS);
        let popup_height = visible_count as u16 + 2; // +2 for border
        let popup_width = self.calculate_popup_width(width);

        // Position popup at center-bottom area (simplified — ideally below cursor).
        let px = width.saturating_sub(popup_width) / 2;
        let py = height.saturating_sub(popup_height).saturating_sub(1);

        // Draw background.
        let bg_style = Style::default().bg(Color::DarkGrey);
        for row in 0..popup_height {
            for col in 0..popup_width {
                backend.set_cell(px + col, py + row, ' ', &bg_style);
            }
        }

        // Draw border.
        let border_style = Style::default().fg(Color::Grey).bg(Color::DarkGrey);
        draw_simple_border(backend, px, py, popup_width, popup_height, &border_style);

        // Draw items.
        let content_x = px + 1;
        let content_width = popup_width.saturating_sub(2);
        let start_idx = self.scroll_offset;

        for (i, item) in self
            .items
            .iter()
            .skip(start_idx)
            .take(visible_count)
            .enumerate()
        {
            let row_y = py + 1 + i as u16;
            let is_selected = start_idx + i == self.selected;

            let row_style = if is_selected {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else {
                Style::default().bg(Color::DarkGrey).fg(Color::White)
            };

            // Clear row.
            for col in 0..content_width {
                backend.set_cell(content_x + col, row_y, ' ', &row_style);
            }

            // Kind abbreviation.
            let kind_style = if is_selected {
                Style::default().bg(Color::Blue).fg(Color::Yellow)
            } else {
                Style::default().bg(Color::DarkGrey).fg(Color::Yellow)
            };

            let mut x = content_x;
            for ch in item.kind_abbrev.chars() {
                if x >= content_x + content_width {
                    break;
                }
                backend.set_cell(x, row_y, ch, &kind_style);
                x += 1;
            }

            // Space separator.
            if x < content_x + content_width {
                backend.set_cell(x, row_y, ' ', &row_style);
                x += 1;
            }

            // Label.
            for ch in item.label.chars() {
                if x >= content_x + content_width {
                    break;
                }
                backend.set_cell(x, row_y, ch, &row_style);
                x += 1;
            }

            // Source ID (right-aligned, dimmed).
            if !item.source_id.is_empty() {
                let source_len = item.source_id.len() as u16;
                let source_x = (content_x + content_width).saturating_sub(source_len);
                if source_x > x + 1 {
                    let source_style = if is_selected {
                        Style::default().bg(Color::Blue).fg(Color::Grey)
                    } else {
                        Style::default().bg(Color::DarkGrey).fg(Color::Grey)
                    };
                    let max_chars = (content_x + content_width).saturating_sub(source_x) as usize;
                    for (sx, ch) in (source_x..).zip(item.source_id.chars().take(max_chars)) {
                        backend.set_cell(sx, row_y, ch, &source_style);
                    }
                }
            }
        }
    }
}

impl CompletionExtension {
    /// Calculate popup width based on terminal and item widths.
    #[allow(clippy::cast_possible_truncation)]
    fn calculate_popup_width(&self, terminal_width: u16) -> u16 {
        let max_item_width = self
            .items
            .iter()
            .map(|item| {
                let base = item.kind_abbrev.len() + 1 + item.label.len();
                if item.source_id.is_empty() {
                    base
                } else {
                    base + 2 + item.source_id.len() // +2 for gap before source
                }
            })
            .max()
            .unwrap_or(10);

        // +2 for border, +2 for padding.
        let desired = (max_item_width + 4) as u16;
        desired.clamp(20, terminal_width.saturating_sub(4))
    }
}

/// Draw a simple box border.
fn draw_simple_border(
    backend: &mut dyn RenderBackend,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    style: &Style,
) {
    // Top border.
    backend.set_cell(x, y, '\u{256D}', style);
    for col in 1..w.saturating_sub(1) {
        backend.set_cell(x + col, y, '\u{2500}', style);
    }
    backend.set_cell(x + w.saturating_sub(1), y, '\u{256E}', style);

    // Side borders.
    for row in 1..h.saturating_sub(1) {
        backend.set_cell(x, y + row, '\u{2502}', style);
        backend.set_cell(x + w.saturating_sub(1), y + row, '\u{2502}', style);
    }

    // Bottom border.
    backend.set_cell(x, y + h.saturating_sub(1), '\u{2570}', style);
    for col in 1..w.saturating_sub(1) {
        backend.set_cell(x + col, y + h.saturating_sub(1), '\u{2500}', style);
    }
    backend.set_cell(x + w.saturating_sub(1), y + h.saturating_sub(1), '\u{256F}', style);
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
