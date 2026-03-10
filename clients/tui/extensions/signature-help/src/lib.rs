#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
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
        "signature-help"
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
mod tests {
    use reovim_driver_display::FrameBuffer;

    use super::*;

    // =========================================================================
    // Helpers
    // =========================================================================

    fn active_payload(label: &str) -> String {
        format!(
            r#"{{"active":true,"label":"{label}","origin":{{"BufferPosition":{{"buffer_id":1,"line":5,"col":10}}}}}}"#
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
        let ext = SignatureHelpExtension::new();
        assert!(!ext.is_active());
        assert_eq!(ext.kind(), "signature-help");
    }

    #[test]
    fn default_is_inactive() {
        let ext = SignatureHelpExtension::default();
        assert!(!ext.is_active());
    }

    #[test]
    fn trait_object() {
        let ext: Box<dyn TuiExtension> = Box::new(SignatureHelpExtension::new());
        assert_eq!(ext.kind(), "signature-help");
        assert!(!ext.is_active());
    }

    // =========================================================================
    // apply_notification tests
    // =========================================================================

    #[test]
    fn apply_activates() {
        let mut ext = SignatureHelpExtension::new();
        ext.apply_notification(&active_payload("fn foo(x: i32)"));
        assert!(ext.is_active());
        assert_eq!(ext.label, "fn foo(x: i32)");
        assert_eq!(ext.origin_line, 5);
        assert_eq!(ext.origin_col, 10);
    }

    #[test]
    fn apply_deactivates() {
        let mut ext = SignatureHelpExtension::new();
        ext.apply_notification(&active_payload("fn foo()"));
        assert!(ext.is_active());

        ext.apply_notification(&inactive());
        assert!(!ext.is_active());
        assert!(ext.label.is_empty());
    }

    #[test]
    fn apply_invalid_json() {
        let mut ext = SignatureHelpExtension::new();
        ext.apply_notification("not json{{{");
        assert!(!ext.is_active());
    }

    #[test]
    fn apply_empty_label_deactivates() {
        let mut ext = SignatureHelpExtension::new();
        ext.apply_notification(
            r#"{"active":true,"label":"","origin":{"BufferPosition":{"buffer_id":1,"line":0,"col":0}}}"#,
        );
        assert!(!ext.is_active());
    }

    #[test]
    fn apply_no_origin_keeps_defaults() {
        let mut ext = SignatureHelpExtension::new();
        ext.apply_notification(r#"{"active":true,"label":"fn foo()"}"#);
        assert!(ext.is_active());
        assert_eq!(ext.origin_line, 0);
        assert_eq!(ext.origin_col, 0);
    }

    #[test]
    fn apply_no_label_defaults_empty() {
        let mut ext = SignatureHelpExtension::new();
        ext.apply_notification(
            r#"{"active":true,"origin":{"BufferPosition":{"buffer_id":1,"line":0,"col":0}}}"#,
        );
        // Empty label → deactivated.
        assert!(!ext.is_active());
    }

    #[test]
    fn apply_overwrites_previous() {
        let mut ext = SignatureHelpExtension::new();
        ext.apply_notification(&active_payload("first"));
        ext.apply_notification(&active_payload("second"));
        assert_eq!(ext.label, "second");
    }

    #[test]
    fn apply_deactivation_clears_origin() {
        let mut ext = SignatureHelpExtension::new();
        ext.apply_notification(&active_payload("fn foo()"));
        assert_eq!(ext.origin_line, 5);
        assert_eq!(ext.origin_col, 10);

        ext.apply_notification(&inactive());
        assert_eq!(ext.origin_line, 0);
        assert_eq!(ext.origin_col, 0);
    }

    #[test]
    fn apply_reactivation_without_origin_uses_defaults() {
        let mut ext = SignatureHelpExtension::new();
        ext.apply_notification(&active_payload("first"));
        ext.apply_notification(&inactive());

        // Re-activate without origin — uses default (0, 0) not stale (5, 10).
        ext.apply_notification(r#"{"active":true,"label":"second"}"#);
        assert_eq!(ext.origin_line, 0);
        assert_eq!(ext.origin_col, 0);
    }

    #[test]
    fn apply_null_label() {
        let mut ext = SignatureHelpExtension::new();
        ext.apply_notification(
            r#"{"active":true,"label":null,"origin":{"BufferPosition":{"buffer_id":1,"line":0,"col":0}}}"#,
        );
        // null label → empty → deactivated.
        assert!(!ext.is_active());
    }

    // =========================================================================
    // render tests
    // =========================================================================

    #[test]
    fn render_empty_no_op() {
        let ext = SignatureHelpExtension::new();
        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);
        assert_eq!(fb.get(0, 0).unwrap().char, ' ');
    }

    #[test]
    fn render_shows_popup_above_origin() {
        let mut ext = SignatureHelpExtension::new();
        ext.apply_notification(&active_payload("fn foo(x: i32)"));

        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);

        // Origin at line 5, popup_h=3 → py = 5 - 3 = 2
        // px = 10 (origin_col)
        assert_eq!(fb.get(10, 2).unwrap().char, '\u{256D}');
    }

    #[test]
    fn render_border_color_yellow() {
        let mut ext = SignatureHelpExtension::new();
        ext.apply_notification(&active_payload("fn foo()"));

        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);

        let cell = fb.get(10, 2).unwrap();
        assert_eq!(cell.style.fg, Some(Color::Yellow));
    }

