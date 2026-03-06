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
mod tests {
    use {super::*, reovim_driver_display::FrameBuffer};

    // =========================================================================
    // Construction and kind
    // =========================================================================

    #[test]
    fn test_fold_new_inactive() {
        let ext = RangeFinderFoldExtension::new();
        assert!(!ext.is_active());
        assert_eq!(ext.kind(), "range-finder-fold");
    }

    #[test]
    fn test_fold_default_inactive() {
        let ext = RangeFinderFoldExtension::default();
        assert!(!ext.is_active());
    }

    #[test]
    fn test_fold_trait_object() {
        let ext: Box<dyn TuiExtension> = Box::new(RangeFinderFoldExtension::new());
        assert_eq!(ext.kind(), "range-finder-fold");
        assert!(!ext.is_active());
    }

    // =========================================================================
    // apply_notification
    // =========================================================================

    #[test]
    fn test_fold_apply_notification_with_folds() {
        let mut ext = RangeFinderFoldExtension::new();
        ext.apply_notification(
            r#"{"folds":{"1":[{"start_line":5,"hidden_count":3,"preview":"fn foo() {"}]}}"#,
        );
        assert!(ext.is_active());
        assert_eq!(ext.folds.len(), 1);
        assert!(ext.folds.contains_key("1"));
        assert_eq!(ext.folds["1"][0].start_line, 5);
        assert_eq!(ext.folds["1"][0].hidden_count, 3);
        assert_eq!(ext.folds["1"][0].preview, "fn foo() {");
    }

