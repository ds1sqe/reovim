//! Signature help popup chrome module.
//!
//! Displays LSP signature help (function signature) in a single-line
//! bordered popup near the cursor position.
//! Native `ClientModule` implementation (no `TuiExtension` bridge).

use reovim_client_driver::{
    ChromePosition, ClientModule, ClientModuleError, ModuleContext, PlatformCapabilities,
    ProbeResult, Rect, RenderSurface, Style, Version,
};
use reovim_client_driver::types::Color;
use serde::Deserialize;

/// Minimum popup width.
const MIN_WIDTH: u16 = 10;

const KIND: &str = "signature-help";

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

/// Signature help popup chrome module.
pub struct SignatureHelpModule {
    /// Whether the popup is visible.
    active: bool,
    /// Signature label to display.
    label: String,
    /// Origin line in the buffer (0-indexed).
    origin_line: u32,
    /// Origin column in the buffer (0-indexed).
    origin_col: u32,
}

impl SignatureHelpModule {
    /// Create a new signature help module (inactive).
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

impl Default for SignatureHelpModule {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientModule for SignatureHelpModule {
    fn id(&self) -> &'static str {
        KIND
    }

    fn kind(&self) -> &'static str {
        KIND
    }

    fn name(&self) -> &'static str {
        "SignatureHelp"
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
        44
    }

    fn on_notification(&mut self, data: &str) {
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
    fn chrome_render(
        &self,
        surface: &mut dyn RenderSurface,
        bounds: Rect,
        _caps: &dyn PlatformCapabilities,
    ) {
        if !self.active || self.label.is_empty() {
            return;
        }

        let (width, height) = (bounds.width, bounds.height);

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

        let border_style = Style::new().fg(Color::Yellow);

        reovim_client_driver::chrome_utils::render_box_border(
            surface, px, py, popup_w, popup_h, &border_style,
        );

        // Clear interior and draw label.
        let content_x = px + 1;
        let content_w = popup_w.saturating_sub(2);

        surface.fill(
            Rect { x: content_x, y: py + 1, width: content_w, height: 1 },
            ' ',
            Style::new(),
        );

        let text_style = Style::new().fg(Color::White);
        let display = reovim_client_driver::ui::truncate_end(&self.label, content_w as usize);
        surface.write_styled(content_x, py + 1, &display, text_style);
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
