//! Render backend abstraction for TUI rendering.
//!
//! This module provides a trait that abstracts over different render targets
//! (`Screen` for interactive TUI, `FrameBuffer` for headless TUI), enabling
//! a single rendering implementation to work with both.
//!
//! # Design
//!
//! The `RenderBackend` trait provides a minimal interface for cell-based
//! rendering. Both `Screen` (terminal output) and `FrameBuffer` (in-memory)
//! implement this trait, allowing the render engine to be agnostic about
//! the actual output target.
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  render_engine.rs                                           │
//! │    render_frame<B: RenderBackend>(backend, state, config)   │
//! ├─────────────────────────────────────────────────────────────┤
//! │  RenderBackend trait                                        │
//! │    set_cell, apply_style, write_str, size, clear            │
//! ├──────────────────────────┬──────────────────────────────────┤
//! │  ScreenBackend           │  FrameBufferBackend              │
//! │  (interactive TUI)       │  (headless TUI)                  │
//! └──────────────────────────┴──────────────────────────────────┘
//! ```

use {reovim_arch::Color, reovim_driver_display::Style};

/// Trait for render backends that can display cell-based content.
///
/// This trait abstracts over `Screen` (terminal) and `FrameBuffer` (memory),
/// allowing unified rendering code to work with both interactive and headless TUIs.
// TODO(#494): Window separator drawing (│, ─, ┼) for multi-window layout
pub trait RenderBackend {
    /// Write a character at (x, y) with the given style.
    ///
    /// Coordinates are 0-indexed. Out-of-bounds writes are silently ignored.
    fn set_cell(&mut self, x: u16, y: u16, ch: char, style: &Style);

    /// Apply style to an existing cell without changing its character.
    ///
    /// Used for overlays like cursor highlighting where we want to preserve
    /// the underlying character but change its appearance.
    fn apply_style(&mut self, x: u16, y: u16, style: &Style);

    /// Write a string starting at (x, y) with the given style.
    ///
    /// Returns the number of columns used (accounting for wide characters).
    /// Does not wrap to the next line.
    fn write_str(&mut self, x: u16, y: u16, text: &str, style: &Style) -> u16;

    /// Get the dimensions of the render target.
    fn size(&self) -> (u16, u16);

    /// Clear the entire render target.
    fn clear(&mut self);

    /// Overlay a background color on an existing cell.
    ///
    /// Preserves the character and foreground color, only changing background.
    /// Used for selection highlighting.
    fn overlay_bg(&mut self, x: u16, y: u16, bg: Color);

    /// Fill a horizontal line with a character.
    fn fill_horizontal(&mut self, x: u16, y: u16, width: u16, ch: char, style: &Style) {
        for col in x..x.saturating_add(width) {
            self.set_cell(col, y, ch, style);
        }
    }

    /// Fill a vertical line with a character.
    fn fill_vertical(&mut self, x: u16, y: u16, height: u16, ch: char, style: &Style) {
        for row in y..y.saturating_add(height) {
            self.set_cell(x, row, ch, style);
        }
    }

    /// Fill a rectangular region with a character.
    fn fill_region(&mut self, x: u16, y: u16, width: u16, height: u16, ch: char, style: &Style) {
        let (w, h) = self.size();
        for row in y..y.saturating_add(height).min(h) {
            for col in x..x.saturating_add(width).min(w) {
                self.set_cell(col, row, ch, style);
            }
        }
    }
}

// ============================================================================
// Screen Backend (Interactive TUI)
// ============================================================================

use reovim_driver_tui::{Attributes as TuiAttrs, Screen, Style as TuiStyle};

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

impl RenderBackend for Screen {
    fn set_cell(&mut self, x: u16, y: u16, ch: char, style: &Style) {
        let tui_style = to_tui_style(style);
        self.put_char(x, y, ch, &tui_style);
    }

    #[allow(clippy::use_self)] // Screen::apply_style is inherent method, Self:: would call trait method
    fn apply_style(&mut self, x: u16, y: u16, style: &Style) {
        let tui_style = to_tui_style(style);
        Screen::apply_style(self, x, y, &tui_style);
    }

    fn write_str(&mut self, x: u16, y: u16, text: &str, style: &Style) -> u16 {
        let tui_style = to_tui_style(style);
        // Screen's write_str doesn't return column count, so calculate manually
        let mut col = x;
        for ch in text.chars() {
            if col >= self.width() {
                break;
            }
            self.put_char(col, y, ch, &tui_style);
            col += if reovim_driver_tui::char_width(ch) == 2 {
                2
            } else {
                1
            };
        }
        col.saturating_sub(x)
    }

