#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
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
        "hover"
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
mod tests {
    use reovim_driver_display::FrameBuffer;

    use super::*;

    // =========================================================================
    // Helpers
    // =========================================================================

    fn active_plaintext(content: &str) -> String {
        format!(
            r#"{{"active":true,"content":"{content}","contentType":"plaintext","origin":{{"BufferPosition":{{"buffer_id":1,"line":5,"col":10}}}}}}"#
        )
    }

    fn active_markdown(content: &str) -> String {
        format!(
            r#"{{"active":true,"content":"{content}","contentType":"markdown","origin":{{"BufferPosition":{{"buffer_id":2,"line":3,"col":0}}}}}}"#
        )
    }

    fn inactive() -> String {
        r#"{"active":false}"#.to_owned()
    }

    // =========================================================================
    // Basic trait tests
    // =========================================================================

    #[test]
    fn new_is_inactive() {
        let ext = HoverExtension::new();
        assert!(!ext.is_active());
        assert_eq!(ext.kind(), "hover");
    }

    #[test]
    fn default_is_inactive() {
        let ext = HoverExtension::default();
        assert!(!ext.is_active());
    }

    #[test]
    fn trait_object() {
        let ext: Box<dyn TuiExtension> = Box::new(HoverExtension::new());
        assert_eq!(ext.kind(), "hover");
        assert!(!ext.is_active());
    }

    // =========================================================================
    // apply_notification tests
    // =========================================================================

    #[test]
    fn apply_activates_plaintext() {
        let mut ext = HoverExtension::new();
        ext.apply_notification(&active_plaintext("fn foo() -> bool"));
        assert!(ext.is_active());
        assert_eq!(ext.lines.len(), 1);
        assert_eq!(ext.lines[0], "fn foo() -> bool");
        assert_eq!(ext.content_type, ContentType::Plaintext);
        assert_eq!(ext.origin_line, 5);
        assert_eq!(ext.origin_col, 10);
    }

    #[test]
    fn apply_activates_markdown() {
        let mut ext = HoverExtension::new();
        ext.apply_notification(&active_markdown("**bold**"));
        assert!(ext.is_active());
        assert_eq!(ext.content_type, ContentType::Markdown);
        assert_eq!(ext.origin_line, 3);
        assert_eq!(ext.origin_col, 0);
    }

    #[test]
    fn apply_deactivates() {
        let mut ext = HoverExtension::new();
        ext.apply_notification(&active_plaintext("hello"));
        assert!(ext.is_active());

        ext.apply_notification(&inactive());
        assert!(!ext.is_active());
        assert!(ext.lines.is_empty());
    }

    #[test]
    fn apply_invalid_json() {
        let mut ext = HoverExtension::new();
        ext.apply_notification("not json{{{");
        assert!(!ext.is_active());
    }

    #[test]
    fn apply_empty_content_deactivates() {
        let mut ext = HoverExtension::new();
        ext.apply_notification(
            r#"{"active":true,"content":"","contentType":"plaintext","origin":{"BufferPosition":{"buffer_id":1,"line":0,"col":0}}}"#,
        );
        assert!(!ext.is_active());
    }

    #[test]
    fn apply_multiline_content() {
        let mut ext = HoverExtension::new();
        ext.apply_notification(
            r#"{"active":true,"content":"line1\nline2\nline3","contentType":"plaintext","origin":{"BufferPosition":{"buffer_id":1,"line":0,"col":0}}}"#,
        );
        assert!(ext.is_active());
        assert_eq!(ext.lines.len(), 3);
        assert_eq!(ext.lines[0], "line1");
        assert_eq!(ext.lines[2], "line3");
    }

    #[test]
    fn apply_defaults_content_type() {
        let mut ext = HoverExtension::new();
        ext.apply_notification(
            r#"{"active":true,"content":"hello","origin":{"BufferPosition":{"buffer_id":1,"line":0,"col":0}}}"#,
        );
        assert!(ext.is_active());
        assert_eq!(ext.content_type, ContentType::Plaintext);
    }

