//! Screen buffer and rendering.
//!
//! Combines `FrameBuffer` (storage) with `FrameRenderer` (output)
//! into a unified screen abstraction.

use std::io::Write;

use crate::{
    frame::{FrameBuffer, FrameRenderer},
    style::Style,
};

use crate::Terminal;

/// Screen buffer with integrated rendering.
///
/// Wraps a `FrameBuffer` for content storage and `FrameRenderer`
/// for efficient delta-based terminal output.
pub struct Screen {
    /// Double-buffered frame storage.
    buffer: FrameBuffer,
    /// Terminal renderer.
    renderer: FrameRenderer,
    /// Screen width.
    width: u16,
    /// Screen height.
    height: u16,
}

impl Screen {
    /// Create a new screen with the given dimensions.
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            buffer: FrameBuffer::new(width, height),
            renderer: FrameRenderer::new(width, height),
            width,
            height,
        }
    }

    /// Create a screen matching terminal size.
    ///
    /// # Errors
    ///
    /// Returns an error if terminal size cannot be determined.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn from_terminal_size() -> std::io::Result<Self> {
        let (width, height) = Terminal::size()?;
        Ok(Self::new(width, height))
    }

    /// Resize the screen.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.buffer = FrameBuffer::new(width, height);
        self.renderer = FrameRenderer::new(width, height);
        self.width = width;
        self.height = height;
    }

    /// Get screen width.
    #[must_use]
    pub const fn width(&self) -> u16 {
        self.width
    }

    /// Get screen height.
    #[must_use]
    pub const fn height(&self) -> u16 {
        self.height
    }

    /// Clear the screen buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Write a character at position with style.
    pub fn put_char(&mut self, x: u16, y: u16, ch: char, style: &Style) {
        self.buffer.put_char(x, y, ch, style);
    }

    /// Write a string at position with style.
    pub fn write_str(&mut self, x: u16, y: u16, text: &str, style: &Style) {
        self.buffer.write_str(x, y, text, style);
    }

    /// Write a string centered on a row.
    #[allow(clippy::cast_possible_truncation)]
    pub fn write_centered(&mut self, y: u16, text: &str, style: &Style) {
        let text_len = text.chars().count() as u16;
        let x = self.width.saturating_sub(text_len) / 2;
        self.buffer.write_str(x, y, text, style);
    }

    /// Fill a region with a character.
    pub fn fill_region(
        &mut self,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        ch: char,
        style: &Style,
    ) {
        for row in y..y.saturating_add(height).min(self.height) {
            for col in x..x.saturating_add(width).min(self.width) {
                self.buffer.put_char(col, row, ch, style);
            }
        }
    }

    /// Fill a horizontal line.
    pub fn fill_horizontal(&mut self, x: u16, y: u16, width: u16, ch: char, style: &Style) {
        self.fill_region(x, y, width, 1, ch, style);
    }

    /// Fill a vertical line.
    pub fn fill_vertical(&mut self, x: u16, y: u16, height: u16, ch: char, style: &Style) {
        self.fill_region(x, y, 1, height, ch, style);
    }

    /// Draw a box outline.
    pub fn draw_box(&mut self, x: u16, y: u16, width: u16, height: u16, style: &Style) {
        if width < 2 || height < 2 {
            return;
        }

        let right = x.saturating_add(width).saturating_sub(1);
        let bottom = y.saturating_add(height).saturating_sub(1);

        // Corners
        self.put_char(x, y, '┌', style);
        self.put_char(right, y, '┐', style);
        self.put_char(x, bottom, '└', style);
        self.put_char(right, bottom, '┘', style);

        // Horizontal edges
        for col in x + 1..right {
            self.put_char(col, y, '─', style);
            self.put_char(col, bottom, '─', style);
        }

        // Vertical edges
        for row in y + 1..bottom {
            self.put_char(x, row, '│', style);
            self.put_char(right, row, '│', style);
        }
    }

    /// Get reference to underlying frame buffer.
    #[must_use]
    pub const fn buffer(&self) -> &FrameBuffer {
        &self.buffer
    }

    /// Get mutable reference to underlying frame buffer.
    pub const fn buffer_mut(&mut self) -> &mut FrameBuffer {
        &mut self.buffer
    }

    /// Overlay a background color on an existing cell (Issue #474).
    ///
    /// Preserves the existing character and foreground color,
    /// only changing the background. Used for remote selection
    /// highlighting without overwriting buffer content.
    pub fn overlay_bg(&mut self, x: u16, y: u16, bg: reovim_arch::Color) {
        if let Some(cell) = self.buffer.get(x, y).cloned() {
            let new_style = cell.style.with_bg(bg);
            let new_cell = crate::frame::Cell::new(cell.char, new_style);
            self.buffer.set(x, y, new_cell);
        }
    }

    /// Apply style to existing cell without replacing character.
    ///
    /// Used for cursor overlays where we want to show both the character
    /// and a cursor indicator (via background color). Delegates to
    /// `FrameBuffer::apply_style()`.
    pub fn apply_style(&mut self, x: u16, y: u16, style: &Style) {
        self.buffer.apply_style(x, y, style);
    }

    /// Render the screen to terminal.
    ///
    /// Uses differential rendering to only update changed cells.
    /// The buffer content is copied to the renderer's back buffer,
    /// then flushed with diff rendering.
    ///
    /// # Errors
    ///
    /// Returns an error if rendering fails.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn render(&mut self, terminal: &mut Terminal) -> std::io::Result<()> {
        // Copy buffer content to renderer's back buffer
        self.sync_to_renderer();
        // Flush changes to terminal
        self.renderer.flush(terminal.stdout())?;
        terminal.flush()
    }

    /// Render the screen to any writer.
    ///
    /// # Errors
    ///
    /// Returns an error if rendering fails.
    pub fn render_to<W: Write>(&mut self, writer: &mut W) -> std::io::Result<()> {
        self.sync_to_renderer();
        self.renderer.flush(writer)
    }

    /// Sync buffer content to renderer's back buffer.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn sync_to_renderer(&mut self) {
        let renderer_buffer = self.renderer.buffer_mut();
        // Copy each cell from our buffer to the renderer's back buffer
        for y in 0..self.height {
            if let Some(row) = self.buffer.row(y) {
                for (x, cell) in row.iter().enumerate() {
                    #[allow(clippy::cast_possible_truncation)]
                    let x = x as u16;
                    renderer_buffer.put_char(x, y, cell.char, &cell.style);
                }
            }
        }
    }

    /// Force full redraw on next render.
    ///
    /// This is done by resizing the renderer which clears both buffers.
    pub fn invalidate(&mut self) {
        // Resize to same size forces reinitialization
        self.renderer.resize(self.width, self.height);
    }
}

