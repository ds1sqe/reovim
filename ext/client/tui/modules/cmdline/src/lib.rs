#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Cmdline chrome module.
//!
//! Displays a floating command-line popup for ex-commands (`:`, `/`, `?`).
//! Includes input text, cursor, and completion candidates.
//!
//! Native `ClientModule` implementation (no `TuiExtension` bridge).

use reovim_client_driver::{
    ChromePosition, ClientModule, ClientModuleError, ModuleContext, PlatformCapabilities,
    ProbeResult, Rect, ChromeSurface, Style, Version, types::Color,
};

const KIND: &str = "cmdline";

/// Cmdline popup chrome module.
///
/// Shows a command-line input popup with prompt, cursor, and completions.
pub struct CmdlineModule {
    /// Whether the popup is currently visible.
    pub(crate) active: bool,
    /// Prompt character (":", "/", "?").
    pub(crate) prompt: String,
    /// Input text.
    pub(crate) input: String,
    /// Cursor position (char index into input).
    pub(crate) cursor: usize,
    /// Completion candidates.
    pub(crate) completions: Vec<String>,
    /// Currently selected completion index.
    pub(crate) completion_index: Option<usize>,
}

impl CmdlineModule {
    /// Create a new cmdline module (inactive).
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

impl Default for CmdlineModule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl ClientModule for CmdlineModule {
    fn id(&self) -> &'static str {
        KIND
    }

    fn kind(&self) -> &'static str {
        KIND
    }

    fn name(&self) -> &'static str {
        "Cmdline"
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

    fn has_chrome(&self) -> bool {
        true
    }

    fn chrome_position(&self) -> ChromePosition {
        ChromePosition::Overlay
    }

    fn chrome_priority(&self) -> u16 {
        90
    }

    #[allow(clippy::cast_possible_truncation)]
    fn on_notification(&mut self, data: &str) {
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
    fn cursor_position(&self, w: u16, _h: u16) -> Option<(u16, u16)> {
        if !self.active {
            return None;
        }
        let pw = reovim_client_driver::chrome_utils::popup_width(w);
        let px = reovim_client_driver::chrome_utils::popup_x(w, pw);
        let content_x = px + 2; // border + padding
        let prompt_len = self.prompt.len() as u16;
        let cursor_x = content_x + prompt_len + self.cursor as u16;
        let cursor_y = 2; // popup at y=1, content row at y=2
        Some((cursor_x, cursor_y))
    }

    #[allow(clippy::cast_possible_truncation)]
    fn chrome_render(
        &self,
        surface: &mut dyn ChromeSurface,
        bounds: Rect,
        _caps: &dyn PlatformCapabilities,
    ) {
        if !self.active {
            return;
        }

        let width = bounds.width;

        let pw = reovim_client_driver::chrome_utils::popup_width(width);
        let px = reovim_client_driver::chrome_utils::popup_x(width, pw);
        let py: u16 = 1; // Near top of screen

        // Popup height: 3 (border + content + border) + completion rows
        let completion_rows = self.completions.len() as u16;
        let popup_height = 3 + completion_rows;

        // Draw border
        let border_style = Style::new().fg(Color::DarkGrey);
        reovim_client_driver::chrome_utils::render_box_border(
            surface,
            px,
            py,
            pw,
            popup_height,
            &border_style,
        );

        // Content row (inside the border)
        let content_y = py + 1;
        let content_x = px + 2; // border + padding
        let content_width = pw.saturating_sub(4); // 2 padding each side

        // Clear content area
        let clear_style = Style::new();
        for col in 0..content_width {
            surface.write_styled(content_x + col, content_y, " ", clear_style.clone());
        }

        // Prompt character in yellow
        let prompt_style = Style::new().fg(Color::Yellow);
        surface.write_styled(content_x, content_y, &self.prompt, prompt_style);

        let prompt_len = self.prompt.len() as u16;
        let input_style = Style::new();

        // Render input text
        for (i, ch) in self.input.chars().enumerate() {
            let x = content_x + prompt_len + i as u16;
            if x >= content_x + content_width {
                break;
            }
            surface.write_styled(x, content_y, &ch.to_string(), input_style.clone());
        }

        // Render block cursor
        let cursor_style = Style::new().bg(Color::White).fg(Color::Black);
        let cursor_x = content_x + prompt_len + self.cursor as u16;
        if cursor_x < content_x + content_width {
            if self.cursor < self.input.len() {
                surface.apply_style(cursor_x, content_y, cursor_style);
            } else {
                surface.write_styled(cursor_x, content_y, " ", cursor_style);
            }
        }

        // Render completions below the input line
        for (i, completion) in self.completions.iter().enumerate() {
            let row_y = content_y + 1 + i as u16;
            let is_selected = self.completion_index == Some(i);

            // Clear the row
            for col in 0..content_width {
                surface.write_styled(content_x + col, row_y, " ", clear_style.clone());
            }

            // Indicator: > for selected, space for others
            let indicator = if is_selected { "\u{25B8}" } else { " " };
            let item_style = if is_selected {
                Style::new().bg(Color::DarkGrey).fg(Color::White)
            } else {
                Style::new()
            };

            surface.write_styled(content_x, row_y, indicator, item_style.clone());
            // Render completion text with 2-char offset (indicator + space)
            for (j, ch) in completion.chars().enumerate() {
                let x = content_x + 2 + j as u16;
                if x >= content_x + content_width {
                    break;
                }
                surface.write_styled(x, row_y, &ch.to_string(), item_style.clone());
            }
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
