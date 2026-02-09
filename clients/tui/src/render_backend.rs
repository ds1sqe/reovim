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

#[cfg_attr(coverage_nightly, coverage(off))]
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
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_format_frame_buffer_ansi_empty() {
        let fb = FrameBuffer::new(3, 1);
        let ansi = format_frame_buffer(&fb, "ansi");
        // Empty cells with default style — may include ANSI codes
        // but must contain the 3 space characters
        assert!(ansi.contains("   ") || ansi.matches(' ').count() >= 3);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_format_frame_buffer_rawansi_alias() {
        let fb = FrameBuffer::new(3, 1);
        let raw = format_frame_buffer(&fb, "raw_ansi");
        let raw2 = format_frame_buffer(&fb, "rawansi");
        // Both raw_ansi aliases should produce the same output
        assert_eq!(raw, raw2);
    }

    #[test]
    fn test_format_frame_buffer_unknown_format_is_plain() {
        let mut fb = FrameBuffer::new(5, 1);
        let style = Style::default();
        fb.set_cell(0, 0, 'X', &style);

        let plain = format_frame_buffer(&fb, "unknown_format");
        // Unknown format falls back to plain text
        assert!(plain.contains('X'));
        // Should NOT contain ANSI escapes
        assert!(!plain.contains("\x1b["));
    }

    #[test]
    fn test_format_frame_buffer_case_insensitive() {
        let fb = FrameBuffer::new(3, 1);
        let upper = format_frame_buffer(&fb, "ANSI");
        let lower = format_frame_buffer(&fb, "ansi");
        assert_eq!(upper, lower);
    }

    #[test]
    fn test_framebuffer_clear() {
        let mut fb = FrameBuffer::new(5, 5);
        let style = Style::default();
        fb.set_cell(2, 2, 'X', &style);
        assert_eq!(fb.get(2, 2).map(|c| c.char), Some('X'));

        RenderBackend::clear(&mut fb);
        assert_eq!(fb.get(2, 2).map(|c| c.char), Some(' '));
    }

    #[test]
    fn test_framebuffer_apply_style() {
        let mut fb = FrameBuffer::new(10, 5);
        let style = Style::default();
        fb.set_cell(3, 2, 'A', &style);

        let new_style = Style::default().fg(reovim_arch::Color::Red);
        RenderBackend::apply_style(&mut fb, 3, 2, &new_style);

        let cell = fb.get(3, 2).unwrap();
        assert_eq!(cell.char, 'A'); // Character preserved
    }

    #[test]
    fn test_framebuffer_overlay_bg() {
        let mut fb = FrameBuffer::new(10, 5);
        let style = Style::default();
        fb.set_cell(1, 1, 'B', &style);

        RenderBackend::overlay_bg(&mut fb, 1, 1, reovim_arch::Color::Blue);
        let cell = fb.get(1, 1).unwrap();
        assert_eq!(cell.char, 'B'); // Character preserved
        assert_eq!(cell.style.bg, Some(reovim_arch::Color::Blue));
    }

    #[test]
    fn test_framebuffer_overlay_bg_out_of_bounds() {
        let mut fb = FrameBuffer::new(5, 5);
        // Should not panic on out-of-bounds
        RenderBackend::overlay_bg(&mut fb, 100, 100, reovim_arch::Color::Red);
    }

    #[test]
    fn test_fill_horizontal() {
        let mut fb = FrameBuffer::new(20, 5);
        let style = Style::default();
        fb.fill_horizontal(5, 2, 10, '-', &style);

        for col in 5..15 {
            assert_eq!(fb.get(col, 2).map(|c| c.char), Some('-'));
        }
        // Before and after should be space
        assert_eq!(fb.get(4, 2).map(|c| c.char), Some(' '));
        assert_eq!(fb.get(15, 2).map(|c| c.char), Some(' '));
    }

    #[test]
    fn test_fill_vertical() {
        let mut fb = FrameBuffer::new(10, 10);
        let style = Style::default();
        fb.fill_vertical(3, 1, 5, '|', &style);

        for row in 1..6 {
            assert_eq!(fb.get(3, row).map(|c| c.char), Some('|'));
        }
        // Before and after should be space
        assert_eq!(fb.get(3, 0).map(|c| c.char), Some(' '));
        assert_eq!(fb.get(3, 6).map(|c| c.char), Some(' '));
    }

    #[test]
    fn test_fill_region_clamped_to_bounds() {
        let mut fb = FrameBuffer::new(5, 5);
        let style = Style::default();
        // Fill region that extends beyond bounds
        fb.fill_region(3, 3, 10, 10, '#', &style);

        // Should fill 3..5 x 3..5 (clamped to bounds)
        assert_eq!(fb.get(3, 3).map(|c| c.char), Some('#'));
        assert_eq!(fb.get(4, 4).map(|c| c.char), Some('#'));
        // Outside bounds should be untouched
        assert_eq!(fb.get(2, 2).map(|c| c.char), Some(' '));
    }

    #[test]
    fn test_screen_backend_write_str() {
        let mut screen = Screen::new(80, 24);
        let style = Style::default();

        let written = RenderBackend::write_str(&mut screen, 0, 0, "Hello", &style);
        assert_eq!(written, 5);
    }

    #[test]
    fn test_screen_backend_clear() {
        let mut screen = Screen::new(10, 10);
        let style = Style::default();
        screen.set_cell(0, 0, 'X', &style);

        RenderBackend::clear(&mut screen);
        // After clear, size should be preserved
        assert_eq!(screen.size(), (10, 10));
    }

    #[test]
    fn test_to_tui_style_attributes() {
        use reovim_driver_display::Attributes as DisplayAttrs;

        let mut style = Style::default();
        style.attributes.set(DisplayAttrs::BOLD);
        style.attributes.set(DisplayAttrs::ITALIC);
        style.attributes.set(DisplayAttrs::UNDERLINE);

        let tui_style = to_tui_style(&style);
        assert!(tui_style.attrs.contains(TuiAttrs::BOLD));
        assert!(tui_style.attrs.contains(TuiAttrs::ITALIC));
        assert!(tui_style.attrs.contains(TuiAttrs::UNDERLINE));
    }

    #[test]
    fn test_to_tui_style_more_attributes() {
        use reovim_driver_display::Attributes as DisplayAttrs;

        let mut style = Style::default();
        style.attributes.set(DisplayAttrs::STRIKETHROUGH);
        style.attributes.set(DisplayAttrs::REVERSE);
        style.attributes.set(DisplayAttrs::DIM);

        let tui_style = to_tui_style(&style);
        assert!(tui_style.attrs.contains(TuiAttrs::STRIKETHROUGH));
        assert!(tui_style.attrs.contains(TuiAttrs::REVERSE));
        assert!(tui_style.attrs.contains(TuiAttrs::DIM));
    }

    #[test]
    fn test_to_tui_style_colors() {
        let mut style = Style::default();
        style.fg = Some(reovim_arch::Color::Red);
        style.bg = Some(reovim_arch::Color::Blue);

        let tui_style = to_tui_style(&style);
        assert_eq!(tui_style.fg, Some(reovim_arch::Color::Red));
        assert_eq!(tui_style.bg, Some(reovim_arch::Color::Blue));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_format_frame_buffer_multiline() {
        let mut fb = FrameBuffer::new(5, 3);
        let style = Style::default();
        for row in 0..3 {
            for col in 0..5 {
                fb.set_cell(col, row, 'a', &style);
            }
        }

        let plain = format_frame_buffer(&fb, "plain_text");
        let lines: Vec<&str> = plain.lines().collect();
        assert_eq!(lines.len(), 3);
        for line in &lines {
            assert!(line.contains("aaaaa"));
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_format_frame_buffer_ansi_with_styled_content() {
        let mut fb = FrameBuffer::new(3, 1);
        let style = Style::default().fg(reovim_arch::Color::Green);
        fb.set_cell(0, 0, 'G', &style);
        fb.set_cell(1, 0, 'o', &style);
        fb.set_cell(2, 0, '!', &style);

        let ansi = format_frame_buffer(&fb, "ansi");
        // Should contain ANSI escape codes
        assert!(ansi.contains("\x1b["));
        assert!(ansi.contains("\x1b[0m")); // Reset code
    }
}
