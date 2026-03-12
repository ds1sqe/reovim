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
#[path = "screen_tests.rs"]
mod tests;
