//! Startup landing screen chrome module.
//!
//! Displays a centered overlay with ASCII art, version info, and quick-action
//! hints when the editor starts with no file argument. Dismissed on any user
//! interaction (cursor move, mode change, or buffer update).
//! Native `ClientModule` implementation (no `TuiExtension` bridge).

use reovim_client_driver::{
    BufferId, BufferUpdateEvent, ChromePosition, ClientModule, ClientModuleError, ModuleContext,
    PlatformCapabilities, ProbeResult, Rect, RenderSurface, Style, Version,
};
use reovim_client_driver::types::Color;

/// ASCII art logo lines (compact, ~40 chars wide).
const LOGO: &[&str] = &[
    r"  ____                  _            ",
    r" |  _ \ ___  _____   _(_)_ __ ___   ",
    r" | |_) / _ \/ _ \ \ / / | '_ ` _ \  ",
    r" |  _ <  __/ (_) \ V /| | | | | | | ",
    r" |_| \_\___|\___/ \_/ |_|_| |_| |_| ",
];

/// Version string derived at compile time.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Quick-action hint lines.
const ACTIONS: &[&str] = &[
    "  e         New file",
    "  :e <file> Open file",
    "  :q        Quit",
    "  ?         Help",
];

/// Footer text.
const FOOTER: &str = "Press any key to start";

/// Minimum terminal width to render the landing screen.
const MIN_WIDTH: u16 = 44;

/// Minimum terminal height to render the landing screen.
const MIN_HEIGHT: u16 = 16;

/// Box width (accommodates logo + padding).
const BOX_WIDTH: u16 = 42;

const KIND: &str = "landing";

/// Startup landing screen module.
///
/// Starts active and dismisses permanently on the first user interaction.
pub struct LandingModule {
    dismissed: bool,
}

impl LandingModule {
    /// Create a new landing module (starts active).
    #[must_use]
    pub const fn new() -> Self {
        Self { dismissed: false }
    }
}

impl Default for LandingModule {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientModule for LandingModule {
    fn id(&self) -> &'static str {
        KIND
    }

    fn kind(&self) -> &'static str {
        KIND
    }

    fn name(&self) -> &'static str {
        "Landing"
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
        ChromePosition::Overlay
    }

    fn chrome_priority(&self) -> u16 {
        30
    }

    fn on_cursor_update(&mut self, _buffer_id: BufferId, _line: usize, _col: usize) {
        self.dismissed = true;
    }

    fn on_mode_change(&mut self, _mode: &str) {
        self.dismissed = true;
    }

    fn on_buffer_update(&mut self, _event: &BufferUpdateEvent) {
        self.dismissed = true;
    }

    #[allow(clippy::cast_possible_truncation)]
    fn chrome_render(
        &self,
        surface: &mut dyn RenderSurface,
        bounds: Rect,
        _caps: &dyn PlatformCapabilities,
    ) {
        if self.dismissed {
            return;
        }

        let (width, height) = (bounds.width, bounds.height);

        // Graceful degradation: skip if terminal too small.
        if width < MIN_WIDTH || height < MIN_HEIGHT {
            return;
        }

        // Compute layout.
        //   logo:    5 lines
        //   version: 1 line
        //   blank:   1 line
        //   actions: 4 lines
        //   blank:   1 line
        //   footer:  1 line
        //   borders: 2 lines (top + bottom)
        //   padding: 1 line top + 1 line bottom
        let content_height: u16 = 5 + 1 + 1 + 4 + 1 + 1; // 13
        let box_height = content_height + 2 + 2; // 17 (borders + padding)

        let box_x = width.saturating_sub(BOX_WIDTH) / 2;
        let box_y = height.saturating_sub(box_height) / 2;

        let border_style = Style::new().fg(Color::Cyan);
        let bg = Color::Rgb { r: 30, g: 30, b: 46 };
        let bg_style = Style::new().bg(bg);

        // Fill background inside box.
        surface.fill(
            Rect { x: box_x, y: box_y, width: BOX_WIDTH, height: box_height },
            ' ',
            bg_style,
        );

        // Draw border.
        reovim_client_driver::chrome_utils::render_box_border(
            surface, box_x, box_y, BOX_WIDTH, box_height, &border_style,
        );

        // Content starts inside the box (1 border + 1 padding).
        let content_x = box_x + 2;
        let inner_width = BOX_WIDTH.saturating_sub(4);
        let mut y = box_y + 2;

        // Logo (centered within inner width).
        let logo_style = Style::new().fg(Color::Cyan).bg(bg);
        for line in LOGO {
            let line_len = line.len() as u16;
            let logo_x = content_x + inner_width.saturating_sub(line_len) / 2;
            surface.write_styled(logo_x, y, line, logo_style.clone());
            y += 1;
        }

        // Version string (centered).
        let version_text = format!("v{VERSION}");
        let version_len = version_text.len() as u16;
        let version_style = Style::new().fg(Color::White).bg(bg);
        let version_x = content_x + inner_width.saturating_sub(version_len) / 2;
        surface.write_styled(version_x, y, &version_text, version_style);
        y += 1;

        // Blank line.
        y += 1;

        // Action hints.
        let action_style = Style::new().fg(Color::White).bg(bg);
        let key_style = Style::new().fg(Color::Yellow).bg(bg);
        for line in ACTIONS {
            // Highlight the key portion (up to first space after leading spaces).
            let trimmed = line.trim_start();
            let leading = line.len() - trimmed.len();
            let key_end = trimmed.find(' ').map_or(trimmed.len(), |i| i + leading);

            surface.write_styled(content_x, y, &line[..key_end], key_style.clone());
            #[allow(clippy::cast_possible_truncation)]
            let key_end_u16 = key_end as u16;
            surface.write_styled(content_x + key_end_u16, y, &line[key_end..], action_style.clone());
            y += 1;
        }

        // Blank line.
        y += 1;

        // Footer (centered, dimmed).
        let footer_len = FOOTER.len() as u16;
        let footer_style = Style::new().fg(Color::DarkGrey).bg(bg);
        let footer_x = content_x + inner_width.saturating_sub(footer_len) / 2;
        surface.write_styled(footer_x, y, FOOTER, footer_style);
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