    #[test]
    fn render_content_text() {
        let mut ext = SignatureHelpExtension::new();
        ext.apply_notification(&active_payload("fn foo()"));

        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);

        // Content at (px+1, py+1) = (11, 3)
        let cell = fb.get(11, 3).unwrap();
        assert_eq!(cell.char, 'f');
        assert_eq!(cell.style.fg, Some(Color::White));
    }

    #[test]
    fn render_popup_below_when_no_room_above() {
        let mut ext = SignatureHelpExtension::new();
        // Origin at line 1 → not enough room above (need 3 rows).
        ext.apply_notification(
            r#"{"active":true,"label":"fn foo()","origin":{"BufferPosition":{"buffer_id":1,"line":1,"col":0}}}"#,
        );

        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);

        // anchor_y=1, popup_h=3: 1 < 3, try below: 1+1+3=5 <= 24 → py=2
        assert_eq!(fb.get(0, 2).unwrap().char, '\u{256D}');
    }

    #[test]
    fn render_popup_at_top_fallback() {
        let mut ext = SignatureHelpExtension::new();
        ext.label = "fn foo()".to_owned();
        ext.active = true;
        ext.origin_line = 1;
        ext.origin_col = 0;

        // Tiny terminal — no room above or below.
        let mut fb = FrameBuffer::new(80, 4);
        ext.render(&mut fb);

        // anchor_y=1: above needs 3, below needs 1+1+3=5 > 4. Fallback to 0.
        assert_eq!(fb.get(0, 0).unwrap().char, '\u{256D}');
    }

    #[test]
    fn render_clamps_x_to_screen() {
        let mut ext = SignatureHelpExtension::new();
        ext.apply_notification(
            r#"{"active":true,"label":"fn f()","origin":{"BufferPosition":{"buffer_id":1,"line":5,"col":75}}}"#,
        );

        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);

        // popup_w = 10 (min), max_x = 80 - 10 = 70. col 75 > 70.
        assert_eq!(fb.get(70, 2).unwrap().char, '\u{256D}');
    }

    #[test]
    fn render_long_label_truncated() {
        let mut ext = SignatureHelpExtension::new();
        let long_label = "A".repeat(200);
        ext.apply_notification(&active_payload(&long_label));

        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);

        // popup_w clamped to 78 (width - 2). px = min(10, 80-78) = 2.
        // Border right edge at px + popup_w - 1 = 2 + 77 = 79.
        // Content should not overflow into border cell.
        let cell = fb.get(79, 3).unwrap();
        assert_ne!(cell.char, 'A');
    }

    // =========================================================================
    // Deserialize type coverage
    // =========================================================================

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
        let p = SignatureHelpPayload {
            active: true,
            label: Some("test".into()),
            origin: None,
        };
        assert!(format!("{p:?}").contains("SignatureHelpPayload"));
    }
}
