//! Startup landing screen TUI extension.
//!
//! Displays a centered overlay with ASCII art, version info, and quick-action
//! hints when the editor starts with no file argument. Dismissed on any user
//! interaction (cursor move, mode change, or buffer update).
//!
//! This crate is a self-contained TUI extension: it owns its state and renders
//! through `RenderBackend`. The engine has ZERO knowledge of this crate.

use {
    reovim_arch::Color,
    reovim_driver_display::{
        Style,
        popup_utils::render_box_border,
        render_backend::{RenderBackend, TuiExtension},
    },
};

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

/// Startup landing screen extension.
///
/// Starts active and dismisses permanently on the first user interaction.
pub struct LandingExtension {
    dismissed: bool,
}

impl LandingExtension {
    /// Create a new landing extension (starts active).
    #[must_use]
    pub const fn new() -> Self {
        Self { dismissed: false }
    }
}

impl Default for LandingExtension {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiExtension for LandingExtension {
    fn kind(&self) -> &'static str {
        "landing"
    }

    fn is_active(&self) -> bool {
        !self.dismissed
    }

    /// No-op: the landing screen is purely client-driven with no server-side
    /// module producing notifications. This method exists only to satisfy the
    /// trait contract.
    fn apply_notification(&mut self, _data: &str) {}

    fn render(&self, backend: &mut dyn RenderBackend) {
        if self.dismissed {
            return;
        }
        render_landing(backend);
    }

    fn tick(&mut self) -> bool {
        false
    }

    fn on_cursor_update(&mut self, _buffer_id: u64, _line: usize, _col: usize) {
        self.dismissed = true;
    }

    fn on_mode_change(&mut self, _mode_name: &str, _is_insert: bool) {
        self.dismissed = true;
    }

    fn on_buffer_update(&mut self, _buffer_id: u64, _lines: &[String]) {
        self.dismissed = true;
    }
}

/// Render the landing screen overlay.
#[allow(clippy::cast_possible_truncation)] // All string constants are <50 chars.
fn render_landing(backend: &mut dyn RenderBackend) {
    let (width, height) = backend.size();

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

    let border_style = Style::default().fg(Color::Cyan);
    let bg_style = Style::default().bg(Color::Rgb {
        r: 30,
        g: 30,
        b: 46,
    });

    // Fill background inside box.
    for row in 0..box_height {
        for col in 0..BOX_WIDTH {
            backend.set_cell(box_x + col, box_y + row, ' ', &bg_style);
        }
    }

    // Draw border.
    render_box_border(backend, box_x, box_y, BOX_WIDTH, box_height, &border_style);

    // Content starts inside the box (1 border + 1 padding).
    let content_x = box_x + 2;
    let inner_width = BOX_WIDTH.saturating_sub(4);
    let mut y = box_y + 2;

    // Logo (centered within inner width).
    let logo_style = Style::default().fg(Color::Cyan).bg(Color::Rgb {
        r: 30,
        g: 30,
        b: 46,
    });
    for line in LOGO {
        let line_len = line.len() as u16;
        let logo_x = content_x + inner_width.saturating_sub(line_len) / 2;
        backend.write_str(logo_x, y, line, &logo_style);
        y += 1;
    }

    // Version string (centered).
    let version_text = format!("v{VERSION}");
    let version_len = version_text.len() as u16;
    let version_style = Style::default().fg(Color::White).bg(Color::Rgb {
        r: 30,
        g: 30,
        b: 46,
    });
    let version_x = content_x + inner_width.saturating_sub(version_len) / 2;
    backend.write_str(version_x, y, &version_text, &version_style);
    y += 1;

    // Blank line.
    y += 1;

    // Action hints.
    let action_style = Style::default().fg(Color::White).bg(Color::Rgb {
        r: 30,
        g: 30,
        b: 46,
    });
    let key_style = Style::default().fg(Color::Yellow).bg(Color::Rgb {
        r: 30,
        g: 30,
        b: 46,
    });
    for line in ACTIONS {
        // Highlight the key portion (up to first space after leading spaces).
        let trimmed = line.trim_start();
        let leading = line.len() - trimmed.len();
        let key_end = trimmed.find(' ').map_or(trimmed.len(), |i| i + leading);

        backend.write_str(content_x, y, &line[..key_end], &key_style);
        backend.write_str(content_x + key_end as u16, y, &line[key_end..], &action_style);
        y += 1;
    }

    // Blank line.
    y += 1;

    // Footer (centered, dimmed).
    let footer_len = FOOTER.len() as u16;
    let footer_style = Style::default().fg(Color::DarkGrey).bg(Color::Rgb {
        r: 30,
        g: 30,
        b: 46,
    });
    let footer_x = content_x + inner_width.saturating_sub(footer_len) / 2;
    backend.write_str(footer_x, y, FOOTER, &footer_style);
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
