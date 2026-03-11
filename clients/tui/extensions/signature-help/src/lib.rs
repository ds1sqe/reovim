//! Signature help popup TUI extension.
//!
//! Displays LSP signature help (function signature) in a single-line
//! bordered popup near the cursor position.
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

/// Minimum popup width.
const MIN_WIDTH: u16 = 10;

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

/// Deserialized signature help notification payload.
#[derive(Debug, Deserialize)]
struct SignatureHelpPayload {
    active: bool,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    origin: Option<Origin>,
}

/// Signature help popup TUI extension.
///
/// Renders a single-line bordered popup showing the active function
/// signature near the origin buffer position.
pub struct SignatureHelpExtension {
    /// Whether the popup is visible.
    active: bool,
    /// Signature label to display.
    label: String,
    /// Origin line in the buffer (0-indexed).
    origin_line: u32,
    /// Origin column in the buffer (0-indexed).
    origin_col: u32,
}

impl SignatureHelpExtension {
    /// Create a new signature help extension (inactive).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            active: false,
            label: String::new(),
            origin_line: 0,
            origin_col: 0,
        }
    }
}

impl Default for SignatureHelpExtension {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiExtension for SignatureHelpExtension {
    fn kind(&self) -> &'static str {
        reovim_extension_kinds::SIGNATURE_HELP
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn apply_notification(&mut self, data: &str) {
        let Ok(payload) = serde_json::from_str::<SignatureHelpPayload>(data) else {
            return;
        };

        self.active = payload.active;

        if !self.active {
            self.label.clear();
            self.origin_line = 0;
            self.origin_col = 0;
            return;
        }

        if let Some(Origin::BufferPosition { line, col, .. }) = payload.origin {
            self.origin_line = line;
            self.origin_col = col;
        }

        self.label = payload.label.unwrap_or_default();

        if self.label.is_empty() {
            self.active = false;
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn render(&self, backend: &mut dyn RenderBackend) {
        if self.label.is_empty() {
            return;
        }

        let (width, height) = backend.size();

        // Popup is always 1 content line + 2 border = 3 rows.
        let popup_h: u16 = 3;

        // Width: label length + 4 (border + padding), clamped.
        let desired_w = (self.label.len() + 4).min(u16::MAX as usize) as u16;
        let popup_w = desired_w.clamp(MIN_WIDTH, width.saturating_sub(2));

        // Horizontal position: align with origin, clamp to screen.
        let px = self
            .origin_col
            .min(u32::from(width.saturating_sub(popup_w))) as u16;

        // Vertical position: prefer above origin line, fall back below.
        let anchor_y = (self.origin_line as u16).min(height.saturating_sub(1));
        let py = if anchor_y >= popup_h {
            anchor_y - popup_h
        } else if anchor_y.saturating_add(1).saturating_add(popup_h) <= height {
            anchor_y + 1
        } else {
            0
        };

        let border_style = Style::default().fg(Color::Yellow);
        render_box_border(backend, px, py, popup_w, popup_h, &border_style);

        // Clear interior and draw label.
        let content_x = px + 1;
        let content_w = popup_w.saturating_sub(2);
        let bg_style = Style::default();
        let text_style = Style::default().fg(Color::White);

        for col in 0..content_w {
            backend.set_cell(content_x + col, py + 1, ' ', &bg_style);
        }

        let display = truncate_end(&self.label, content_w as usize);
        backend.write_str(content_x, py + 1, &display, &text_style);
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
