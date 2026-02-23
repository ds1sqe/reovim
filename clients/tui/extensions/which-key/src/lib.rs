#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Which-key TUI extension.
//!
//! Displays a popup showing available key continuations when a prefix
//! key is held (e.g., `g` shows `gg`, `gd`, etc.).
//!
//! This crate is a self-contained TUI extension: it owns its state,
//! parses notifications, and renders through `RenderBackend`.
//! The engine has ZERO knowledge of this crate.

use {
    reovim_arch::Color,
    reovim_driver_display::{
        Style,
        popup_utils::{popup_width, popup_x, render_box_border},
        render_backend::{RenderBackend, TuiExtension},
        ui::truncate_end,
    },
};

/// Which-key popup extension.
///
/// Shows available key continuations when a prefix key is pending.
pub struct WhichKeyExtension {
    /// Whether the popup is currently visible.
    active: bool,
    /// Pending key prefix (e.g., "g").
    prefix: String,
    /// Available continuations: `(key, command)` pairs.
    hints: Vec<(String, String)>,
}

impl WhichKeyExtension {
    /// Create a new which-key extension (inactive).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            active: false,
            prefix: String::new(),
            hints: Vec::new(),
        }
    }
}

impl Default for WhichKeyExtension {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiExtension for WhichKeyExtension {
    fn kind(&self) -> &'static str {
        "whichkey"
    }

    fn is_active(&self) -> bool {
        self.active
    }

