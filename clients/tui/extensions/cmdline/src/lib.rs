#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Cmdline TUI extension.
//!
//! Displays a floating command-line popup for ex-commands (`:`, `/`, `?`).
//! Includes input text, cursor, and completion candidates.
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
    },
};

/// Cmdline popup extension.
///
/// Shows a command-line input popup with prompt, cursor, and completions.
pub struct CmdlineExtension {
    /// Whether the popup is currently visible.
    active: bool,
    /// Prompt character (":", "/", "?").
    prompt: String,
    /// Input text.
    input: String,
    /// Cursor position (char index into input).
    cursor: usize,
    /// Completion candidates.
    completions: Vec<String>,
    /// Currently selected completion index.
    completion_index: Option<usize>,
}

impl CmdlineExtension {
    /// Create a new cmdline extension (inactive).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            active: false,
            prompt: String::new(),
            input: String::new(),
            cursor: 0,
            completions: Vec::new(),
            completion_index: None,
        }
    }
}

impl Default for CmdlineExtension {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiExtension for CmdlineExtension {
    fn kind(&self) -> &'static str {
        "cmdline"
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
            self.prompt = json
                .get("prompt")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(":")
                .to_string();
            self.input = json
                .get("input")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string();
            self.cursor = json
                .get("cursor")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0) as usize;
            self.completions = json
                .get("completions")
                .and_then(serde_json::Value::as_array)
                .map_or_else(Vec::new, |arr| {
                    arr.iter()
                        .filter_map(serde_json::Value::as_str)
                        .map(String::from)
                        .collect()
                });
            self.completion_index = json
                .get("completion_index")
                .and_then(serde_json::Value::as_u64)
                .map(|v| v as usize);
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn cursor_position(&self, terminal_width: u16, _terminal_height: u16) -> Option<(u16, u16)> {
        if !self.active {
            return None;
        }
        let pw = popup_width(terminal_width);
        let px = popup_x(terminal_width, pw);
        let content_x = px + 2; // border + padding
        let prompt_len = self.prompt.len() as u16;
        let cursor_x = content_x + prompt_len + self.cursor as u16;
        let cursor_y = 2; // popup at y=1, content row at y=2
        Some((cursor_x, cursor_y))
    }

