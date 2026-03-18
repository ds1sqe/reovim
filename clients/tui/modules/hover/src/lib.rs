//! Hover popup chrome module.
//!
//! Displays LSP hover information in a bordered popup near the cursor.
//! Native `ClientModule` implementation (no `TuiExtension` bridge).

use {
    reovim_client_driver::{
        BufferId, ChromePosition, ClientModule, ClientModuleError, ModuleContext,
        PlatformCapabilities, ProbeResult, Rect, RenderSurface, Style, Version, types::Color,
    },
    serde::Deserialize,
};

const KIND: &str = "hover";
const MAX_WIDTH_RATIO: f32 = 0.6;
const MIN_WIDTH: u16 = 20;
const MAX_LINES: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ContentType {
    Plaintext,
    Markdown,
}

#[derive(Debug, Clone, Deserialize)]
enum Origin {
    BufferPosition {
        #[allow(dead_code)]
        buffer_id: u64,
        line: u32,
        col: u32,
    },
}

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

/// Hover popup chrome module.
pub struct HoverModule {
    active: bool,
    lines: Vec<String>,
    content_type: ContentType,
    origin_line: u32,
    origin_col: u32,
}

impl HoverModule {
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

        #[allow(clippy::cast_possible_truncation)]
        let desired = (longest_line + 4).min(u16::MAX as usize) as u16;
        desired.clamp(MIN_WIDTH, max_width)
    }
}

impl Default for HoverModule {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientModule for HoverModule {
    fn id(&self) -> &'static str {
        KIND
    }

    fn kind(&self) -> &'static str {
        KIND
    }

    fn name(&self) -> &'static str {
        "Hover"
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
        45
    }

    fn on_notification(&mut self, data: &str) {
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

        if self.lines.is_empty() {
            self.active = false;
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn on_cursor_update(&mut self, _buffer_id: BufferId, line: usize, col: usize) {
        if self.active && (line as u32 != self.origin_line || col as u32 != self.origin_col) {
            self.active = false;
            self.lines.clear();
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn chrome_render(
        &self,
        surface: &mut dyn RenderSurface,
        bounds: Rect,
        _caps: &dyn PlatformCapabilities,
    ) {
        if !self.active || self.lines.is_empty() {
            return;
        }

        let (width, height) = (bounds.width, bounds.height);
        let popup_w = self.popup_width(width);
        let content_lines = self.lines.len().min(MAX_LINES) as u16;
        let popup_h = content_lines + 2;

        let px = self
            .origin_col
            .min(u32::from(width.saturating_sub(popup_w))) as u16;

        let anchor_y = (self.origin_line as u16).min(height.saturating_sub(1));
        let py = if anchor_y.saturating_add(1).saturating_add(popup_h) < height {
            anchor_y + 1
        } else {
            anchor_y.saturating_sub(popup_h)
        };

        let border_color = match self.content_type {
            ContentType::Markdown => Color::Cyan,
            ContentType::Plaintext => Color::Grey,
        };
        let border_style = Style::new().fg(border_color);

        reovim_client_driver::chrome_utils::render_box_border(
            surface,
            px,
            py,
            popup_w,
            popup_h,
            &border_style,
        );

        let content_x = px + 1;
        let content_w = popup_w.saturating_sub(2);
        let text_style = Style::new().fg(Color::White);

        for (i, line) in self.lines.iter().take(content_lines as usize).enumerate() {
            let row = py + 1 + i as u16;

            // Clear interior row
            surface.fill(
                Rect {
                    x: content_x,
                    y: row,
                    width: content_w,
                    height: 1,
                },
                ' ',
                Style::new(),
            );

            let display = reovim_client_driver::ui::truncate_end(line, content_w as usize);
            surface.write_styled(content_x, row, &display, text_style.clone());
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
