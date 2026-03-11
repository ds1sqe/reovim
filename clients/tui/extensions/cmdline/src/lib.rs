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
        reovim_extension_kinds::CMDLINE
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
#[path = "lib_tests.rs"]
mod tests;