impl std::fmt::Debug for Screen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Screen")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screen_new() {
        let screen = Screen::new(80, 24);
        assert_eq!(screen.width(), 80);
        assert_eq!(screen.height(), 24);
    }

    #[test]
    fn test_screen_resize() {
        let mut screen = Screen::new(80, 24);
        screen.resize(120, 40);
        assert_eq!(screen.width(), 120);
        assert_eq!(screen.height(), 40);
    }

    #[test]
    fn test_screen_write_str() {
        let mut screen = Screen::new(80, 24);
        screen.write_str(0, 0, "Hello", &Style::default());

        // Verify via buffer
        let row = screen.buffer().row(0).expect("row exists");
        assert_eq!(row[0].char, 'H');
        assert_eq!(row[1].char, 'e');
        assert_eq!(row[2].char, 'l');
        assert_eq!(row[3].char, 'l');
        assert_eq!(row[4].char, 'o');
    }

    #[test]
    fn test_screen_clear() {
        let mut screen = Screen::new(80, 24);
        screen.write_str(0, 0, "Hello", &Style::default());
        screen.clear();

        let row = screen.buffer().row(0).expect("row exists");
        assert_eq!(row[0].char, ' ');
    }

    #[test]
    fn test_screen_put_char() {
        let mut screen = Screen::new(80, 24);
        screen.put_char(5, 3, 'X', &Style::default());

        let row = screen.buffer().row(3).expect("row exists");
        assert_eq!(row[5].char, 'X');
    }

    #[test]
    fn test_screen_draw_box() {
        let mut screen = Screen::new(80, 24);
        screen.draw_box(0, 0, 5, 3, &Style::default());

        let row0 = screen.buffer().row(0).expect("row exists");
        let row1 = screen.buffer().row(1).expect("row exists");
        let row2 = screen.buffer().row(2).expect("row exists");

        // Corners
        assert_eq!(row0[0].char, '┌');
        assert_eq!(row0[4].char, '┐');
        assert_eq!(row2[0].char, '└');
        assert_eq!(row2[4].char, '┘');

        // Edges
        assert_eq!(row0[1].char, '─');
        assert_eq!(row1[0].char, '│');
    }

    #[test]
    fn test_screen_draw_box_too_small() {
        let mut screen = Screen::new(80, 24);
        // Box too small (width < 2)
        screen.draw_box(0, 0, 1, 3, &Style::default());
        // Should not crash, just do nothing
        let row0 = screen.buffer().row(0).expect("row exists");
        assert_eq!(row0[0].char, ' ');
    }

    #[test]
    fn test_screen_write_centered() {
        let mut screen = Screen::new(80, 24);
        screen.write_centered(0, "Hello", &Style::default());

        let row = screen.buffer().row(0).expect("row exists");
        // Should be centered: (80 - 5) / 2 = 37
        assert_eq!(row[37].char, 'H');
        assert_eq!(row[38].char, 'e');
    }

    #[test]
    fn test_screen_fill_region() {
        let mut screen = Screen::new(80, 24);
        screen.fill_region(5, 3, 4, 2, 'X', &Style::default());

        let row3 = screen.buffer().row(3).expect("row exists");
        let row4 = screen.buffer().row(4).expect("row exists");
        assert_eq!(row3[5].char, 'X');
        assert_eq!(row3[8].char, 'X');
        assert_eq!(row4[5].char, 'X');
    }

    #[test]
    fn test_screen_fill_horizontal() {
        let mut screen = Screen::new(80, 24);
        screen.fill_horizontal(10, 5, 5, '-', &Style::default());

        let row = screen.buffer().row(5).expect("row exists");
        assert_eq!(row[10].char, '-');
        assert_eq!(row[14].char, '-');
        assert_eq!(row[15].char, ' '); // Should not fill beyond width
    }

    #[test]
    fn test_screen_fill_vertical() {
        let mut screen = Screen::new(80, 24);
        screen.fill_vertical(10, 5, 3, '|', &Style::default());

        let row5 = screen.buffer().row(5).expect("row exists");
        let row6 = screen.buffer().row(6).expect("row exists");
        let row7 = screen.buffer().row(7).expect("row exists");
        assert_eq!(row5[10].char, '|');
        assert_eq!(row6[10].char, '|');
        assert_eq!(row7[10].char, '|');
    }

    #[test]
    fn test_screen_buffer_mut() {
        let mut screen = Screen::new(80, 24);
        screen.buffer_mut().put_char(5, 5, 'Z', &Style::default());

        let row = screen.buffer().row(5).expect("row exists");
        assert_eq!(row[5].char, 'Z');
    }

    #[test]
    fn test_screen_overlay_bg() {
        let mut screen = Screen::new(80, 24);
        screen.put_char(5, 5, 'A', &Style::default());
        screen.overlay_bg(5, 5, reovim_arch::Color::Red);

        let cell = screen.buffer().get(5, 5).expect("cell exists");
        assert_eq!(cell.char, 'A');
        assert_eq!(cell.style.bg, Some(reovim_arch::Color::Red));
    }

    #[test]
    fn test_screen_overlay_bg_out_of_bounds() {
        let mut screen = Screen::new(10, 10);
        // Should not crash
        screen.overlay_bg(100, 100, reovim_arch::Color::Red);
    }

    #[test]
    fn test_screen_apply_style() {
        let mut screen = Screen::new(80, 24);
        screen.put_char(3, 3, 'B', &Style::default());
        let style = Style::new().with_bg(reovim_arch::Color::Blue);
        screen.apply_style(3, 3, &style);

        let cell = screen.buffer().get(3, 3).expect("cell exists");
        assert_eq!(cell.char, 'B');
        assert_eq!(cell.style.bg, Some(reovim_arch::Color::Blue));
    }

    #[test]
    fn test_screen_invalidate() {
        let mut screen = Screen::new(80, 24);
        screen.write_str(0, 0, "Test", &Style::default());
        screen.invalidate();
        // Should not crash
        assert_eq!(screen.width(), 80);
    }

    #[test]
    fn test_screen_debug() {
        let screen = Screen::new(80, 24);
        let debug = format!("{screen:?}");
        assert!(debug.contains("Screen"));
        assert!(debug.contains("80"));
        assert!(debug.contains("24"));
    }

    #[test]
    fn test_screen_from_terminal_size_fallback() {
        // In CI without terminal, this might fail
        let result = Screen::from_terminal_size();
        // Just ensure it doesn't panic
        let _ = result;
    }

    #[test]
    fn test_screen_render_to() {
        let mut screen = Screen::new(10, 2);
        screen.write_str(0, 0, "Hello", &Style::default());
        screen.write_str(0, 1, "World", &Style::default());

        let mut output = Vec::new();
        screen.render_to(&mut output).unwrap();

        let out = String::from_utf8_lossy(&output);
        assert!(out.contains("Hello"));
        assert!(out.contains("World"));
    }

    #[test]
    fn test_screen_render_to_twice() {
        let mut screen = Screen::new(10, 1);
        screen.write_str(0, 0, "First", &Style::default());

        let mut out1 = Vec::new();
        screen.render_to(&mut out1).unwrap();
        assert!(!out1.is_empty());

        // Render same content again (diff should detect no changes)
        screen.write_str(0, 0, "First", &Style::default());
        let mut out2 = Vec::new();
        screen.render_to(&mut out2).unwrap();
        assert!(out2.len() <= out1.len());
    }

    #[test]
    fn test_screen_render_to_with_style() {
        let mut screen = Screen::new(10, 1);
        let styled = Style::new().with_fg(reovim_arch::Color::Green).bold();
        screen.write_str(0, 0, "Styled", &styled);

        let mut output = Vec::new();
        screen.render_to(&mut output).unwrap();

        let out = String::from_utf8_lossy(&output);
        assert!(out.contains("Styled"));
        // Should include ANSI escape codes
        assert!(out.contains("\x1b["));
    }

    #[test]
    fn test_screen_draw_box_height_too_small() {
        let mut screen = Screen::new(80, 24);
        // Height < 2 should do nothing
        screen.draw_box(0, 0, 5, 1, &Style::default());
        let row0 = screen.buffer().row(0).expect("row exists");
        assert_eq!(row0[0].char, ' ');
    }

    #[test]
    fn test_screen_draw_box_both_too_small() {
        let mut screen = Screen::new(80, 24);
        // Both width and height < 2
        screen.draw_box(0, 0, 1, 1, &Style::default());
        let row0 = screen.buffer().row(0).expect("row exists");
        assert_eq!(row0[0].char, ' ');
    }

    #[test]
    fn test_screen_draw_box_larger() {
        let mut screen = Screen::new(80, 24);
        screen.draw_box(1, 1, 6, 4, &Style::default());

        let row1 = screen.buffer().row(1).expect("row exists");
        let row2 = screen.buffer().row(2).expect("row exists");
        let row3 = screen.buffer().row(3).expect("row exists");
        let row4 = screen.buffer().row(4).expect("row exists");

        // Top-left corner
        assert_eq!(row1[1].char, '\u{250c}'); // ┌
        // Top-right corner
        assert_eq!(row1[6].char, '\u{2510}'); // ┐
        // Bottom-left corner
        assert_eq!(row4[1].char, '\u{2514}'); // └
        // Bottom-right corner
        assert_eq!(row4[6].char, '\u{2518}'); // ┘
        // Vertical edges
        assert_eq!(row2[1].char, '\u{2502}'); // │
        assert_eq!(row2[6].char, '\u{2502}');
        assert_eq!(row3[1].char, '\u{2502}');
        assert_eq!(row3[6].char, '\u{2502}');
        // Horizontal edges
        assert_eq!(row1[2].char, '\u{2500}'); // ─
        assert_eq!(row1[5].char, '\u{2500}');
        assert_eq!(row4[2].char, '\u{2500}');
    }

    #[test]
    fn test_screen_write_centered_long_text() {
        let mut screen = Screen::new(10, 1);
        // Text longer than screen width
        screen.write_centered(0, "VeryLongTextString", &Style::default());
        // x = (10 - 18) / 2 = 0 (saturating_sub makes it 0)
        let row = screen.buffer().row(0).expect("row exists");
        assert_eq!(row[0].char, 'V');
    }

    #[test]
    fn test_screen_fill_region_clamped() {
        let mut screen = Screen::new(5, 5);
        // Fill region that extends beyond screen bounds
        screen.fill_region(3, 3, 10, 10, 'X', &Style::default());
        let row3 = screen.buffer().row(3).expect("row exists");
        assert_eq!(row3[3].char, 'X');
        assert_eq!(row3[4].char, 'X');
        let row4 = screen.buffer().row(4).expect("row exists");
        assert_eq!(row4[3].char, 'X');
        assert_eq!(row4[4].char, 'X');
    }

    #[test]
    fn test_screen_invalidate_then_render() {
        let mut screen = Screen::new(10, 1);
        screen.write_str(0, 0, "Hello", &Style::default());

        let mut out1 = Vec::new();
        screen.render_to(&mut out1).unwrap();

        // Invalidate forces full redraw
        screen.invalidate();
        screen.write_str(0, 0, "Hello", &Style::default());

        let mut out2 = Vec::new();
        screen.render_to(&mut out2).unwrap();
        // Full redraw should produce output
        let out_str = String::from_utf8_lossy(&out2);
        assert!(out_str.contains("Hello"));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_screen_sync_to_renderer() {
        // Test that sync_to_renderer copies buffer content properly
        let mut screen = Screen::new(5, 2);
        screen.put_char(0, 0, 'A', &Style::default());
        screen.put_char(1, 0, 'B', &Style::default());
        screen.put_char(0, 1, 'C', &Style::default());

        let mut output = Vec::new();
        screen.render_to(&mut output).unwrap();

        let out = String::from_utf8_lossy(&output);
        assert!(out.contains('A'));
        assert!(out.contains('B'));
        assert!(out.contains('C'));
    }

    #[test]
    fn test_screen_overlay_bg_preserves_fg() {
        let mut screen = Screen::new(10, 10);
        let style = Style::new().with_fg(reovim_arch::Color::Green);
        screen.put_char(2, 2, 'X', &style);
        screen.overlay_bg(2, 2, reovim_arch::Color::Yellow);

        let cell = screen.buffer().get(2, 2).expect("cell exists");
        assert_eq!(cell.char, 'X');
        assert_eq!(cell.style.fg, Some(reovim_arch::Color::Green));
        assert_eq!(cell.style.bg, Some(reovim_arch::Color::Yellow));
    }

    #[test]
    fn test_screen_apply_style_preserves_char() {
        let mut screen = Screen::new(10, 10);
        screen.put_char(1, 1, 'Q', &Style::default());
        let new_style = Style::new()
            .with_fg(reovim_arch::Color::Red)
            .with_bg(reovim_arch::Color::Blue);
        screen.apply_style(1, 1, &new_style);

        let cell = screen.buffer().get(1, 1).expect("cell exists");
        assert_eq!(cell.char, 'Q');
        assert_eq!(cell.style.fg, Some(reovim_arch::Color::Red));
        assert_eq!(cell.style.bg, Some(reovim_arch::Color::Blue));
    }

    #[test]
    fn test_screen_apply_style_out_of_bounds() {
        let mut screen = Screen::new(5, 5);
        let style = Style::new().with_bg(reovim_arch::Color::Red);
        // Should not crash
        screen.apply_style(100, 100, &style);
    }

    #[test]
    fn test_screen_resize_clears_content() {
        let mut screen = Screen::new(10, 10);
        screen.write_str(0, 0, "Hello", &Style::default());
        screen.resize(5, 5);

        // After resize, buffer is recreated
        let row = screen.buffer().row(0).expect("row exists");
        assert_eq!(row[0].char, ' ');
    }
}
