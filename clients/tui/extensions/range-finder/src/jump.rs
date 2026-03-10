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
mod tests {
    use {super::*, reovim_driver_display::FrameBuffer};

    // =========================================================================
    // Construction and kind
    // =========================================================================

    #[test]
    fn test_new_inactive() {
        let ext = RangeFinderJumpExtension::new();
        assert!(!ext.is_active());
        assert_eq!(ext.kind(), "range-finder-jump");
    }

    #[test]
    fn test_default_inactive() {
        let ext = RangeFinderJumpExtension::default();
        assert!(!ext.is_active());
    }

    #[test]
    fn test_trait_object() {
        let ext: Box<dyn TuiExtension> = Box::new(RangeFinderJumpExtension::new());
        assert_eq!(ext.kind(), "range-finder-jump");
        assert!(!ext.is_active());
    }

    // =========================================================================
    // apply_notification
    // =========================================================================

    #[test]
    fn test_apply_notification_activates() {
        let mut ext = RangeFinderJumpExtension::new();
        ext.apply_notification(r#"{"active":true,"matches":[{"line":0,"col":5,"label":"s"}]}"#);
        assert!(ext.is_active());
        assert_eq!(ext.labels.len(), 1);
        assert_eq!(ext.labels[0].line, 0);
        assert_eq!(ext.labels[0].col, 5);
        assert_eq!(ext.labels[0].label, "s");
    }

    #[test]
    fn test_apply_notification_deactivates() {
        let mut ext = RangeFinderJumpExtension::new();
        ext.apply_notification(r#"{"active":true,"matches":[{"line":0,"col":0,"label":"s"}]}"#);
        assert!(ext.is_active());

        ext.apply_notification(r#"{"active":false}"#);
        assert!(!ext.is_active());
        assert!(ext.labels.is_empty());
    }

    #[test]
    fn test_apply_notification_invalid_json() {
        let mut ext = RangeFinderJumpExtension::new();
        ext.apply_notification("not valid json{{{");
        assert!(!ext.is_active());
    }

    #[test]
    fn test_apply_notification_missing_active() {
        let mut ext = RangeFinderJumpExtension::new();
        ext.apply_notification(r#"{"matches":[]}"#);
        assert!(!ext.is_active());
    }

    #[test]
    fn test_apply_notification_missing_matches() {
        let mut ext = RangeFinderJumpExtension::new();
        ext.apply_notification(r#"{"active":true}"#);
        assert!(ext.is_active());
        assert!(ext.labels.is_empty());
    }

    #[test]
    fn test_apply_notification_empty_matches() {
        let mut ext = RangeFinderJumpExtension::new();
        ext.apply_notification(r#"{"active":true,"matches":[]}"#);
        assert!(ext.is_active());
        assert!(ext.labels.is_empty());
    }

    #[test]
    fn test_apply_notification_multiple_matches() {
        let mut ext = RangeFinderJumpExtension::new();
        ext.apply_notification(
            r#"{"active":true,"matches":[
                {"line":0,"col":0,"label":"s"},
                {"line":1,"col":3,"label":"f"},
                {"line":5,"col":10,"label":"n"}
            ]}"#,
        );
        assert_eq!(ext.labels.len(), 3);
        assert_eq!(ext.labels[1].line, 1);
        assert_eq!(ext.labels[1].col, 3);
        assert_eq!(ext.labels[1].label, "f");
    }

    #[test]
    fn test_apply_notification_match_missing_line() {
        let mut ext = RangeFinderJumpExtension::new();
        ext.apply_notification(r#"{"active":true,"matches":[{"col":0,"label":"s"}]}"#);
        // Match skipped due to missing "line"
        assert!(ext.labels.is_empty());
    }

    #[test]
    fn test_apply_notification_match_missing_col() {
        let mut ext = RangeFinderJumpExtension::new();
        ext.apply_notification(r#"{"active":true,"matches":[{"line":0,"label":"s"}]}"#);
        assert!(ext.labels.is_empty());
    }

    #[test]
    fn test_apply_notification_match_missing_label() {
        let mut ext = RangeFinderJumpExtension::new();
        ext.apply_notification(r#"{"active":true,"matches":[{"line":0,"col":0}]}"#);
        assert!(ext.labels.is_empty());
    }

    #[test]
    fn test_apply_notification_replaces_previous() {
        let mut ext = RangeFinderJumpExtension::new();
        ext.apply_notification(r#"{"active":true,"matches":[{"line":0,"col":0,"label":"s"}]}"#);
        assert_eq!(ext.labels.len(), 1);

        ext.apply_notification(
            r#"{"active":true,"matches":[
                {"line":1,"col":1,"label":"f"},
                {"line":2,"col":2,"label":"n"}
            ]}"#,
        );
        assert_eq!(ext.labels.len(), 2);
        assert_eq!(ext.labels[0].label, "f");
    }

    // =========================================================================
    // render / render_with_viewport
    // =========================================================================

    #[test]
    fn test_render_no_op_without_viewport() {
        let mut ext = RangeFinderJumpExtension::new();
        ext.apply_notification(r#"{"active":true,"matches":[{"line":0,"col":0,"label":"s"}]}"#);
        let mut fb = FrameBuffer::new(20, 10);
        ext.render(&mut fb);
        // render() is a no-op for jump labels (needs viewport)
        assert_eq!(fb.get(0, 0).unwrap().char, ' ');
    }

    #[test]
    fn test_render_labels_at_positions() {
        let mut ext = RangeFinderJumpExtension::new();
        ext.apply_notification(
            r#"{"active":true,"matches":[
                {"line":0,"col":3,"label":"s"},
                {"line":2,"col":0,"label":"f"}
            ]}"#,
        );

        let viewport = ViewportContext {
            scroll_top: 0,
            content_x: 4,
            content_height: 20,
        };
        let mut fb = FrameBuffer::new(40, 20);
        ext.render_with_viewport(&mut fb, &viewport);

        // Label "s" at screen (4+3=7, 0)
        assert_eq!(fb.get(7, 0).unwrap().char, 's');
        assert_eq!(fb.get(7, 0).unwrap().style.bg, Some(Color::Yellow));

        // Label "f" at screen (4+0=4, 2)
        assert_eq!(fb.get(4, 2).unwrap().char, 'f');
    }

    #[test]
    fn test_render_labels_outside_viewport_skipped_above() {
        let mut ext = RangeFinderJumpExtension::new();
        ext.apply_notification(r#"{"active":true,"matches":[{"line":5,"col":0,"label":"s"}]}"#);

        let viewport = ViewportContext {
            scroll_top: 10, // line 5 is above viewport
            content_x: 4,
            content_height: 20,
        };
        let mut fb = FrameBuffer::new(40, 20);
        ext.render_with_viewport(&mut fb, &viewport);

        // Nothing rendered
        assert_eq!(fb.get(4, 0).unwrap().char, ' ');
    }

    #[test]
    fn test_render_labels_outside_viewport_skipped_below() {
        let mut ext = RangeFinderJumpExtension::new();
        ext.apply_notification(r#"{"active":true,"matches":[{"line":30,"col":0,"label":"s"}]}"#);

        let viewport = ViewportContext {
            scroll_top: 0,
            content_x: 4,
            content_height: 20, // only 20 rows, line 30 is below
        };
        let mut fb = FrameBuffer::new(40, 20);
        ext.render_with_viewport(&mut fb, &viewport);

        assert_eq!(fb.get(4, 0).unwrap().char, ' ');
    }

    #[test]
    fn test_render_two_char_labels() {
        let mut ext = RangeFinderJumpExtension::new();
        ext.apply_notification(r#"{"active":true,"matches":[{"line":0,"col":0,"label":"sf"}]}"#);

        let viewport = ViewportContext {
            scroll_top: 0,
            content_x: 2,
            content_height: 10,
        };
        let mut fb = FrameBuffer::new(20, 10);
        ext.render_with_viewport(&mut fb, &viewport);

        // First char at (2, 0) with bright style
        assert_eq!(fb.get(2, 0).unwrap().char, 's');
        assert_eq!(fb.get(2, 0).unwrap().style.bg, Some(Color::Yellow));

        // Second char at (3, 0) with dim style
        assert_eq!(fb.get(3, 0).unwrap().char, 'f');
        assert_ne!(fb.get(3, 0).unwrap().style.bg, Some(Color::Yellow));
    }

    #[test]
    fn test_render_empty_label_no_op() {
        let mut ext = RangeFinderJumpExtension::new();
        ext.apply_notification(r#"{"active":true,"matches":[{"line":0,"col":0,"label":""}]}"#);

        let viewport = ViewportContext {
            scroll_top: 0,
            content_x: 0,
            content_height: 10,
        };
        let mut fb = FrameBuffer::new(20, 10);
        ext.render_with_viewport(&mut fb, &viewport);

        // Empty label renders nothing
        assert_eq!(fb.get(0, 0).unwrap().char, ' ');
    }

    #[test]
    fn test_render_inactive_no_op() {
        let ext = RangeFinderJumpExtension::new();
        let viewport = ViewportContext {
            scroll_top: 0,
            content_x: 0,
            content_height: 20,
        };
        let mut fb = FrameBuffer::new(20, 10);
        ext.render_with_viewport(&mut fb, &viewport);
        assert_eq!(fb.get(0, 0).unwrap().char, ' ');
    }

    #[test]
    fn test_render_active_empty_matches_no_op() {
        let mut ext = RangeFinderJumpExtension::new();
        ext.apply_notification(r#"{"active":true,"matches":[]}"#);
        let viewport = ViewportContext {
            scroll_top: 0,
            content_x: 0,
            content_height: 20,
        };
        let mut fb = FrameBuffer::new(20, 10);
        ext.render_with_viewport(&mut fb, &viewport);
        assert_eq!(fb.get(0, 0).unwrap().char, ' ');
    }

    #[test]
    fn test_render_with_scroll_offset() {
        let mut ext = RangeFinderJumpExtension::new();
        ext.apply_notification(r#"{"active":true,"matches":[{"line":15,"col":5,"label":"s"}]}"#);

        let viewport = ViewportContext {
            scroll_top: 10, // line 15 is at screen row 5
            content_x: 3,
            content_height: 20,
        };
        let mut fb = FrameBuffer::new(40, 20);
        ext.render_with_viewport(&mut fb, &viewport);

        // screen_y = 15 - 10 = 5, screen_x = 3 + 5 = 8
        assert_eq!(fb.get(8, 5).unwrap().char, 's');
    }

    #[test]
    fn test_render_at_viewport_boundary() {
        let mut ext = RangeFinderJumpExtension::new();
        // Label at exactly the last visible row
        ext.apply_notification(r#"{"active":true,"matches":[{"line":9,"col":0,"label":"s"}]}"#);

        let viewport = ViewportContext {
            scroll_top: 0,
            content_x: 0,
            content_height: 10, // rows 0-9 visible
        };
        let mut fb = FrameBuffer::new(20, 10);
        ext.render_with_viewport(&mut fb, &viewport);

        // Line 9 = screen row 9, which is < content_height (10)
        assert_eq!(fb.get(0, 9).unwrap().char, 's');
    }

    #[test]
    fn test_render_just_past_viewport_boundary() {
        let mut ext = RangeFinderJumpExtension::new();
        ext.apply_notification(r#"{"active":true,"matches":[{"line":10,"col":0,"label":"s"}]}"#);

        let viewport = ViewportContext {
            scroll_top: 0,
            content_x: 0,
            content_height: 10, // rows 0-9 visible, line 10 is out
        };
        let mut fb = FrameBuffer::new(20, 11);
        ext.render_with_viewport(&mut fb, &viewport);

        // Line 10 = screen row 10, which is >= content_height (10)
        assert_eq!(fb.get(0, 10).unwrap().char, ' ');
    }

    // =========================================================================
    // Style helpers
    // =========================================================================

    #[test]
    fn test_label_style() {
        let style = label_style();
        assert_eq!(style.fg, Some(Color::Black));
        assert_eq!(style.bg, Some(Color::Yellow));
    }

    #[test]
    fn test_label_dim_style() {
        let style = label_dim_style();
        assert!(style.fg.is_some());
        assert!(style.bg.is_some());
        // Dim style should differ from bright style
        assert_ne!(style.bg, Some(Color::Yellow));
    }

    // =========================================================================
    // Default trait methods
    // =========================================================================

    #[test]
    fn test_default_tick() {
        let mut ext = RangeFinderJumpExtension::new();
        assert!(!ext.tick());
    }

    #[test]
    fn test_default_cursor_position() {
        let ext = RangeFinderJumpExtension::new();
        assert!(ext.cursor_position(80, 24).is_none());
    }

    #[test]
    fn test_default_content_offset_left() {
        let ext = RangeFinderJumpExtension::new();
        assert_eq!(ext.content_offset_left(), 0);
    }
}
