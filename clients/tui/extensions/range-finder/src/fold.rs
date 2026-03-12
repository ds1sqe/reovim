//! Fold indicator rendering extension.
//!
//! Receives fold state from `FoldBridge` and renders fold markers
//! at collapsed line positions using `ViewportContext`.

use std::collections::HashMap;

use reovim_driver_display::{
    Style,
    render_backend::{RenderBackend, TuiExtension, ViewportContext},
    ui::truncate_end,
};

use reovim_arch::Color;

/// A collapsed fold for display.
#[derive(Debug, Clone)]
struct CollapsedFold {
    /// Buffer line where the fold starts (0-indexed).
    start_line: u32,
    /// Number of hidden lines.
    hidden_count: u32,
    /// Preview text (e.g., `fn foo() {`).
    preview: String,
}

/// Fold indicator rendering extension.
///
/// Parses `FoldBridge` JSON notifications and renders fold markers
/// as overlays at collapsed fold start lines.
///
/// Kind: `"range-finder-fold"` (matches `FoldBridge::kind()`).
pub struct RangeFinderFoldExtension {
    active: bool,
    /// Per-buffer fold info. Key is `buffer_id` as string (from JSON).
    folds: HashMap<String, Vec<CollapsedFold>>,
    /// Currently active buffer ID (set from notification context).
    active_buffer_id: Option<String>,
    /// Cached hidden line ranges for the active buffer: `(start_line, hidden_count)`.
    ///
    /// Rebuilt on `apply_notification` and `set_active_buffer`.
    /// The `start_line` here is the first *hidden* line (fold start + 1),
    /// since the fold marker line itself remains visible.
    hidden_ranges: Vec<(u32, u32)>,
}

impl RangeFinderFoldExtension {
    /// Create a new inactive fold extension.
    #[must_use]
    pub fn new() -> Self {
        Self {
            active: false,
            folds: HashMap::new(),
            active_buffer_id: None,
            hidden_ranges: Vec::new(),
        }
    }

    /// Rebuild the cached hidden ranges from the active buffer's folds.
    fn rebuild_hidden_ranges(&mut self) {
        self.hidden_ranges.clear();
        let Some(buf_id) = &self.active_buffer_id else {
            return;
        };
        let Some(folds) = self.folds.get(buf_id) else {
            return;
        };
        for fold in folds {
            if fold.hidden_count > 0 {
                // Hidden lines start after the fold marker line
                self.hidden_ranges
                    .push((fold.start_line + 1, fold.hidden_count));
            }
        }
    }
}

impl Default for RangeFinderFoldExtension {
    fn default() -> Self {
        Self::new()
    }
}

/// Style for the fold marker line.
fn fold_marker_style() -> Style {
    Style::default().fg(Color::DarkGrey)
}

impl TuiExtension for RangeFinderFoldExtension {
    fn kind(&self) -> &'static str {
        "range-finder-fold"
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn apply_notification(&mut self, data: &str) {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(data) else {
            return;
        };

        self.folds.clear();
        self.active = false;

        let Some(folds_obj) = json.get("folds").and_then(serde_json::Value::as_object) else {
            return;
        };

        for (buf_id, collapsed_arr) in folds_obj {
            let Some(arr) = collapsed_arr.as_array() else {
                continue;
            };

            let mut folds = Vec::new();
            for entry in arr {
                let Some(start_line) = entry.get("start_line").and_then(serde_json::Value::as_u64)
                else {
                    continue;
                };
                let Some(hidden_count) = entry
                    .get("hidden_count")
                    .and_then(serde_json::Value::as_u64)
                else {
                    continue;
                };
                let preview = entry
                    .get("preview")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .to_string();

                #[allow(clippy::cast_possible_truncation)]
                folds.push(CollapsedFold {
                    start_line: start_line as u32,
                    hidden_count: hidden_count as u32,
                    preview,
                });
            }

            if !folds.is_empty() {
                self.folds.insert(buf_id.clone(), folds);
            }
        }

        self.active = !self.folds.is_empty();

        // If only one buffer has folds, auto-select it
        if self.folds.len() == 1 {
            self.active_buffer_id = self.folds.keys().next().cloned();
        }

        self.rebuild_hidden_ranges();
    }

    fn render(&self, _backend: &mut dyn RenderBackend) {
        // Fold markers need viewport context — rendering in render_with_viewport.
    }

    fn fold_hidden_lines(&self) -> &[(u32, u32)] {
        &self.hidden_ranges
    }

    #[allow(clippy::cast_possible_truncation)]
    fn render_with_viewport(&self, backend: &mut dyn RenderBackend, viewport: &ViewportContext) {
        if !self.active {
            return;
        }

        let Some(buf_id) = &self.active_buffer_id else {
            return;
        };
        let Some(folds) = self.folds.get(buf_id) else {
            return;
        };

        let (width, _) = backend.size();
        let style = fold_marker_style();

        for fold in folds {
            let line = fold.start_line as usize;

            if line < viewport.scroll_top {
                continue;
            }

            let screen_row = line - viewport.scroll_top;
            if screen_row >= viewport.content_height as usize {
                continue;
            }

            let screen_y = screen_row as u16;
            let content_width = width.saturating_sub(viewport.content_x);

            // Format: "--- N lines: preview ---"
            let marker = format!("--- {} lines: {} ---", fold.hidden_count, fold.preview);
            let display = truncate_end(&marker, content_width as usize);

            backend.write_str(viewport.content_x, screen_y, &display, &style);
        }
    }
}

/// Set the active buffer ID for fold rendering.
///
/// Called externally when the focused buffer changes.
impl RangeFinderFoldExtension {
    /// Set which buffer's folds to render.
    pub fn set_active_buffer(&mut self, buffer_id: &str) {
        self.active_buffer_id = Some(buffer_id.to_string());
        self.rebuild_hidden_ranges();
    }
}

#[cfg(test)]
#[path = "fold_tests.rs"]
mod tests;
