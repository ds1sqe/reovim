//! Jump label rendering extension.
//!
//! Receives jump match data from `JumpBridge` and renders label characters
//! as overlays at buffer positions using `ViewportContext`.

use reovim_driver_display::{
    Style,
    render_backend::{RenderBackend, TuiExtension, ViewportContext},
};

use reovim_arch::Color;

/// A single jump label with its buffer position.
#[derive(Debug, Clone)]
struct JumpLabel {
    /// Buffer line (0-indexed).
    line: u32,
    /// Buffer column (0-indexed, byte offset).
    col: u32,
    /// Label string (e.g., "s", "sf").
    label: String,
}

/// Jump label rendering extension.
///
/// Parses `JumpBridge` JSON notifications and renders label characters
/// at match positions using viewport context for coordinate mapping.
///
/// Kind: `"range-finder-jump"` (matches `JumpBridge::kind()`).
pub struct RangeFinderJumpExtension {
    active: bool,
    labels: Vec<JumpLabel>,
}

impl RangeFinderJumpExtension {
    /// Create a new inactive jump extension.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            active: false,
            labels: Vec::new(),
        }
    }
}

impl Default for RangeFinderJumpExtension {
    fn default() -> Self {
        Self::new()
    }
}

/// Style for jump label overlay: black text on yellow background.
fn label_style() -> Style {
    Style::default().fg(Color::Black).bg(Color::Yellow)
}

/// Style for jump label first character (dimmed second char).
fn label_dim_style() -> Style {
    Style::default()
        .fg(Color::Rgb {
            r: 80,
            g: 80,
            b: 80,
        })
        .bg(Color::Rgb {
            r: 180,
            g: 180,
            b: 60,
        })
}

impl TuiExtension for RangeFinderJumpExtension {
    fn kind(&self) -> &'static str {
        "range-finder-jump"
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn apply_notification(&mut self, data: &str) {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(data) else {
            return;
        };

        self.active = json
            .get("active")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        self.labels.clear();

        if !self.active {
            return;
        }

        let Some(matches) = json.get("matches").and_then(serde_json::Value::as_array) else {
            return;
        };

        for m in matches {
            let Some(line) = m.get("line").and_then(serde_json::Value::as_u64) else {
                continue;
            };
            let Some(col) = m.get("col").and_then(serde_json::Value::as_u64) else {
                continue;
            };
            let Some(label) = m.get("label").and_then(serde_json::Value::as_str) else {
                continue;
            };

            #[allow(clippy::cast_possible_truncation)]
            self.labels.push(JumpLabel {
                line: line as u32,
                col: col as u32,
                label: label.to_string(),
            });
        }
    }

    fn render(&self, _backend: &mut dyn RenderBackend) {
        // Jump labels need viewport context — this is a no-op.
        // Rendering happens in render_with_viewport.
    }

    #[allow(clippy::cast_possible_truncation)]
    fn render_with_viewport(&self, backend: &mut dyn RenderBackend, viewport: &ViewportContext) {
        if !self.active || self.labels.is_empty() {
            return;
        }

        let bright = label_style();
        let dim = label_dim_style();

        for label in &self.labels {
            let line = label.line as usize;

            // Skip if above viewport
            if line < viewport.scroll_top {
                continue;
            }

            let screen_row = line - viewport.scroll_top;

            // Skip if below viewport
            if screen_row >= viewport.content_height as usize {
                continue;
            }

            #[allow(clippy::cast_possible_truncation)]
            let screen_y = screen_row as u16;
            let screen_x = viewport.content_x.saturating_add(label.col as u16);

            // Render label characters
            let mut chars = label.label.chars();
            if let Some(first) = chars.next() {
                backend.set_cell(screen_x, screen_y, first, &bright);

                // Second char (if two-char label) gets dimmer style
                if let Some(second) = chars.next() {
                    backend.set_cell(screen_x.saturating_add(1), screen_y, second, &dim);
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "jump_tests.rs"]
mod tests;
