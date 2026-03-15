//! Completion popup chrome module.
//!
//! Displays a floating completion popup below the cursor position.
//! Shows completion items with kind abbreviation, label, and source.
//! Native `ClientModule` implementation (no `TuiExtension` bridge).

use reovim_client_driver::{
    ChromePosition, ClientModule, ClientModuleError, ModuleContext, PlatformCapabilities,
    ProbeResult, Rect, RenderSurface, Style, Version, types::Color,
};

/// Maximum number of visible items in the popup.
const MAX_VISIBLE_ITEMS: usize = 10;

const KIND: &str = "completion";

/// Completion item for rendering.
#[derive(Debug, Clone)]
struct CompletionRow {
    /// Primary display text.
    label: String,
    /// Kind abbreviation (e.g., "fn", "va", "kw").
    kind_abbrev: String,
    /// Kind icon (Nerd Font glyph, preferred over abbreviation).
    kind_icon: String,
    /// Source ID (e.g., "lsp", "buffer").
    source_id: String,
}

/// Completion popup chrome module.
pub struct CompletionModule {
    /// Whether the popup is currently visible.
    active: bool,
    /// Completion items to display.
    items: Vec<CompletionRow>,
    /// Index of the selected item.
    selected: usize,
    /// Scroll offset for the item list.
    scroll_offset: usize,
}

impl CompletionModule {
    /// Create a new completion module (inactive).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            active: false,
            items: Vec::new(),
            selected: 0,
            scroll_offset: 0,
        }
    }

    /// Calculate popup width based on terminal and item widths.
    #[allow(clippy::cast_possible_truncation)]
    fn calculate_popup_width(&self, terminal_width: u16) -> u16 {
        let max_item_width = self
            .items
            .iter()
            .map(|item| {
                let kind_len = if item.kind_icon.is_empty() {
                    item.kind_abbrev.len()
                } else {
                    // Nerd Font icons are typically 2 display columns wide
                    2
                };
                let base = kind_len + 1 + item.label.len();
                if item.source_id.is_empty() {
                    base
                } else {
                    base + 2 + item.source_id.len()
                }
            })
            .max()
            .unwrap_or(10);

        // +2 for border, +2 for padding.
        let desired = (max_item_width + 4) as u16;
        desired.clamp(20, terminal_width.saturating_sub(4))
    }
}

impl Default for CompletionModule {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientModule for CompletionModule {
    fn id(&self) -> &'static str {
        KIND
    }

    fn kind(&self) -> &'static str {
        KIND
    }

    fn name(&self) -> &'static str {
        "Completion"
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
        70
    }

    #[allow(clippy::cast_possible_truncation)]
    fn on_notification(&mut self, data: &str) {
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
                            let kind_icon = v
                                .get("kindIcon")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("")
                                .to_owned();
                            let source_id = v
                                .get("sourceId")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("")
                                .to_owned();
                            Some(CompletionRow {
                                label,
                                kind_abbrev,
                                kind_icon,
                                source_id,
                            })
                        })
                        .collect()
                });
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn chrome_render(
        &self,
        surface: &mut dyn RenderSurface,
        bounds: Rect,
        _caps: &dyn PlatformCapabilities,
    ) {
        if !self.active || self.items.is_empty() {
            return;
        }

        let (width, height) = (bounds.width, bounds.height);

        // Calculate visible window.
        let visible_count = self.items.len().min(MAX_VISIBLE_ITEMS);
        let popup_height = visible_count as u16 + 2; // +2 for border
        let popup_width = self.calculate_popup_width(width);

        // Position popup at center-bottom area.
        let px = width.saturating_sub(popup_width) / 2;
        let py = height.saturating_sub(popup_height).saturating_sub(1);

        // Draw background.
        let bg_style = Style::new().bg(Color::DarkGrey);
        surface.fill(
            Rect {
                x: px,
                y: py,
                width: popup_width,
                height: popup_height,
            },
            ' ',
            bg_style,
        );

        // Draw border.
        let border_style = Style::new().fg(Color::Grey).bg(Color::DarkGrey);
        draw_simple_border(surface, px, py, popup_width, popup_height, &border_style);

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
                Style::new().bg(Color::Blue).fg(Color::White)
            } else {
                Style::new().bg(Color::DarkGrey).fg(Color::White)
            };

            // Clear row.
            surface.fill(
                Rect {
                    x: content_x,
                    y: row_y,
                    width: content_width,
                    height: 1,
                },
                ' ',
                row_style.clone(),
            );

            // Kind abbreviation.
            let kind_style = if is_selected {
                Style::new().bg(Color::Blue).fg(Color::Yellow)
            } else {
                Style::new().bg(Color::DarkGrey).fg(Color::Yellow)
            };

            let mut x = content_x;
            let kind_display = if item.kind_icon.is_empty() {
                &item.kind_abbrev
            } else {
                &item.kind_icon
            };
            let kind_truncated: String = kind_display
                .chars()
                .take((content_x + content_width).saturating_sub(x) as usize)
                .collect();
            let written = surface.write_styled(x, row_y, &kind_truncated, kind_style);
            x += written;

            // Space separator.
            if x < content_x + content_width {
                surface.write_styled(x, row_y, " ", row_style.clone());
                x += 1;
            }

            // Label.
            let max_label = (content_x + content_width).saturating_sub(x) as usize;
            let label_display: String = item.label.chars().take(max_label).collect();
            let written = surface.write_styled(x, row_y, &label_display, row_style.clone());
            x += written;

            // Source ID (right-aligned, dimmed).
            if !item.source_id.is_empty() {
                let source_len = item.source_id.len() as u16;
                let source_x = (content_x + content_width).saturating_sub(source_len);
                if source_x > x + 1 {
                    let source_style = if is_selected {
                        Style::new().bg(Color::Blue).fg(Color::Grey)
                    } else {
                        Style::new().bg(Color::DarkGrey).fg(Color::Grey)
                    };
                    let max_chars = (content_x + content_width).saturating_sub(source_x) as usize;
                    let source_display: String = item.source_id.chars().take(max_chars).collect();
                    surface.write_styled(source_x, row_y, &source_display, source_style);
                }
            }
        }
    }
}

/// Draw a simple box border on a `RenderSurface`.
fn draw_simple_border(
    surface: &mut dyn RenderSurface,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    style: &Style,
) {
    if w < 2 || h < 2 {
        return;
    }
    // Top border.
    surface.write_styled(x, y, "\u{256D}", style.clone());
    for col in 1..w.saturating_sub(1) {
        surface.write_styled(x + col, y, "\u{2500}", style.clone());
    }
    surface.write_styled(x + w - 1, y, "\u{256E}", style.clone());

    // Side borders.
    for row in 1..h.saturating_sub(1) {
        surface.write_styled(x, y + row, "\u{2502}", style.clone());
        surface.write_styled(x + w - 1, y + row, "\u{2502}", style.clone());
    }

    // Bottom border.
    surface.write_styled(x, y + h - 1, "\u{2570}", style.clone());
    for col in 1..w.saturating_sub(1) {
        surface.write_styled(x + col, y + h - 1, "\u{2500}", style.clone());
    }
    surface.write_styled(x + w - 1, y + h - 1, "\u{256F}", style.clone());
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
