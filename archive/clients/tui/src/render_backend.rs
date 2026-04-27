//! Render backend abstraction for TUI rendering.
//!
//! This module provides `ScreenBackend` (a newtype around `Screen`) that
//! implements `RenderBackend` for interactive TUI rendering. It also provides
//! frame buffer capture utilities.
//!
//! The `RenderBackend` trait itself and `impl RenderBackend for FrameBuffer`
//! live in `reovim-driver-display`.
//!
//! # Design
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  render_engine.rs                                           │
//! │    render_frame<B: RenderBackend>(backend, state, config)   │
//! ├─────────────────────────────────────────────────────────────┤
//! │  RenderBackend trait  (reovim-driver-display)               │
//! │    set_cell, apply_style, write_str, size, clear            │
//! ├──────────────────────────┬──────────────────────────────────┤
//! │  ScreenBackend           │  FrameBufferBackend              │
//! │  (this module)           │  (reovim-driver-display)         │
//! └──────────────────────────┴──────────────────────────────────┘
//! ```

// Re-export the trait from display crate
pub use reovim_driver_display::render_backend::RenderBackend;

use reovim_driver_display::{FrameBuffer, Style};

// ============================================================================
// Screen Backend (Interactive TUI)
// ============================================================================

use reovim_driver_tui::{Attributes as TuiAttrs, Screen, Style as TuiStyle};

/// Newtype wrapper around `Screen` that implements `RenderBackend`.
///
/// This wrapper exists to satisfy Rust's orphan rule: `RenderBackend` is
/// defined in `reovim-driver-display` and `Screen` in `reovim-driver-tui`,
/// so neither can be implemented in this crate without a local type.
pub struct ScreenBackend(pub Screen);

impl ScreenBackend {
    /// Create a new screen backend with the given dimensions.
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        Self(Screen::new(width, height))
    }

    /// Get the width of the screen.
    #[must_use]
    pub const fn width(&self) -> u16 {
        self.0.width()
    }

    /// Get the height of the screen.
    #[must_use]
    pub const fn height(&self) -> u16 {
        self.0.height()
    }

    /// Resize the screen.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.0.resize(width, height);
    }

    /// Invalidate the screen for full redraw.
    pub fn invalidate(&mut self) {
        self.0.invalidate();
    }

    /// Render the screen to the terminal.
    ///
    /// # Errors
    ///
    /// Returns an error if terminal rendering fails.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn render(&mut self, terminal: &mut reovim_driver_tui::Terminal) -> std::io::Result<()> {
        self.0.render(terminal)
    }
}

/// Convert display driver Style to TUI driver Style.
///
/// The display driver's Style is more feature-complete (includes `underline_color`),
/// while the TUI driver's Style is simpler. This function converts between them.
fn to_tui_style(style: &Style) -> TuiStyle {
    use reovim_driver_display::Attributes as DisplayAttrs;

    let mut tui_style = TuiStyle::new();
    tui_style.fg = style.fg;
    tui_style.bg = style.bg;

    // Convert attributes
    let mut attrs = TuiAttrs::new();
    if style.attributes.contains(DisplayAttrs::BOLD) {
        attrs.set(TuiAttrs::BOLD);
    }
    if style.attributes.contains(DisplayAttrs::ITALIC) {
        attrs.set(TuiAttrs::ITALIC);
    }
    if style.attributes.contains(DisplayAttrs::UNDERLINE) {
        attrs.set(TuiAttrs::UNDERLINE);
    }
    if style.attributes.contains(DisplayAttrs::STRIKETHROUGH) {
        attrs.set(TuiAttrs::STRIKETHROUGH);
    }
    if style.attributes.contains(DisplayAttrs::REVERSE) {
        attrs.set(TuiAttrs::REVERSE);
    }
    if style.attributes.contains(DisplayAttrs::DIM) {
        attrs.set(TuiAttrs::DIM);
    }
    tui_style.attrs = attrs;

    tui_style
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl RenderBackend for ScreenBackend {
    fn set_cell(&mut self, x: u16, y: u16, ch: char, style: &Style) {
        let tui_style = to_tui_style(style);
        self.0.put_char(x, y, ch, &tui_style);
    }

    fn apply_style(&mut self, x: u16, y: u16, style: &Style) {
        let tui_style = to_tui_style(style);
        self.0.apply_style(x, y, &tui_style);
    }

    fn write_str(&mut self, x: u16, y: u16, text: &str, style: &Style) -> u16 {
        let tui_style = to_tui_style(style);
        // Screen's write_str doesn't return column count, so calculate manually
        let mut col = x;
        for ch in text.chars() {
            if col >= self.0.width() {
                break;
            }
            self.0.put_char(col, y, ch, &tui_style);
            col += if reovim_driver_tui::char_width(ch) == 2 {
                2
            } else {
                1
            };
        }
        col.saturating_sub(x)
    }

    fn size(&self) -> (u16, u16) {
        (self.0.width(), self.0.height())
    }

    fn clear(&mut self) {
        self.0.clear();
    }

    fn overlay_bg(&mut self, x: u16, y: u16, bg: reovim_arch::Color) {
        self.0.overlay_bg(x, y, bg);
    }
}

// ============================================================================
// Frame Buffer Capture (standalone, works for both modes)
// ============================================================================

/// Format a `FrameBuffer` as a string.
///
/// This is the single capture implementation for both interactive and headless
/// modes. Since `TuiApp` always owns a `FrameBuffer`, capture works universally.
///
/// # Formats
///
/// - `"ansi"` / `"raw_ansi"` — ANSI-colored output with escape sequences
/// - anything else — plain text (no escape sequences)
#[must_use]
pub fn format_frame_buffer(fb: &FrameBuffer, format: &str) -> String {
    match format.to_lowercase().as_str() {
        "ansi" | "raw_ansi" | "rawansi" => frame_to_ansi(fb),
        _ => frame_to_plain_text(fb),
    }
}

/// Convert frame buffer to ANSI-colored string.
#[cfg_attr(coverage_nightly, coverage(off))]
fn frame_to_ansi(fb: &FrameBuffer) -> String {
    use reovim_driver_display::ColorMode;

    let mut output = String::new();
    let height = fb.height();

    for y in 0..height {
        if let Some(row) = fb.row(y) {
            for cell in row {
                // Skip continuation cells (part of wide characters)
                if cell.is_continuation {
                    continue;
                }

                // Apply style
                let ansi_start = cell.style.to_ansi_start(ColorMode::TrueColor);
                if !ansi_start.is_empty() {
                    output.push_str(&ansi_start);
                }

                output.push(cell.char);

                // Reset if style was applied
                if !ansi_start.is_empty() {
                    output.push_str("\x1b[0m");
                }
            }
        }
        if y < height - 1 {
            output.push('\n');
        }
    }

    output
}

/// Convert frame buffer to plain text (no ANSI codes).
#[cfg_attr(coverage_nightly, coverage(off))]
fn frame_to_plain_text(fb: &FrameBuffer) -> String {
    let mut output = String::new();
    let height = fb.height();

    for y in 0..height {
        if let Some(row) = fb.row(y) {
            for cell in row {
                // Skip continuation cells
                if cell.is_continuation {
                    continue;
                }
                output.push(cell.char);
            }
        }
        if y < height - 1 {
            output.push('\n');
        }
    }

    output
}

#[cfg(test)]
#[path = "render_backend_tests.rs"]
mod tests;
