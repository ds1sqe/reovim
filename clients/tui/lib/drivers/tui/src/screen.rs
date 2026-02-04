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

    /// Render the screen to terminal.
    ///
    /// Uses differential rendering to only update changed cells.
    /// The buffer content is copied to the renderer's back buffer,
    /// then flushed with diff rendering.
    ///
    /// # Errors
    ///
    /// Returns an error if rendering fails.
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
}