    #[test]
    fn apply_no_origin_keeps_defaults() {
        let mut ext = HoverExtension::new();
        ext.apply_notification(r#"{"active":true,"content":"hello","contentType":"plaintext"}"#);
        assert!(ext.is_active());
        assert_eq!(ext.origin_line, 0);
        assert_eq!(ext.origin_col, 0);
    }

    #[test]
    fn apply_overwrites_previous() {
        let mut ext = HoverExtension::new();
        ext.apply_notification(&active_plaintext("first"));
        assert_eq!(ext.lines[0], "first");

        ext.apply_notification(&active_markdown("second"));
        assert_eq!(ext.lines[0], "second");
        assert_eq!(ext.content_type, ContentType::Markdown);
    }

    #[test]
    fn apply_caps_at_max_lines() {
        let mut ext = HoverExtension::new();
        let many_lines: Vec<&str> = (0..30).map(|_| "line").collect();
        let content = many_lines.join("\\n");
        let data = format!(
            r#"{{"active":true,"content":"{content}","contentType":"plaintext","origin":{{"BufferPosition":{{"buffer_id":1,"line":0,"col":0}}}}}}"#
        );
        ext.apply_notification(&data);
        assert!(ext.is_active());
        assert_eq!(ext.lines.len(), MAX_LINES);
    }

    #[test]
    fn apply_deactivation_clears_origin() {
        let mut ext = HoverExtension::new();
        ext.apply_notification(&active_plaintext("hello"));
        assert_eq!(ext.origin_line, 5);
        assert_eq!(ext.origin_col, 10);

        ext.apply_notification(&inactive());
        assert_eq!(ext.origin_line, 0);
        assert_eq!(ext.origin_col, 0);
    }

    #[test]
    fn apply_reactivation_without_origin_uses_defaults() {
        let mut ext = HoverExtension::new();
        // First activation sets origin to (5, 10).
        ext.apply_notification(&active_plaintext("first"));
        assert_eq!(ext.origin_line, 5);

        // Deactivate clears origin.
        ext.apply_notification(&inactive());

        // Re-activate without origin field — uses default (0, 0) not stale (5, 10).
        ext.apply_notification(r#"{"active":true,"content":"second","contentType":"plaintext"}"#);
        assert_eq!(ext.origin_line, 0);
        assert_eq!(ext.origin_col, 0);
    }

    #[test]
    fn apply_null_content() {
        let mut ext = HoverExtension::new();
        ext.apply_notification(
            r#"{"active":true,"content":null,"contentType":"plaintext","origin":{"BufferPosition":{"buffer_id":1,"line":0,"col":0}}}"#,
        );
        // null content → empty → deactivated.
        assert!(!ext.is_active());
    }

    // =========================================================================
    // popup_width tests
    // =========================================================================

    #[test]
    fn popup_width_min() {
        let ext = HoverExtension::new();
        // No lines → max line length is 0, desired = 4, clamped to MIN_WIDTH.
        assert_eq!(ext.popup_width(80), MIN_WIDTH);
    }

    #[test]
    fn popup_width_adapts_to_content() {
        let mut ext = HoverExtension::new();
        ext.lines = vec!["a".repeat(30)];
        // desired = 30 + 4 = 34
        let w = ext.popup_width(80);
        assert_eq!(w, 34);
    }

    #[test]
    fn popup_width_clamped_to_max() {
        let mut ext = HoverExtension::new();
        ext.lines = vec!["a".repeat(200)];
        let w = ext.popup_width(80);
        // max_width = 0.6 * 80 = 48
        assert_eq!(w, 48);
    }

    #[test]
    fn popup_width_narrow_terminal() {
        let mut ext = HoverExtension::new();
        ext.lines = vec!["hello".to_owned()];
        let w = ext.popup_width(22);
        assert_eq!(w, MIN_WIDTH);
    }

    #[test]
    fn popup_width_zero_terminal() {
        let ext = HoverExtension::new();
        // Zero-width terminal should not panic.
        assert_eq!(ext.popup_width(0), 0);
    }

    #[test]
    fn popup_width_tiny_terminal() {
        let ext = HoverExtension::new();
        assert_eq!(ext.popup_width(5), 5);
    }

    // =========================================================================
    // render tests
    // =========================================================================

    #[test]
    fn render_empty_no_op() {
        let ext = HoverExtension::new();
        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);
        assert_eq!(fb.get(0, 0).unwrap().char, ' ');
    }

    #[test]
    fn render_single_line() {
        let mut ext = HoverExtension::new();
        ext.apply_notification(&active_plaintext("fn foo()"));

        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);

        // Origin at line 5, col 10 → popup at (10, 6) below origin.
        // Top-left corner at (10, 6)
        assert_eq!(fb.get(10, 6).unwrap().char, '\u{256D}');
    }

    #[test]
    fn render_border_color_plaintext() {
        let mut ext = HoverExtension::new();
        ext.apply_notification(&active_plaintext("hello"));

        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);