    #[allow(clippy::cast_possible_truncation)]
    fn render(&self, backend: &mut dyn RenderBackend) {
        let (width, _height) = backend.size();

        let pw = popup_width(width);
        let px = popup_x(width, pw);
        let py: u16 = 1; // Near top of screen

        // Popup height: 3 (border + content + border) + completion rows
        let completion_rows = self.completions.len() as u16;
        let popup_height = 3 + completion_rows;

        // Draw border
        let border_style = Style::default().fg(Color::DarkGrey);
        render_box_border(backend, px, py, pw, popup_height, &border_style);

        // Content row (inside the border)
        let content_y = py + 1;
        let content_x = px + 2; // border + padding
        let content_width = pw.saturating_sub(4); // 2 padding each side

        // Clear content area
        let clear_style = Style::default();
        for col in 0..content_width {
            backend.set_cell(content_x + col, content_y, ' ', &clear_style);
        }

        // Prompt character in yellow
        let prompt_style = Style::default().fg(Color::Yellow);
        backend.write_str(content_x, content_y, &self.prompt, &prompt_style);

        let prompt_len = self.prompt.len() as u16;
        let input_style = Style::default();

        // Render input text
        for (i, ch) in self.input.chars().enumerate() {
            let x = content_x + prompt_len + i as u16;
            if x >= content_x + content_width {
                break;
            }
            backend.set_cell(x, content_y, ch, &input_style);
        }

        // Render block cursor
        let cursor_style = Style::default().bg(Color::White).fg(Color::Black);
        let cursor_x = content_x + prompt_len + self.cursor as u16;
        if cursor_x < content_x + content_width {
            if self.cursor < self.input.len() {
                backend.apply_style(cursor_x, content_y, &cursor_style);
            } else {
                backend.set_cell(cursor_x, content_y, ' ', &cursor_style);
            }
        }

        // Render completions below the input line
        for (i, completion) in self.completions.iter().enumerate() {
            let row_y = content_y + 1 + i as u16;
            let is_selected = self.completion_index == Some(i);

            // Clear the row
            for col in 0..content_width {
                backend.set_cell(content_x + col, row_y, ' ', &clear_style);
            }

            // Indicator: > for selected, space for others
            let indicator = if is_selected { '\u{25B8}' } else { ' ' };
            let item_style = if is_selected {
                Style::default().bg(Color::DarkGrey).fg(Color::White)
            } else {
                Style::default()
            };

            backend.set_cell(content_x, row_y, indicator, &item_style);
            // Render completion text with 2-char offset (indicator + space)
            for (j, ch) in completion.chars().enumerate() {
                let x = content_x + 2 + j as u16;
                if x >= content_x + content_width {
                    break;
                }
                backend.set_cell(x, row_y, ch, &item_style);
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
        let ext = CmdlineExtension::new();
        assert!(!ext.is_active());
        assert_eq!(ext.kind(), "cmdline");
    }

    #[test]
    fn test_default_is_inactive() {
        let ext = CmdlineExtension::default();
        assert!(!ext.is_active());
    }

    #[test]
    fn test_apply_notification_activates() {
        let mut ext = CmdlineExtension::new();
        let data = r#"{"active":true,"prompt":":","input":"wq","cursor":2}"#;
        ext.apply_notification(data);

        assert!(ext.is_active());
        assert_eq!(ext.prompt, ":");
        assert_eq!(ext.input, "wq");
        assert_eq!(ext.cursor, 2);
    }

    #[test]
    fn test_apply_notification_deactivates() {
        let mut ext = CmdlineExtension::new();
        ext.apply_notification(r#"{"active":true,"prompt":":","input":"wq","cursor":2}"#);
        assert!(ext.is_active());

        ext.apply_notification(r#"{"active":false}"#);
        assert!(!ext.is_active());
    }

    #[test]
    fn test_apply_notification_invalid_json() {
        let mut ext = CmdlineExtension::new();
        ext.apply_notification("not json");
        assert!(!ext.is_active());
    }

    #[test]
    fn test_apply_notification_search_prompt() {
        let mut ext = CmdlineExtension::new();
        ext.apply_notification(r#"{"active":true,"prompt":"/","input":"hello","cursor":5}"#);
        assert_eq!(ext.prompt, "/");
        assert_eq!(ext.input, "hello");
    }

    #[test]
    fn test_apply_notification_with_completions() {
        let mut ext = CmdlineExtension::new();
        ext.apply_notification(
            r#"{"active":true,"prompt":":","input":"w","cursor":1,"completions":["write","wq","wall"],"completion_index":0}"#,
        );
        assert_eq!(ext.completions.len(), 3);
        assert_eq!(ext.completions[0], "write");
        assert_eq!(ext.completion_index, Some(0));
    }

    #[test]
    fn test_apply_notification_client_id_zero_is_local() {
        // client_id check is done by the engine, not the extension.
        // Extension just parses data. This tests that parsing works.
        let mut ext = CmdlineExtension::new();
        ext.apply_notification(r#"{"active":true,"prompt":":","input":"q","cursor":1}"#);
        assert!(ext.is_active());
    }

    #[test]
    fn test_render_shows_popup() {
        let mut ext = CmdlineExtension::new();
        ext.apply_notification(r#"{"active":true,"prompt":":","input":"wq","cursor":2}"#);

        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);

        // Check border at py=1
        let pw = popup_width(80);
        let px = popup_x(80, pw);
        assert_eq!(fb.get(px, 1).unwrap().char, '\u{256D}'); // top-left corner
        assert_eq!(fb.get(px + pw - 1, 1).unwrap().char, '\u{256E}'); // top-right corner

        // Check prompt character
        let content_x = px + 2;
        assert_eq!(fb.get(content_x, 2).unwrap().char, ':');
    }

    #[test]
    fn test_render_cursor_in_middle() {
        let mut ext = CmdlineExtension::new();
        ext.apply_notification(r#"{"active":true,"prompt":":","input":"hello","cursor":2}"#);

        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);

        // Cursor should be at position 2 in the input (on 'l')
        let pw = popup_width(80);
        let px = popup_x(80, pw);
        let content_x = px + 2;
        let cursor_x = content_x + 1 + 2; // prompt_len(1) + cursor(2)

        // The cell should have cursor style (white bg)
        let cell = fb.get(cursor_x, 2).unwrap();
        assert_eq!(cell.char, 'l'); // character preserved
        assert_eq!(cell.style.bg, Some(Color::White)); // cursor bg
    }

    #[test]
    fn test_render_with_completions() {
        let mut ext = CmdlineExtension::new();
        ext.apply_notification(
            r#"{"active":true,"prompt":":","input":"w","cursor":1,"completions":["write","wq"],"completion_index":0}"#,
        );

        let mut fb = FrameBuffer::new(80, 24);
        ext.render(&mut fb);

        // Check that the popup height includes completion rows
        let pw = popup_width(80);
        let px = popup_x(80, pw);
        // Height = 3 (border+content+border) + 2 completions = 5
        // Bottom border should be at y=1+4=5
        assert_eq!(fb.get(px, 5).unwrap().char, '\u{2570}'); // bottom-left

        // Selected item should have indicator
        let content_x = px + 2;
        assert_eq!(fb.get(content_x, 3).unwrap().char, '\u{25B8}'); // selected indicator
    }

    #[test]
    fn test_render_narrow_terminal() {
        let mut ext = CmdlineExtension::new();
        ext.apply_notification(r#"{"active":true,"prompt":":","input":"test","cursor":4}"#);

        // Narrow terminal (32 wide)
        let mut fb = FrameBuffer::new(32, 24);
        ext.render(&mut fb);

        // Should still render without panic
        let pw = popup_width(32);
        let px = popup_x(32, pw);
        assert_eq!(fb.get(px, 1).unwrap().char, '\u{256D}');
    }

    #[test]
    fn test_cursor_position_when_active() {
        let mut ext = CmdlineExtension::new();
        ext.apply_notification(r#"{"active":true,"prompt":":","input":"wq","cursor":2}"#);

        let pos = ext.cursor_position(80, 24);
        assert!(pos.is_some());
        let (x, y) = pos.unwrap();
        // y should be 2 (popup at y=1, content at y=2)
        assert_eq!(y, 2);
        // x = popup_x + 2 (border) + 1 (prompt ":") + 2 (cursor pos)
        let pw = popup_width(80);
        let px = popup_x(80, pw);
        assert_eq!(x, px + 2 + 1 + 2);
    }

    #[test]
    fn test_cursor_position_when_inactive() {
        let ext = CmdlineExtension::new();
        assert!(ext.cursor_position(80, 24).is_none());
    }

    #[test]
    fn test_render_overflow_narrow_popup() {
        let mut ext = CmdlineExtension::new();
        // Long input + completions that exceed popup content width
        ext.apply_notification(
            r#"{"active":true,"prompt":":","input":"this_is_a_very_long_command_that_overflows","cursor":42,"completions":["this_is_a_very_long_completion_candidate"],"completion_index":0}"#,
        );

        // Very narrow terminal — content overflows trigger break branches
        let mut fb = FrameBuffer::new(16, 24);
        ext.render(&mut fb);
        // Should render without panic — overflow branches hit
    }

    #[test]
    fn test_trait_object() {
        let ext: Box<dyn TuiExtension> = Box::new(CmdlineExtension::new());
        assert_eq!(ext.kind(), "cmdline");
        assert!(!ext.is_active());
    }
}
