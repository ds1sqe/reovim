//! Statusline chrome module.
//!
//! Renders the mode indicator and cursor position at the bottom of the screen.
//! Extracted from the hardcoded `render_statusline()` in `render_engine.rs`.

use reovim_client_driver::{
    BufferId, ChromePosition, ClientModule, ClientModuleError, ModuleContext, PlatformCapabilities,
    ProbeResult, Rect, RenderSurface, Style, Version, types::Color,
};

/// Statusline chrome module.
///
/// Shows the current mode (colored badge) on the left and cursor position
/// (line:col) on the right.
pub struct StatuslineModule {
    mode: String,
    cursor_line: usize,
    cursor_col: usize,
    has_cursor: bool,
}

impl StatuslineModule {
    #[must_use]
    pub fn new() -> Self {
        Self {
            mode: String::from("NORMAL"),
            cursor_line: 0,
            cursor_col: 0,
            has_cursor: false,
        }
    }
}

impl Default for StatuslineModule {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientModule for StatuslineModule {
    fn id(&self) -> &'static str {
        "statusline"
    }

    fn kind(&self) -> &'static str {
        "statusline"
    }

    fn name(&self) -> &'static str {
        "Statusline"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn server_kinds(&self) -> Vec<&'static str> {
        vec![]
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
        ChromePosition::Bottom
    }

    fn chrome_requested_size(&self, _caps: &dyn PlatformCapabilities) -> u16 {
        1
    }

    fn chrome_priority(&self) -> u16 {
        100
    }

    fn on_mode_change(&mut self, mode: &str) {
        self.mode = mode.to_uppercase();
    }

    fn on_cursor_update(&mut self, _buffer_id: BufferId, line: usize, col: usize) {
        self.cursor_line = line;
        self.cursor_col = col;
        self.has_cursor = true;
    }

    #[allow(clippy::cast_possible_truncation)]
    fn chrome_render(
        &self,
        surface: &mut dyn RenderSurface,
        bounds: Rect,
        _caps: &dyn PlatformCapabilities,
    ) {
        // Mode indicator (left)
        let mode_style = mode_style(&self.mode);
        let mode_str = format!(" {} ", self.mode);
        surface.write_styled(bounds.x, bounds.y, &mode_str, mode_style);

        // Cursor position (right)
        let pos_str = if self.has_cursor {
            format!("{}:{}", self.cursor_line + 1, self.cursor_col + 1)
        } else {
            "?:?".to_string()
        };
        let pos_x = bounds.x + bounds.width.saturating_sub(pos_str.len() as u16 + 1);
        surface.write_styled(pos_x, bounds.y, &pos_str, Style::new());
    }
}

/// Get the style for a mode indicator.
fn mode_style(mode: &str) -> Style {
    let mode_lower = mode.to_lowercase();
    let (fg, bg) = if mode_lower.contains("insert") {
        (Color::Black, Color::Green)
    } else if mode_lower.contains("visual") {
        (Color::Black, Color::Magenta)
    } else if mode_lower.contains("command") || mode_lower.contains("cmdline") {
        (Color::Black, Color::Yellow)
    } else if mode_lower.contains("replace") {
        (Color::Black, Color::Red)
    } else {
        // Normal mode
        (Color::Black, Color::Blue)
    };
    Style::new().fg(fg).bg(bg)
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
