//! Hover popup TUI extension.
//!
//! Displays LSP hover information in a bordered popup near the cursor.
//! Content is rendered as plain text lines within a box border.
//!
//! This crate is a self-contained TUI extension: it owns its state,
//! parses notifications, and renders through `RenderBackend`.
//! The engine has ZERO knowledge of this crate.

use {
    reovim_arch::Color,
    reovim_driver_display::{
        Style,
        popup_utils::render_box_border,
        render_backend::{RenderBackend, TuiExtension},
        ui::truncate_end,
    },
    serde::Deserialize,
};

/// Maximum popup width (fraction of terminal width).
const MAX_WIDTH_RATIO: f32 = 0.6;

/// Minimum popup width.
const MIN_WIDTH: u16 = 20;

/// Maximum number of content lines displayed.
const MAX_LINES: usize = 20;

/// Hover content type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ContentType {
    Plaintext,
    Markdown,
}

/// Origin position from the server.
#[derive(Debug, Clone, Deserialize)]
enum Origin {
    BufferPosition {
        #[allow(dead_code)]
        buffer_id: u64,
        line: u32,
        col: u32,
    },
}

/// Deserialized hover notification payload.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HoverPayload {
    active: bool,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    content_type: Option<ContentType>,
    #[serde(default)]
    origin: Option<Origin>,
}

/// Hover popup TUI extension.
///
/// Renders LSP hover content in a bordered popup positioned near the
/// origin buffer position.
const KIND: &str = "hover";

pub struct HoverExtension {
    /// Whether the popup is visible.
    active: bool,
    /// Content lines to display.
    lines: Vec<String>,
    /// Content type (affects border color).
    content_type: ContentType,
    /// Origin line in the buffer (0-indexed).
    origin_line: u32,
    /// Origin column in the buffer (0-indexed).
    origin_col: u32,
}

impl HoverExtension {
    /// Create a new hover extension (inactive).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            active: false,
            lines: Vec::new(),
            content_type: ContentType::Plaintext,
            origin_line: 0,
            origin_col: 0,
        }
    }

    /// Calculate popup width from content and terminal width.
    fn popup_width(&self, terminal_width: u16) -> u16 {
        if terminal_width < MIN_WIDTH {
            return terminal_width;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let max_width = (f32::from(terminal_width) * MAX_WIDTH_RATIO) as u16;
        let max_width = max_width
            .max(MIN_WIDTH)
            .min(terminal_width.saturating_sub(2));

        let longest_line = self.lines.iter().map(String::len).max().unwrap_or(0);

        // +4 for border (2) + padding (2)
        #[allow(clippy::cast_possible_truncation)]
        let desired = (longest_line + 4).min(u16::MAX as usize) as u16;
        desired.clamp(MIN_WIDTH, max_width)
    }
}

impl Default for HoverExtension {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiExtension for HoverExtension {
    fn kind(&self) -> &'static str {
        KIND
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn apply_notification(&mut self, data: &str) {
        let Ok(payload) = serde_json::from_str::<HoverPayload>(data) else {
            return;
        };

        self.active = payload.active;

        if !self.active {
            self.lines.clear();
            self.origin_line = 0;
            self.origin_col = 0;
            return;
        }

        self.content_type = payload.content_type.unwrap_or(ContentType::Plaintext);

        if let Some(Origin::BufferPosition { line, col, .. }) = payload.origin {
            self.origin_line = line;
            self.origin_col = col;
        }

        self.lines = payload
            .content
            .as_deref()
            .unwrap_or("")
            .lines()
            .take(MAX_LINES)
            .map(String::from)
            .collect();

        // If content was empty, deactivate.
        if self.lines.is_empty() {
            self.active = false;
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn render(&self, backend: &mut dyn RenderBackend) {
        if self.lines.is_empty() {
            return;
        }

        let (width, height) = backend.size();
        let popup_w = self.popup_width(width);
        let content_lines = self.lines.len().min(MAX_LINES) as u16;
        let popup_h = content_lines + 2; // +2 for top/bottom border

        // Horizontal position: try to align with origin column, clamp to screen.
        let px = self
            .origin_col
            .min(u32::from(width.saturating_sub(popup_w))) as u16;

        // Vertical position: prefer below origin line + 1, fall back to above.
        let anchor_y = (self.origin_line as u16).min(height.saturating_sub(1));
        let py = if anchor_y.saturating_add(1).saturating_add(popup_h) < height {
            anchor_y + 1
        } else {
            anchor_y.saturating_sub(popup_h)
        };

        // Border color: cyan for markdown, grey for plaintext.
        let border_color = match self.content_type {
            ContentType::Markdown => Color::Cyan,
            ContentType::Plaintext => Color::Grey,
        };
        let border_style = Style::default().fg(border_color);

        render_box_border(backend, px, py, popup_w, popup_h, &border_style);

        // Clear and draw content interior.
        let content_x = px + 1;
        let content_w = popup_w.saturating_sub(2);
        let bg_style = Style::default();
        let text_style = Style::default().fg(Color::White);

        for (i, line) in self.lines.iter().take(content_lines as usize).enumerate() {
            let row = py + 1 + i as u16;

            // Clear interior row.
            for col in 0..content_w {
                backend.set_cell(content_x + col, row, ' ', &bg_style);
            }

            // Draw text.
            let display = truncate_end(line, content_w as usize);
            backend.write_str(content_x, row, &display, &text_style);
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