    fn size(&self) -> (u16, u16) {
        (self.width(), self.height())
    }

    #[allow(clippy::use_self)] // Screen::clear is inherent method, Self:: would call trait method
    fn clear(&mut self) {
        Screen::clear(self);
    }

    #[allow(clippy::use_self)] // Screen::overlay_bg is inherent method, Self:: would call trait method
    fn overlay_bg(&mut self, x: u16, y: u16, bg: Color) {
        Screen::overlay_bg(self, x, y, bg);
    }
}

// ============================================================================
// FrameBuffer Backend (Headless TUI)
// ============================================================================

use reovim_driver_display::FrameBuffer;

impl RenderBackend for FrameBuffer {
    fn set_cell(&mut self, x: u16, y: u16, ch: char, style: &Style) {
        self.put_char(x, y, ch, style);
    }

    #[allow(clippy::use_self)] // FrameBuffer::apply_style is inherent method
    fn apply_style(&mut self, x: u16, y: u16, style: &Style) {
        FrameBuffer::apply_style(self, x, y, style);
    }

    #[allow(clippy::use_self)] // FrameBuffer::write_str is inherent method
    fn write_str(&mut self, x: u16, y: u16, text: &str, style: &Style) -> u16 {
        FrameBuffer::write_str(self, x, y, text, style)
    }

    fn size(&self) -> (u16, u16) {
        (self.width(), self.height())
    }

    #[allow(clippy::use_self)] // FrameBuffer::clear is inherent method
    fn clear(&mut self) {
        FrameBuffer::clear(self);
    }

    fn overlay_bg(&mut self, x: u16, y: u16, bg: Color) {
        // FrameBuffer doesn't have overlay_bg, implement manually
        if let Some(cell) = self.get(x, y).cloned() {
            let mut new_style = cell.style.clone();
            new_style.bg = Some(bg);
            let new_cell = reovim_driver_display::Cell::new(cell.char, new_style);
            self.set(x, y, new_cell);
        }
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
mod tests {
    use super::*;

    #[test]
    fn test_screen_backend() {
        let mut screen = Screen::new(80, 24);
        let style = Style::default();

        // Test set_cell
        screen.set_cell(0, 0, 'H', &style);
        screen.set_cell(1, 0, 'i', &style);

        // Test size
        assert_eq!(screen.size(), (80, 24));
    }

    #[test]
    fn test_framebuffer_backend() {
        let mut fb = FrameBuffer::new(80, 24);
        let style = Style::default();

        // Test set_cell
        fb.set_cell(0, 0, 'H', &style);
        fb.set_cell(1, 0, 'i', &style);

        // Verify
        assert_eq!(fb.get(0, 0).map(|c| c.char), Some('H'));
        assert_eq!(fb.get(1, 0).map(|c| c.char), Some('i'));

        // Test size
        assert_eq!(fb.size(), (80, 24));
    }

    #[test]
    fn test_fill_region() {
        let mut fb = FrameBuffer::new(10, 10);
        let style = Style::default();

        fb.fill_region(2, 2, 3, 3, '#', &style);

        // Check corners of filled region
        assert_eq!(fb.get(2, 2).map(|c| c.char), Some('#'));
        assert_eq!(fb.get(4, 4).map(|c| c.char), Some('#'));

        // Check outside region
        assert_eq!(fb.get(1, 1).map(|c| c.char), Some(' '));
        assert_eq!(fb.get(5, 5).map(|c| c.char), Some(' '));
    }

    #[test]
    fn test_write_str() {
        let mut fb = FrameBuffer::new(80, 24);
        let style = Style::default();

        let written = fb.write_str(0, 0, "Hello", &style);
        assert_eq!(written, 5);
        assert_eq!(fb.get(0, 0).map(|c| c.char), Some('H'));
        assert_eq!(fb.get(4, 0).map(|c| c.char), Some('o'));
    }

    #[test]
    fn test_format_frame_buffer_plain() {
        let fb = FrameBuffer::new(5, 1);
        let plain = format_frame_buffer(&fb, "plain_text");
        assert_eq!(plain, "     ");
    }

    #[test]
    fn test_format_frame_buffer_ansi_empty() {
        let fb = FrameBuffer::new(3, 1);
        let ansi = format_frame_buffer(&fb, "ansi");
        // Empty cells with default style — may include ANSI codes
        // but must contain the 3 space characters
        assert!(ansi.contains("   ") || ansi.matches(' ').count() >= 3);
    }
}
