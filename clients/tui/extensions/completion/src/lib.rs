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
        "completion"
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
mod tests {
    use reovim_driver_display::FrameBuffer;

    use super::*;

    #[test]
    fn test_new_is_inactive() {
        let ext = CompletionExtension::new();
        assert!(!ext.is_active());
        assert_eq!(ext.kind(), "completion");
    }

    #[test]
    fn test_default_is_inactive() {
        let ext = CompletionExtension::default();
        assert!(!ext.is_active());
    }

    #[test]
    fn test_apply_notification_activates() {
        let mut ext = CompletionExtension::new();
        let data = r#"{"active":true,"items":[{"label":"println","kindAbbrev":"fn","sourceId":"lsp"}],"selected":0,"prefix":"pr","scrollOffset":0}"#;
        ext.apply_notification(data);

        assert!(ext.is_active());
        assert_eq!(ext.items.len(), 1);
        assert_eq!(ext.items[0].label, "println");
        assert_eq!(ext.items[0].kind_abbrev, "fn");
        assert_eq!(ext.items[0].source_id, "lsp");
        assert_eq!(ext.selected, 0);
    }

    #[test]
    fn test_apply_notification_deactivates() {
        let mut ext = CompletionExtension::new();
        ext.apply_notification(
            r#"{"active":true,"items":[{"label":"foo","kindAbbrev":"va","sourceId":"buffer"}],"selected":0,"prefix":"f","scrollOffset":0}"#,
        );
        assert!(ext.is_active());

        ext.apply_notification(r#"{"active":false}"#);
        assert!(!ext.is_active());
        assert!(ext.items.is_empty());
    }

    #[test]
    fn test_apply_notification_invalid_json() {
        let mut ext = CompletionExtension::new();
        ext.apply_notification("not json");
        assert!(!ext.is_active());
    }

    #[test]
    fn test_apply_notification_multiple_items() {
        let mut ext = CompletionExtension::new();
        ext.apply_notification(
            r#"{"active":true,"items":[
                {"label":"println","kindAbbrev":"fn","sourceId":"lsp"},
                {"label":"print","kindAbbrev":"fn","sourceId":"lsp"},
                {"label":"prefix","kindAbbrev":"va","sourceId":"buffer"}
            ],"selected":1,"prefix":"pr","scrollOffset":0}"#,
        );

