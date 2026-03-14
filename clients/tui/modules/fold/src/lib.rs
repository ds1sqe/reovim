//! Fold rendering module.
//!
//! Receives fold state from the server's `FoldBridge` and provides fold ranges
//! to the viewport renderer. Fold markers are rendered as transformed lines
//! at collapsed fold positions.
//!
//! Migrated from `RangeFinderFoldExtension` (`TuiExtension`) to native
//! `ClientModule` as part of M6 (#637).

use std::collections::HashMap;

use reovim_client_driver::{
    BufferId, ClientModule, ClientModuleError, ModuleContext, ProbeResult, Style, TransformedLine,
    Version,
};

use reovim_arch::Color;

/// A collapsed fold for display.
#[derive(Debug, Clone)]
struct CollapsedFold {
    /// Buffer line where the fold starts (0-indexed).
    start_line: usize,
    /// Number of hidden lines.
    hidden_count: usize,
    /// Preview text (e.g., `fn foo() {`).
    preview: String,
}

/// Fold rendering module.
///
/// Parses fold notifications from the server and provides fold ranges
/// to the viewport renderer. Fold start lines are transformed to show
/// markers like `--- N lines: preview ---`.
///
/// Kind: `"range-finder-fold"` (matches server's `FoldBridge::kind()`).
pub struct FoldModule {
    active: bool,
    /// Per-buffer fold info. Key is buffer ID (parsed from JSON).
    folds: HashMap<usize, Vec<CollapsedFold>>,
    /// Currently active buffer ID.
    active_buffer_id: Option<usize>,
    /// Cached hidden line ranges: `(first_hidden_line, count)`.
    ///
    /// The `first_hidden_line` is `fold.start_line + 1` since the fold
    /// marker line itself remains visible.
    cached_fold_ranges: Vec<(usize, usize)>,
}

impl FoldModule {
    #[must_use]
    pub fn new() -> Self {
        Self {
            active: false,
            folds: HashMap::new(),
            active_buffer_id: None,
            cached_fold_ranges: Vec::new(),
        }
    }

    /// Rebuild the cached fold ranges from the active buffer's folds.
    fn rebuild_hidden_ranges(&mut self) {
        self.cached_fold_ranges.clear();
        let Some(buf_id) = self.active_buffer_id else {
            return;
        };
        let Some(folds) = self.folds.get(&buf_id) else {
            return;
        };
        for fold in folds {
            if fold.hidden_count > 0 {
                // Hidden lines start after the fold marker line
                self.cached_fold_ranges
                    .push((fold.start_line + 1, fold.hidden_count));
            }
        }
    }
}

impl Default for FoldModule {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientModule for FoldModule {
    fn id(&self) -> &'static str {
        "range-finder-fold"
    }

    fn kind(&self) -> &'static str {
        "range-finder-fold"
    }

    fn name(&self) -> &'static str {
        "Range Finder Fold"
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

        self.folds.clear();
        self.active = false;

        let Some(folds_obj) = json.get("folds").and_then(serde_json::Value::as_object) else {
            return;
        };

        for (buf_id_str, collapsed_arr) in folds_obj {
            let Ok(buf_id) = buf_id_str.parse::<usize>() else {
                continue;
            };
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

                folds.push(CollapsedFold {
                    start_line: start_line as usize,
                    hidden_count: hidden_count as usize,
                    preview,
                });
            }

            if !folds.is_empty() {
                self.folds.insert(buf_id, folds);
            }
        }

        self.active = !self.folds.is_empty();

        // If only one buffer has folds, auto-select it
        if self.folds.len() == 1 {
            self.active_buffer_id = self.folds.keys().next().copied();
        }

        self.rebuild_hidden_ranges();
    }

    fn on_buffer_focus(&mut self, buffer_id: BufferId) {
        self.active_buffer_id = Some(buffer_id.0);
        self.rebuild_hidden_ranges();
    }

    fn fold_ranges(&self) -> &[(usize, usize)] {
        &self.cached_fold_ranges
    }

    fn transform_line(&self, _buf: BufferId, line: usize, _text: &str) -> Option<TransformedLine> {
        if !self.active {
            return None;
        }
        let buf_id = self.active_buffer_id?;
        let folds = self.folds.get(&buf_id)?;
        let fold = folds.iter().find(|f| f.start_line == line)?;

        let marker = format!("--- {} lines: {} ---", fold.hidden_count, fold.preview);
        let style = Style::new().fg(Color::DarkGrey);

        Some(TransformedLine {
            segments: vec![(marker, Some(style))],
        })
    }
}

#[cfg(test)]
mod tests;