    #[test]
    fn test_fold_apply_notification_no_folds() {
        let mut ext = RangeFinderFoldExtension::new();
        ext.apply_notification(r#"{"folds":{}}"#);
        assert!(!ext.is_active());
    }

    #[test]
    fn test_fold_apply_notification_missing_folds_key() {
        let mut ext = RangeFinderFoldExtension::new();
        ext.apply_notification(r#"{"active":true}"#);
        assert!(!ext.is_active());
    }

    #[test]
    fn test_fold_apply_notification_invalid_json() {
        let mut ext = RangeFinderFoldExtension::new();
        ext.apply_notification("not json{{{");
        assert!(!ext.is_active());
    }

    #[test]
    fn test_fold_apply_notification_replaces_previous() {
        let mut ext = RangeFinderFoldExtension::new();
        ext.apply_notification(
            r#"{"folds":{"1":[{"start_line":0,"hidden_count":5,"preview":"a"}]}}"#,
        );
        assert_eq!(ext.folds.len(), 1);

        ext.apply_notification(
            r#"{"folds":{"2":[{"start_line":10,"hidden_count":2,"preview":"b"}]}}"#,
        );
        assert_eq!(ext.folds.len(), 1);
        assert!(ext.folds.contains_key("2"));
        assert!(!ext.folds.contains_key("1"));
    }

    #[test]
    fn test_fold_apply_notification_missing_start_line() {
        let mut ext = RangeFinderFoldExtension::new();
        ext.apply_notification(r#"{"folds":{"1":[{"hidden_count":5,"preview":"a"}]}}"#);
        // Entry skipped, no folds parsed
        assert!(!ext.is_active());
    }

    #[test]
    fn test_fold_apply_notification_missing_hidden_count() {
        let mut ext = RangeFinderFoldExtension::new();
        ext.apply_notification(r#"{"folds":{"1":[{"start_line":0,"preview":"a"}]}}"#);
        assert!(!ext.is_active());
    }

    #[test]
    fn test_fold_apply_notification_missing_preview_defaults_empty() {
        let mut ext = RangeFinderFoldExtension::new();
        ext.apply_notification(r#"{"folds":{"1":[{"start_line":0,"hidden_count":5}]}}"#);
        assert!(ext.is_active());
        assert_eq!(ext.folds["1"][0].preview, "");
    }

    #[test]
    fn test_fold_apply_notification_multi_buffer() {
        let mut ext = RangeFinderFoldExtension::new();
        ext.apply_notification(
            r#"{"folds":{
                "1":[{"start_line":0,"hidden_count":3,"preview":"fn a"}],
                "2":[{"start_line":10,"hidden_count":5,"preview":"fn b"}]
            }}"#,
        );
        assert!(ext.is_active());
        assert_eq!(ext.folds.len(), 2);
    }

    #[test]
    fn test_fold_apply_notification_non_array_value() {
        let mut ext = RangeFinderFoldExtension::new();
        ext.apply_notification(r#"{"folds":{"1":"not an array"}}"#);
        assert!(!ext.is_active());
    }

    #[test]
    fn test_fold_auto_selects_single_buffer() {
        let mut ext = RangeFinderFoldExtension::new();
        ext.apply_notification(
            r#"{"folds":{"42":[{"start_line":0,"hidden_count":3,"preview":"fn a"}]}}"#,
        );
        assert_eq!(ext.active_buffer_id.as_deref(), Some("42"));
    }

    #[test]
    fn test_fold_set_active_buffer() {
        let mut ext = RangeFinderFoldExtension::new();
        ext.set_active_buffer("5");
        assert_eq!(ext.active_buffer_id.as_deref(), Some("5"));
    }

    // =========================================================================
    // render / render_with_viewport
    // =========================================================================

    #[test]
    fn test_fold_render_no_op_without_viewport() {
        let mut ext = RangeFinderFoldExtension::new();
        ext.apply_notification(
            r#"{"folds":{"1":[{"start_line":0,"hidden_count":3,"preview":"fn foo"}]}}"#,
        );
        let mut fb = FrameBuffer::new(40, 10);
        ext.render(&mut fb);
        // render() is a no-op
        assert_eq!(fb.get(0, 0).unwrap().char, ' ');
    }

    #[test]
    fn test_fold_render_marker() {
        let mut ext = RangeFinderFoldExtension::new();
        ext.apply_notification(
            r#"{"folds":{"1":[{"start_line":2,"hidden_count":6,"preview":"fn foo() {"}]}}"#,
        );

        let viewport = ViewportContext {
            scroll_top: 0,
            content_x: 4,
            content_height: 20,
        };
        let mut fb = FrameBuffer::new(60, 20);
        ext.render_with_viewport(&mut fb, &viewport);

        // Marker at row 2, starting at content_x=4
        // Should contain "--- 6 lines: fn foo() { ---"
        assert_eq!(fb.get(4, 2).unwrap().char, '-');
        assert_eq!(fb.get(5, 2).unwrap().char, '-');
        assert_eq!(fb.get(6, 2).unwrap().char, '-');
        assert_eq!(fb.get(7, 2).unwrap().char, ' ');
        assert_eq!(fb.get(8, 2).unwrap().char, '6');
    }

    #[test]
    fn test_fold_render_outside_viewport_above() {
        let mut ext = RangeFinderFoldExtension::new();
        ext.apply_notification(
            r#"{"folds":{"1":[{"start_line":2,"hidden_count":3,"preview":"fn foo"}]}}"#,
        );

        let viewport = ViewportContext {
            scroll_top: 10,
            content_x: 0,
            content_height: 20,
        };
        let mut fb = FrameBuffer::new(40, 20);
        ext.render_with_viewport(&mut fb, &viewport);

        // Nothing rendered (line 2 is above viewport at scroll_top=10)
        assert_eq!(fb.get(0, 0).unwrap().char, ' ');
    }

    #[test]
    fn test_fold_render_outside_viewport_below() {
        let mut ext = RangeFinderFoldExtension::new();
        ext.apply_notification(
            r#"{"folds":{"1":[{"start_line":30,"hidden_count":3,"preview":"fn foo"}]}}"#,
        );

        let viewport = ViewportContext {
            scroll_top: 0,
            content_x: 0,
            content_height: 20,
        };
        let mut fb = FrameBuffer::new(40, 20);
        ext.render_with_viewport(&mut fb, &viewport);

        assert_eq!(fb.get(0, 0).unwrap().char, ' ');
    }

    #[test]
    fn test_fold_render_inactive_no_op() {
        let ext = RangeFinderFoldExtension::new();
        let viewport = ViewportContext {
            scroll_top: 0,
            content_x: 0,
            content_height: 20,
        };
        let mut fb = FrameBuffer::new(40, 10);
        ext.render_with_viewport(&mut fb, &viewport);
        assert_eq!(fb.get(0, 0).unwrap().char, ' ');
    }

    #[test]
    fn test_fold_render_no_active_buffer() {
        let mut ext = RangeFinderFoldExtension::new();
        ext.apply_notification(
            r#"{"folds":{
                "1":[{"start_line":0,"hidden_count":3,"preview":"a"}],
                "2":[{"start_line":5,"hidden_count":2,"preview":"b"}]
            }}"#,
        );
        // Multi-buffer: auto-select doesn't apply
        ext.active_buffer_id = None;

        let viewport = ViewportContext {
            scroll_top: 0,
            content_x: 0,
            content_height: 20,
        };
        let mut fb = FrameBuffer::new(40, 10);
        ext.render_with_viewport(&mut fb, &viewport);
        // No buffer selected, nothing rendered
        assert_eq!(fb.get(0, 0).unwrap().char, ' ');
    }

    #[test]
    fn test_fold_render_wrong_buffer_id() {
        let mut ext = RangeFinderFoldExtension::new();
        ext.apply_notification(
            r#"{"folds":{"1":[{"start_line":0,"hidden_count":3,"preview":"a"}]}}"#,
        );
        ext.active_buffer_id = Some("999".to_string());

        let viewport = ViewportContext {
            scroll_top: 0,
            content_x: 0,
            content_height: 20,
        };
        let mut fb = FrameBuffer::new(40, 10);
        ext.render_with_viewport(&mut fb, &viewport);
        assert_eq!(fb.get(0, 0).unwrap().char, ' ');
    }

    // =========================================================================
    // Style helpers
    // =========================================================================

    #[test]
    fn test_fold_marker_style() {
        let style = fold_marker_style();
        assert_eq!(style.fg, Some(Color::DarkGrey));
    }

    // =========================================================================
    // Default trait methods
    // =========================================================================

    #[test]
    fn test_fold_default_tick() {
        let mut ext = RangeFinderFoldExtension::new();
        assert!(!ext.tick());
    }

    #[test]
    fn test_fold_default_cursor_position() {
        let ext = RangeFinderFoldExtension::new();
        assert!(ext.cursor_position(80, 24).is_none());
    }

    #[test]
    fn test_fold_default_content_offset_left() {
        let ext = RangeFinderFoldExtension::new();
        assert_eq!(ext.content_offset_left(), 0);
    }

    // =========================================================================
    // fold_hidden_lines
    // =========================================================================

    #[test]
    fn test_fold_hidden_lines_empty_when_inactive() {
        let ext = RangeFinderFoldExtension::new();
        assert!(ext.fold_hidden_lines().is_empty());
    }

    #[test]
    fn test_fold_hidden_lines_returns_ranges() {
        let mut ext = RangeFinderFoldExtension::new();
        ext.apply_notification(
            r#"{"folds":{"1":[{"start_line":5,"hidden_count":3,"preview":"fn foo"}]}}"#,
        );
        let ranges = ext.fold_hidden_lines();
        assert_eq!(ranges.len(), 1);
        // Hidden lines start at start_line + 1 (fold marker line is visible)
        assert_eq!(ranges[0], (6, 3));
    }

    #[test]
    fn test_fold_hidden_lines_multiple_folds() {
        let mut ext = RangeFinderFoldExtension::new();
        ext.apply_notification(
            r#"{"folds":{"1":[
                {"start_line":2,"hidden_count":4,"preview":"a"},
                {"start_line":10,"hidden_count":2,"preview":"b"}
            ]}}"#,
        );
        let ranges = ext.fold_hidden_lines();
        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0], (3, 4));
        assert_eq!(ranges[1], (11, 2));
    }

    #[test]
    fn test_fold_hidden_lines_zero_count_excluded() {
        let mut ext = RangeFinderFoldExtension::new();
        ext.apply_notification(
            r#"{"folds":{"1":[{"start_line":5,"hidden_count":0,"preview":"empty"}]}}"#,
        );
        let ranges = ext.fold_hidden_lines();
        assert!(ranges.is_empty());
    }

    #[test]
    fn test_fold_hidden_lines_no_active_buffer() {
        let mut ext = RangeFinderFoldExtension::new();
        ext.apply_notification(
            r#"{"folds":{
                "1":[{"start_line":0,"hidden_count":3,"preview":"a"}],
                "2":[{"start_line":5,"hidden_count":2,"preview":"b"}]
            }}"#,
        );
        // Multi-buffer: auto-select doesn't apply
        ext.active_buffer_id = None;
        ext.rebuild_hidden_ranges();
        assert!(ext.fold_hidden_lines().is_empty());
    }

    #[test]
    fn test_fold_hidden_lines_wrong_buffer() {
        let mut ext = RangeFinderFoldExtension::new();
        ext.apply_notification(
            r#"{"folds":{"1":[{"start_line":0,"hidden_count":3,"preview":"a"}]}}"#,
        );
        ext.active_buffer_id = Some("999".to_string());
        ext.rebuild_hidden_ranges();
        assert!(ext.fold_hidden_lines().is_empty());
    }

    #[test]
    fn test_fold_hidden_lines_updated_on_set_active_buffer() {
        let mut ext = RangeFinderFoldExtension::new();
        ext.apply_notification(
            r#"{"folds":{
                "1":[{"start_line":0,"hidden_count":3,"preview":"a"}],
                "2":[{"start_line":10,"hidden_count":5,"preview":"b"}]
            }}"#,
        );
        // Initially no active buffer (multi-buffer, no auto-select)
        ext.active_buffer_id = None;
        ext.rebuild_hidden_ranges();
        assert!(ext.fold_hidden_lines().is_empty());

        ext.set_active_buffer("2");
        let ranges = ext.fold_hidden_lines();
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0], (11, 5));
    }

    #[test]
    fn test_fold_hidden_lines_cleared_on_deactivation() {
        let mut ext = RangeFinderFoldExtension::new();
        ext.apply_notification(
            r#"{"folds":{"1":[{"start_line":5,"hidden_count":3,"preview":"a"}]}}"#,
        );
        assert!(!ext.fold_hidden_lines().is_empty());

        // Deactivate with empty folds
        ext.apply_notification(r#"{"folds":{}}"#);
        assert!(ext.fold_hidden_lines().is_empty());
    }
}