    #[allow(clippy::cast_possible_truncation)]
    fn apply_notification(&mut self, data: &str) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
            self.active = json
                .get("active")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            self.prefix = json
                .get("prefix")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string();
            self.hints = json
                .get("hints")
                .and_then(serde_json::Value::as_array)
                .map_or_else(Vec::new, |arr| {
                    arr.iter()
                        .filter_map(|h| {
                            let key = h.get("key")?.as_str()?;
                            let cmd = h.get("command")?.as_str()?;
                            Some((key.to_string(), cmd.to_string()))
                        })
                        .collect()
                });
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn render(&self, backend: &mut dyn RenderBackend) {
        if self.hints.is_empty() {
            return;
        }

        let (width, height) = backend.size();

        let hint_count = self.hints.len() as u16;
        // popup height: top border + hints + bottom border
        let popup_height = hint_count + 2;

        let pw = popup_width(width);
        let px = popup_x(width, pw);
        // Position above the statusline (bottom-anchored)
        let py = height.saturating_sub(1 + popup_height);

        let border_style = Style::default().fg(Color::DarkGrey);

        // Draw border
        render_box_border(backend, px, py, pw, popup_height, &border_style);

        // Draw title in the top border: "--- g ---"
        if !self.prefix.is_empty() {
            let title = format!(" {} ", self.prefix);
            let title_x = px + 3; // after "---"
            let title_style = Style::default().fg(Color::Yellow);
            backend.write_str(title_x, py, &title, &title_style);
        }

        // Draw hints
        let content_x = px + 2; // border + padding
        let content_width = pw.saturating_sub(4); // 2 padding each side

        let key_style = Style::default().fg(Color::Cyan);
        let cmd_style = Style::default().fg(Color::White);
        let clear_style = Style::default();

        for (i, (key, command)) in self.hints.iter().enumerate() {
            let row_y = py + 1 + i as u16;

            // Clear the row
            for col in 0..content_width {
                backend.set_cell(content_x + col, row_y, ' ', &clear_style);
            }

            // Key column (left-aligned, max 6 chars)
            let key_display = truncate_end(key, 6);
            backend.write_str(content_x, row_y, &key_display, &key_style);

            // Command column (after key + gap)
            let cmd_x = content_x + 7; // 6 chars for key + 1 space gap
            if cmd_x < px + pw - 2 {
                let max_cmd_width = (px + pw - 2).saturating_sub(cmd_x) as usize;
                let cmd_display = truncate_end(command, max_cmd_width);
                backend.write_str(cmd_x, row_y, &cmd_display, &cmd_style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use reovim_driver_display::FrameBuffer;

    use super::*;

    #[test]
    fn test_new_is_inactive() {
        let ext = WhichKeyExtension::new();
        assert!(!ext.is_active());
        assert_eq!(ext.kind(), "whichkey");
    }

    #[test]
    fn test_default_is_inactive() {
        let ext = WhichKeyExtension::default();
        assert!(!ext.is_active());
    }

    #[test]
    fn test_apply_notification_activates() {
        let mut ext = WhichKeyExtension::new();
        let data = r#"{"active":true,"prefix":"g","hints":[{"key":"g","command":"goto-top"},{"key":"d","command":"goto-definition"}]}"#;
        ext.apply_notification(data);

        assert!(ext.is_active());
        assert_eq!(ext.prefix, "g");
        assert_eq!(ext.hints.len(), 2);
        assert_eq!(ext.hints[0].0, "g");
        assert_eq!(ext.hints[0].1, "goto-top");
        assert_eq!(ext.hints[1].0, "d");
        assert_eq!(ext.hints[1].1, "goto-definition");
    }

    #[test]
    fn test_apply_notification_deactivates() {
        let mut ext = WhichKeyExtension::new();
        // Activate
        ext.apply_notification(
            r#"{"active":true,"prefix":"g","hints":[{"key":"g","command":"top"}]}"#,
        );
        assert!(ext.is_active());

        // Deactivate
        ext.apply_notification(r#"{"active":false}"#);
        assert!(!ext.is_active());
    }

    #[test]
    fn test_apply_notification_invalid_json() {
        let mut ext = WhichKeyExtension::new();
        ext.apply_notification("not json");
        assert!(!ext.is_active());
    }

    #[test]
    fn test_apply_notification_missing_hints() {
        let mut ext = WhichKeyExtension::new();
        ext.apply_notification(r#"{"active":true,"prefix":"g"}"#);
        assert!(ext.is_active());
        assert!(ext.hints.is_empty());
    }

    #[test]
    fn test_apply_notification_malformed_hint() {
        let mut ext = WhichKeyExtension::new();
        ext.apply_notification(
            r#"{"active":true,"prefix":"g","hints":[{"key":"g"},{"bad":"data"}]}"#,
        );
        assert!(ext.is_active());
        // Malformed hints are filtered out
        assert!(ext.hints.is_empty());
    }

    #[test]
    fn test_render_not_shown_when_inactive() {
        let ext = WhichKeyExtension::new();
        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);
        // Should be all spaces (no rendering)
        assert_eq!(fb.get(0, 0).unwrap().char, ' ');
    }

    #[test]
    fn test_render_not_shown_with_empty_hints() {
        let mut ext = WhichKeyExtension::new();
        ext.active = true;
        ext.prefix = "g".to_string();
        // Empty hints -> don't render
        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);
        assert_eq!(fb.get(0, 0).unwrap().char, ' ');
    }

    #[test]
    fn test_render_shows_popup() {
        let mut ext = WhichKeyExtension::new();
        ext.apply_notification(
            r#"{"active":true,"prefix":"g","hints":[{"key":"g","command":"goto-top"},{"key":"d","command":"goto-definition"}]}"#,
        );

        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);

        // Popup should be visible near bottom
        // Check for border characters
        let pw = popup_width(80);
        let px = popup_x(80, pw);
        // popup height = 2 hints + 2 borders = 4
        // py = 24 - 1 - 4 = 19
        let py = 19;

        // Top-left corner
        assert_eq!(fb.get(px, py).unwrap().char, '\u{256D}');
        // Top-right corner
        assert_eq!(fb.get(px + pw - 1, py).unwrap().char, '\u{256E}');
        // Bottom-left corner
        assert_eq!(fb.get(px, py + 3).unwrap().char, '\u{2570}');
    }

    #[test]
    fn test_render_no_prefix() {
        let mut ext = WhichKeyExtension::new();
        ext.active = true;
        ext.hints = vec![("a".to_string(), "cmd-a".to_string())];
        // No prefix set

        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);

        // Should still render the popup (just no title)
        let pw = popup_width(80);
        let px = popup_x(80, pw);
        let py = 24 - 1 - 3; // 1 hint + 2 borders = 3
        assert_eq!(fb.get(px, py).unwrap().char, '\u{256D}');
    }

    #[test]
    fn test_render_command_text() {
        let mut ext = WhichKeyExtension::new();
        ext.apply_notification(
            r#"{"active":true,"prefix":"g","hints":[{"key":"g","command":"goto-top"}]}"#,
        );

        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);

        // Check command text is rendered
        let pw = popup_width(80);
        let px = popup_x(80, pw);
        let py = 24 - 1 - 3; // 1 hint + 2 borders
        let content_x = px + 2;
        let cmd_x = content_x + 7;

        // 'g' should appear somewhere near content_x on the hint row
        let hint_row_y = py + 1;
        assert_eq!(fb.get(content_x, hint_row_y).unwrap().char, 'g');
        // 'goto-top' should start at cmd_x
        assert_eq!(fb.get(cmd_x, hint_row_y).unwrap().char, 'g');
    }

    #[test]
    fn test_trait_object() {
        let ext: Box<dyn TuiExtension> = Box::new(WhichKeyExtension::new());
        assert_eq!(ext.kind(), "whichkey");
        assert!(!ext.is_active());
    }
}
