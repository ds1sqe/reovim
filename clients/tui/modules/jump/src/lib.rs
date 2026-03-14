//! Jump label rendering module.
//!
//! Receives jump match data from the server's `JumpBridge` and renders
//! label characters at buffer positions via `transform_line()`.
//!
//! Migrated from `RangeFinderJumpExtension` (`TuiExtension`) to native
//! `ClientModule` as part of M6 (#637).

use reovim_client_driver::{
    BufferId, ClientModule, ClientModuleError, ModuleContext, ProbeResult, Style, TransformedLine,
    Version,
};

use reovim_arch::Color;

/// A single jump label with its buffer position.
#[derive(Debug, Clone)]
struct JumpLabel {
    /// Buffer line (0-indexed).
    line: usize,
    /// Buffer column (0-indexed).
    col: usize,
    /// Label string (e.g., "s", "sf").
    label: String,
}

/// Style for jump label: black text on yellow background.
const fn label_style() -> Style {
    Style::new().fg(Color::Black).bg(Color::Yellow)
}

/// Style for second character of two-char labels (dimmed).
const fn label_dim_style() -> Style {
    Style::new()
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

/// Jump label rendering module.
///
/// Parses jump notifications from the server and transforms lines
/// at label positions to show label characters overlaid on the buffer text.
///
/// Kind: `"range-finder-jump"` (matches server's `JumpBridge::kind()`).
pub struct JumpModule {
    active: bool,
    labels: Vec<JumpLabel>,
}

impl JumpModule {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            active: false,
            labels: Vec::new(),
        }
    }
}

impl Default for JumpModule {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientModule for JumpModule {
    fn id(&self) -> &'static str {
        "range-finder-jump"
    }

    fn kind(&self) -> &'static str {
        "range-finder-jump"
    }

    fn name(&self) -> &'static str {
        "Range Finder Jump"
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

    fn has_buffer_contrib(&self) -> bool {
        self.active
    }

    #[allow(clippy::cast_possible_truncation)]
    fn on_notification(&mut self, data: &str) {
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

            self.labels.push(JumpLabel {
                line: line as usize,
                col: col as usize,
                label: label.to_string(),
            });
        }
    }

    fn transform_line(&self, _buf: BufferId, line: usize, text: &str) -> Option<TransformedLine> {
        if !self.active {
            return None;
        }

        // Collect all labels on this line, sorted by column
        let mut line_labels: Vec<&JumpLabel> =
            self.labels.iter().filter(|l| l.line == line).collect();
        if line_labels.is_empty() {
            return None;
        }
        line_labels.sort_by_key(|l| l.col);

        let bright = label_style();
        let dim = label_dim_style();

        let chars: Vec<char> = text.chars().collect();
        let mut segments: Vec<(String, Option<Style>)> = Vec::new();
        let mut pos = 0;

        for label in &line_labels {
            let col = label.col;

            // Skip if label is beyond text
            if col > chars.len() {
                continue;
            }

            // Add text before this label
            if col > pos {
                let before: String = chars[pos..col].iter().collect();
                segments.push((before, None));
            }

            // Add label characters
            let mut label_chars = label.label.chars();
            if let Some(first) = label_chars.next() {
                segments.push((String::from(first), Some(bright.clone())));
                if let Some(second) = label_chars.next() {
                    segments.push((String::from(second), Some(dim.clone())));
                    pos = col + 2;
                } else {
                    pos = col + 1;
                }
            } else {
                pos = col;
            }
        }

        // Add remaining text after the last label
        if pos < chars.len() {
            let remaining: String = chars[pos..].iter().collect();
            segments.push((remaining, None));
        }

        Some(TransformedLine { segments })
    }
}

#[cfg(test)]
mod tests;