        let cell = fb.get(10, 6).unwrap();
        assert_eq!(cell.style.fg, Some(Color::Grey));
    }

    #[test]
    fn render_border_color_markdown() {
        let mut ext = HoverExtension::new();
        ext.apply_notification(&active_markdown("**bold**"));

        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);

        // Origin at line 3, col 0 → popup at (0, 4)
        let cell = fb.get(0, 4).unwrap();
        assert_eq!(cell.style.fg, Some(Color::Cyan));
    }

    #[test]
    fn render_content_text() {
        let mut ext = HoverExtension::new();
        ext.apply_notification(&active_plaintext("hello"));

        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);

        // Content at (11, 7) — px+1, py+1
        let cell = fb.get(11, 7).unwrap();
        assert_eq!(cell.char, 'h');
        assert_eq!(cell.style.fg, Some(Color::White));
    }

    #[test]
    fn render_multiline() {
        let mut ext = HoverExtension::new();
        ext.apply_notification(
            r#"{"active":true,"content":"line1\nline2","contentType":"plaintext","origin":{"BufferPosition":{"buffer_id":1,"line":5,"col":0}}}"#,
        );

        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);

        // popup_h = 2 + 2 = 4, positioned at y=6
        // Bottom border at y=6+3=9
        assert_eq!(fb.get(0, 9).unwrap().char, '\u{2570}');
    }

    #[test]
    fn render_popup_above_when_no_room_below() {
        let mut ext = HoverExtension::new();
        // Origin near bottom of screen.
        ext.apply_notification(
            r#"{"active":true,"content":"text","contentType":"plaintext","origin":{"BufferPosition":{"buffer_id":1,"line":22,"col":0}}}"#,
        );

        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);

        // popup_h = 3 (1 line + 2 border). anchor_y = 22.
        // 22 + 1 + 3 = 26 > 24, so try above: 22 - 3 = 19.
        assert_eq!(fb.get(0, 19).unwrap().char, '\u{256D}');
    }

    #[test]
    fn render_popup_at_top_when_no_room_either_way() {
        let mut ext = HoverExtension::new();
        // Small terminal, origin at line 1, long content.
        ext.lines = (0..5).map(|i| format!("line {i}")).collect();
        ext.active = true;
        ext.origin_line = 1;
        ext.origin_col = 0;
        ext.content_type = ContentType::Plaintext;

        // Terminal height 5 — popup_h = 7 (5 lines + 2 border).
        // Below: 1 + 1 + 7 = 9 > 5. Above: 1 < 7. Fallback to y=0.
        let mut fb = FrameBuffer::new(80, 5);
        ext.render(&mut fb);

        assert_eq!(fb.get(0, 0).unwrap().char, '\u{256D}');
    }

    #[test]
    fn render_clamps_x_to_screen() {
        let mut ext = HoverExtension::new();
        // Origin col far right — popup should clamp x so it fits.
        ext.apply_notification(
            r#"{"active":true,"content":"hello","contentType":"plaintext","origin":{"BufferPosition":{"buffer_id":1,"line":5,"col":75}}}"#,
        );

        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);

        // popup_w = 20 (min), so max_x = 80 - 20 = 60. origin_col = 75 > 60.
        assert_eq!(fb.get(60, 6).unwrap().char, '\u{256D}');
    }

    #[test]
    fn render_long_line_truncated() {
        let mut ext = HoverExtension::new();
        let long = "A".repeat(100);
        ext.apply_notification(&active_plaintext(&long));

        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);

        // Content should not overflow the popup border.
        let popup_w = ext.popup_width(80);
        let px: u16 = 10; // origin_col
        let border_x = px + popup_w - 1;
        assert_ne!(fb.get(border_x, 7).unwrap().char, 'A');
    }

    // =========================================================================
    // Deserialize type coverage
    // =========================================================================

    #[test]
    fn content_type_debug() {
        let ct = ContentType::Markdown;
        assert!(format!("{ct:?}").contains("Markdown"));
    }

    #[test]
    fn content_type_clone_copy_eq() {
        let a = ContentType::Plaintext;
        let b = a;
        #[allow(clippy::clone_on_copy)]
        let c = a.clone();
        assert_eq!(a, b);
        assert_eq!(a, c);
        assert_ne!(ContentType::Plaintext, ContentType::Markdown);
    }

    #[test]
    fn origin_debug() {
        let o = Origin::BufferPosition {
            buffer_id: 1,
            line: 2,
            col: 3,
        };
        assert!(format!("{o:?}").contains("BufferPosition"));
    }

    #[test]
    fn origin_clone() {
        let o = Origin::BufferPosition {
            buffer_id: 1,
            line: 2,
            col: 3,
        };
        #[allow(clippy::redundant_clone)]
        let c = o.clone();
        let Origin::BufferPosition { line, .. } = c;
        assert_eq!(line, 2);
    }

    #[test]
    fn payload_debug() {
        let p = HoverPayload {
            active: true,
            content: Some("test".into()),
            content_type: Some(ContentType::Plaintext),
            origin: None,
        };
        assert!(format!("{p:?}").contains("HoverPayload"));
    }
}