        assert_eq!(ext.items.len(), 3);
        assert_eq!(ext.selected, 1);
    }

    #[test]
    fn test_apply_notification_defaults() {
        let mut ext = CompletionExtension::new();
        ext.apply_notification(r#"{"active":true,"items":[{"label":"test"}]}"#);

        assert!(ext.is_active());
        assert_eq!(ext.items.len(), 1);
        assert_eq!(ext.items[0].kind_abbrev, "tx"); // Default kind.
        assert!(ext.items[0].source_id.is_empty()); // Default source.
        assert_eq!(ext.selected, 0); // Default selected.
    }

    #[test]
    fn test_apply_notification_item_without_label_skipped() {
        let mut ext = CompletionExtension::new();
        ext.apply_notification(
            r#"{"active":true,"items":[{"kindAbbrev":"fn"},{"label":"valid","kindAbbrev":"va"}]}"#,
        );
        // Item without label is filtered out.
        assert_eq!(ext.items.len(), 1);
        assert_eq!(ext.items[0].label, "valid");
    }

    #[test]
    fn test_render_empty_items_is_noop() {
        let mut ext = CompletionExtension::new();
        ext.active = true;
        // No items — render should be a no-op.
        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);
        // No crash, no rendering.
    }

    #[test]
    fn test_render_shows_popup() {
        let mut ext = CompletionExtension::new();
        ext.apply_notification(
            r#"{"active":true,"items":[
                {"label":"println","kindAbbrev":"fn","sourceId":"lsp"},
                {"label":"print","kindAbbrev":"fn","sourceId":"lsp"}
            ],"selected":0,"prefix":"pr","scrollOffset":0}"#,
        );

        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);

        // Verify the popup border is drawn.
        let pw = ext.calculate_popup_width(80);
        let px = (80 - pw) / 2;
        let py = 24 - 4 - 1; // height=4, margin=1

        assert_eq!(fb.get(px, py).unwrap().char, '\u{256D}'); // top-left
        assert_eq!(fb.get(px + pw - 1, py).unwrap().char, '\u{256E}'); // top-right
    }

    #[test]
    fn test_render_selected_item_highlighted() {
        let mut ext = CompletionExtension::new();
        ext.apply_notification(
            r#"{"active":true,"items":[
                {"label":"aaa","kindAbbrev":"fn","sourceId":"lsp"},
                {"label":"bbb","kindAbbrev":"va","sourceId":"buf"}
            ],"selected":1,"prefix":"","scrollOffset":0}"#,
        );

        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);

        let pw = ext.calculate_popup_width(80);
        let px = (80 - pw) / 2;
        let py = 24 - 4 - 1;

        // Selected row (index 1) should have Blue bg.
        let cell = fb.get(px + 1, py + 2).unwrap();
        assert_eq!(cell.style.bg, Some(Color::Blue));
    }

    #[test]
    fn test_render_kind_abbreviation() {
        let mut ext = CompletionExtension::new();
        ext.apply_notification(
            r#"{"active":true,"items":[{"label":"my_func","kindAbbrev":"fn","sourceId":"lsp"}],"selected":0,"prefix":"","scrollOffset":0}"#,
        );

        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);

        let pw = ext.calculate_popup_width(80);
        let px = (80 - pw) / 2;
        let py = 24 - 3 - 1;
        let content_x = px + 1;

        // Kind abbreviation "fn" in yellow.
        let cell = fb.get(content_x, py + 1).unwrap();
        assert_eq!(cell.char, 'f');
        assert_eq!(cell.style.fg, Some(Color::Yellow));
    }

    #[test]
    fn test_calculate_popup_width_min() {
        let ext = CompletionExtension::new();
        assert_eq!(ext.calculate_popup_width(80), 20); // Min 20 with no items.
    }

    #[test]
    fn test_calculate_popup_width_adapts() {
        let mut ext = CompletionExtension::new();
        ext.items = vec![CompletionRow {
            label: "a_very_long_function_name".to_owned(),
            kind_abbrev: "fn".to_owned(),
            source_id: "lsp".to_owned(),
        }];
        let width = ext.calculate_popup_width(80);
        // "fn" + " " + "a_very_long_function_name" + "  " + "lsp" = 33 chars + 4 padding = 37
        assert_eq!(width, 37);
    }

    #[test]
    fn test_calculate_popup_width_clamped() {
        let mut ext = CompletionExtension::new();
        ext.items = vec![CompletionRow {
            label: "x".repeat(100),
            kind_abbrev: "fn".to_owned(),
            source_id: "lsp".to_owned(),
        }];
        let width = ext.calculate_popup_width(50);
        assert_eq!(width, 46); // 50 - 4
    }

    #[test]
    fn test_render_narrow_terminal() {
        let mut ext = CompletionExtension::new();
        ext.apply_notification(
            r#"{"active":true,"items":[{"label":"test","kindAbbrev":"fn","sourceId":"lsp"}],"selected":0,"prefix":"","scrollOffset":0}"#,
        );

        let mut fb = FrameBuffer::new(24, 10);
        ext.render(&mut fb);
        // Should render without panic.
    }

    #[test]
    fn test_render_many_items_capped() {
        let mut ext = CompletionExtension::new();
        let items: Vec<String> = (0..20)
            .map(|i| format!(r#"{{"label":"item{i}","kindAbbrev":"tx","sourceId":"buf"}}"#))
            .collect();
        let json = format!(
            r#"{{"active":true,"items":[{}],"selected":5,"prefix":"","scrollOffset":0}}"#,
            items.join(",")
        );
        ext.apply_notification(&json);

        assert_eq!(ext.items.len(), 20);

        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);
        // Only MAX_VISIBLE_ITEMS (10) should be rendered.
    }

    #[test]
    fn test_trait_object() {
        let ext: Box<dyn TuiExtension> = Box::new(CompletionExtension::new());
        assert_eq!(ext.kind(), "completion");
        assert!(!ext.is_active());
    }

    #[test]
    fn test_completion_row_debug() {
        let row = CompletionRow {
            label: "test".to_owned(),
            kind_abbrev: "fn".to_owned(),
            source_id: "lsp".to_owned(),
        };
        let debug = format!("{row:?}");
        assert!(debug.contains("CompletionRow"));
    }

    #[test]
    fn test_completion_row_clone() {
        let row = CompletionRow {
            label: "test".to_owned(),
            kind_abbrev: "fn".to_owned(),
            source_id: "lsp".to_owned(),
        };
        #[allow(clippy::redundant_clone)]
        let cloned = row.clone();
        assert_eq!(cloned.label, "test");
    }

    #[test]
    fn test_draw_simple_border() {
        let mut fb = FrameBuffer::new(40, 20);
        let style = Style::default();
        draw_simple_border(&mut fb, 5, 5, 10, 4, &style);

        assert_eq!(fb.get(5, 5).unwrap().char, '\u{256D}');
        assert_eq!(fb.get(14, 5).unwrap().char, '\u{256E}');
        assert_eq!(fb.get(5, 8).unwrap().char, '\u{2570}');
        assert_eq!(fb.get(14, 8).unwrap().char, '\u{256F}');
        assert_eq!(fb.get(6, 5).unwrap().char, '\u{2500}');
        assert_eq!(fb.get(5, 6).unwrap().char, '\u{2502}');
    }

    #[test]
    fn test_render_kind_abbrev_overflow() {
        // Kind abbreviation longer than content width → triggers break at truncation.
        // Min popup width = 20, content_width = 18. kind_abbrev > 18 overflows.
        let mut ext = CompletionExtension::new();
        ext.items = vec![CompletionRow {
            label: "a".to_owned(),
            kind_abbrev: "x".repeat(25),
            source_id: String::new(),
        }];
        ext.active = true;
        ext.selected = 0;
        ext.scroll_offset = 0;

        // Terminal of 24 → popup_width = 20, content_width = 18.
        let mut fb = FrameBuffer::new(24, 10);
        ext.render(&mut fb);
    }

    #[test]
    fn test_render_label_overflow() {
        // Label longer than content width → triggers truncation break.
        let mut ext = CompletionExtension::new();
        ext.items = vec![CompletionRow {
            label: "a".repeat(200),
            kind_abbrev: "fn".to_owned(),
            source_id: String::new(),
        }];
        ext.active = true;
        ext.selected = 0;
        ext.scroll_offset = 0;

        let mut fb = FrameBuffer::new(26, 10);
        ext.render(&mut fb);
    }

    #[test]
    fn test_render_source_id_truncated() {
        // Source ID that would extend beyond content width.
        let mut ext = CompletionExtension::new();
        ext.items = vec![CompletionRow {
            label: "x".repeat(50),
            kind_abbrev: "fn".to_owned(),
            source_id: "x".repeat(50),
        }];
        ext.active = true;
        ext.selected = 0;
        ext.scroll_offset = 0;

        let mut fb = FrameBuffer::new(30, 10);
        ext.render(&mut fb);
        // Source ID rendering truncated to fit.
    }

    #[test]
    fn test_render_source_id_skipped_when_overlapping_label() {
        // Source ID would overlap with label text → skipped.
        let mut ext = CompletionExtension::new();
        ext.items = vec![CompletionRow {
            label: "a_very_long_name_that_fills".to_owned(),
            kind_abbrev: "fn".to_owned(),
            source_id: "lsp".to_owned(),
        }];
        ext.active = true;
        ext.selected = 0;
        ext.scroll_offset = 0;

        // Tight terminal: popup will be narrow, label fills most of it.
        let mut fb = FrameBuffer::new(30, 10);
        ext.render(&mut fb);
    }

    #[test]
    fn test_render_empty_source_id() {
        // Empty source_id → source rendering skipped entirely.
        let mut ext = CompletionExtension::new();
        ext.items = vec![CompletionRow {
            label: "test_item".to_owned(),
            kind_abbrev: "va".to_owned(),
            source_id: String::new(),
        }];
        ext.active = true;
        ext.selected = 0;
        ext.scroll_offset = 0;

        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);
    }

    #[test]
    fn test_calculate_popup_width_empty_source_id() {
        // Exercises the empty source_id branch in width calculation.
        let mut ext = CompletionExtension::new();
        ext.items = vec![CompletionRow {
            label: "hello".to_owned(),
            kind_abbrev: "fn".to_owned(),
            source_id: String::new(),
        }];
        let width = ext.calculate_popup_width(80);
        // "fn" + " " + "hello" = 8 chars + 4 padding = 12, but min is 20.
        assert_eq!(width, 20);
    }

    #[test]
    fn test_render_source_id_on_selected_row() {
        // Source ID rendered on a selected row (Blue bg + Grey fg).
        let mut ext = CompletionExtension::new();
        ext.items = vec![
            CompletionRow {
                label: "short".to_owned(),
                kind_abbrev: "fn".to_owned(),
                source_id: "lsp".to_owned(),
            },
            CompletionRow {
                label: "short".to_owned(),
                kind_abbrev: "fn".to_owned(),
                source_id: "buf".to_owned(),
            },
        ];
        ext.active = true;
        ext.selected = 0;
        ext.scroll_offset = 0;

        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);

        // Verify source_id "lsp" appears on the selected row.
        let pw = ext.calculate_popup_width(80);
        let px = (80 - pw) / 2;
        let py = 24 - 4 - 1;
        let content_x = px + 1;
        let content_width = pw - 2;
        let source_x = content_x + content_width - 3; // "lsp" len = 3
        let cell = fb.get(source_x, py + 1).unwrap();
        assert_eq!(cell.char, 'l');
        assert_eq!(cell.style.fg, Some(Color::Grey));
        assert_eq!(cell.style.bg, Some(Color::Blue));
    }
}
