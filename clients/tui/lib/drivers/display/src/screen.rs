//! Screen management for terminal display.
//!
//! The Screen struct manages the terminal surface, providing a high-level
//! interface over the frame buffer and renderer. It coordinates:
//!
//! - Terminal dimensions
//! - Frame buffer rendering
//! - RPC frame capture
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────┐
//! │                   Screen                     │
//! │  ┌─────────────────────────────────────────┐│
//! │  │            FrameRenderer                ││
//! │  │  ┌──────────────┐  ┌──────────────┐    ││
//! │  │  │ Back Buffer  │  │ Front Buffer │    ││
//! │  │  └──────────────┘  └──────────────┘    ││
//! │  └─────────────────────────────────────────┘│
//! └─────────────────────────────────────────────┘
//!                      │
//!                      ▼
//!                Terminal Output
//! ```
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_display::{Screen, Style};
//! use std::io::stdout;
//!
//! let mut screen = Screen::new(80, 24);
//!
//! // Draw to the screen
//! screen.frame_buffer_mut().write_str(0, 0, "Hello!", &Style::default());
//!
//! // Flush to terminal
//! screen.flush(&mut stdout())?;
//! ```

use std::io::{self, Write};

use crate::{
    compositor::Style,
    frame::{FrameBuffer, FrameBufferHandle, FrameRenderer},
    policy::WindowView,
};

/// Terminal surface manager.
///
/// Screen provides a high-level interface for managing the terminal display.
/// It wraps `FrameRenderer` and coordinates rendering operations.
pub struct Screen {
    /// The frame renderer (double-buffer with diff).
    renderer: FrameRenderer,
}

impl Screen {
    /// Create a new screen with the given dimensions.
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            renderer: FrameRenderer::new(width, height),
        }
    }

    /// Resize the screen to new dimensions.
    ///
    /// This resizes the underlying frame buffers and marks the
    /// display as requiring a full redraw.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.renderer.resize(width, height);
    }

    /// Get the current screen dimensions (width, height).
    #[must_use]
    pub const fn size(&self) -> (u16, u16) {
        self.renderer.dimensions()
    }

    /// Get the screen width in columns.
    #[must_use]
    pub const fn width(&self) -> u16 {
        self.renderer.dimensions().0
    }

    /// Get the screen height in rows.
    #[must_use]
    pub const fn height(&self) -> u16 {
        self.renderer.dimensions().1
    }

    /// Get mutable access to the frame buffer for drawing.
    ///
    /// This returns the back buffer, which is being rendered to.
    /// Changes won't appear on screen until `flush()` is called.
    #[must_use]
    pub const fn frame_buffer_mut(&mut self) -> &mut FrameBuffer {
        self.renderer.buffer_mut()
    }

    /// Get read-only access to the frame buffer.
    #[must_use]
    pub const fn frame_buffer(&self) -> &FrameBuffer {
        self.renderer.buffer()
    }

    /// Clear the entire screen (fill with empty cells).
    pub fn clear(&mut self) {
        self.renderer.buffer_mut().clear();
    }

    /// Clear a rectangular region of the screen.
    pub fn clear_rect(&mut self, x: u16, y: u16, width: u16, height: u16) {
        self.renderer.buffer_mut().clear_rect(x, y, width, height);
    }

    /// Enable frame capture for RPC clients.
    ///
    /// Returns a handle that can be used to get snapshots of the frame buffer.
    /// The handle is thread-safe and can be shared across threads.
    pub fn enable_capture(&mut self) -> FrameBufferHandle {
        self.renderer.enable_capture()
    }

    /// Disable frame capture.
    pub fn disable_capture(&mut self) {
        self.renderer.disable_capture();
    }

    /// Flush the screen to the terminal.
    ///
    /// This computes the diff between the front and back buffers,
    /// writes only the changed cells to the terminal, and swaps
    /// the buffers.
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the terminal fails.
    pub fn flush(&mut self, writer: &mut dyn Write) -> io::Result<()> {
        self.renderer.flush(writer)
    }

    /// Clear the screen and fill with a style.
    ///
    /// Useful for setting a background color.
    pub fn fill(&mut self, style: &Style) {
        let (w, h) = self.size();
        let cell = crate::frame::Cell::new(' ', style.clone());
        self.renderer.buffer_mut().fill_rect(0, 0, w, h, &cell);
    }

    /// Draw a string at the given position with style.
    ///
    /// Returns the number of columns used (accounting for wide characters).
    pub fn draw_str(&mut self, x: u16, y: u16, s: &str, style: &Style) -> u16 {
        self.renderer.buffer_mut().write_str(x, y, s, style)
    }

    /// Get the inner bounds for window content area.
    ///
    /// This method calculates the area available for window content,
    /// accounting for any chrome (tab bar, status line) that might
    /// be reserved.
    ///
    /// For now, returns the full screen. Chrome reservation will be
    /// added when implementing tab/status lines.
    #[must_use]
    pub const fn content_area(&self) -> crate::window::Rect {
        crate::window::Rect::new(0, 0, self.width(), self.height())
    }

    /// Render windows at their specified positions.
    ///
    /// This is a placeholder for the full render implementation.
    /// The actual rendering will be implemented in Phase 4 with `WindowRenderer`.
    ///
    /// # Arguments
    ///
    /// * `views` - Window positions from the layout policy
    /// * `default_style` - Default style for empty areas
    pub fn render_views(&mut self, views: &[WindowView], default_style: &Style) {
        // Clear the screen first
        self.fill(default_style);

        // Draw placeholders for each window (actual content rendering in Phase 4)
        for view in views {
            let b = &view.bounds;
            // Draw a simple border to show where windows are
            if b.width > 0 && b.height > 0 {
                // Top border
                let top_line = "─".repeat(b.width.saturating_sub(2) as usize);
                self.draw_str(b.x, b.y, &format!("┌{top_line}┐"), default_style);

                // Bottom border
                let bottom_y = b.y + b.height.saturating_sub(1);
                self.draw_str(b.x, bottom_y, &format!("└{top_line}┘"), default_style);

                // Side borders
                for row in (b.y + 1)..bottom_y {
                    self.draw_str(b.x, row, "│", default_style);
                    let right_x = b.x + b.width.saturating_sub(1);
                    self.draw_str(right_x, row, "│", default_style);
                }

                // Window ID indicator
                let id_str = format!("Win {}", view.window_id.as_usize());
                let text_x = b.x + 2;
                let text_y = b.y + b.height / 2;
                #[allow(clippy::cast_possible_truncation)]
                let id_len = id_str.len() as u16;
                if text_x + id_len <= b.x + b.width.saturating_sub(1) {
                    self.draw_str(text_x, text_y, &id_str, default_style);
                }
            }
        }
    }
}

impl Default for Screen {
    fn default() -> Self {
        Self::new(80, 24)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{WindowId, window::Rect},
    };

    #[test]
    fn test_new() {
        let screen = Screen::new(80, 24);
        assert_eq!(screen.size(), (80, 24));
        assert_eq!(screen.width(), 80);
        assert_eq!(screen.height(), 24);
    }

    #[test]
    fn test_resize() {
        let mut screen = Screen::new(80, 24);
        screen.resize(100, 50);
        assert_eq!(screen.size(), (100, 50));
    }

    #[test]
    fn test_clear() {
        let mut screen = Screen::new(10, 10);
        screen.draw_str(0, 0, "Hello", &Style::default());
        screen.clear();

        // All cells should be empty
        let buf = screen.frame_buffer();
        assert!(buf.get(0, 0).unwrap().is_empty());
    }

    #[test]
    fn test_draw_str() {
        let mut screen = Screen::new(80, 24);
        let written = screen.draw_str(0, 0, "Hello", &Style::default());
        assert_eq!(written, 5);
        assert_eq!(screen.frame_buffer().get(0, 0).unwrap().char, 'H');
    }

    #[test]
    fn test_fill() {
        let mut screen = Screen::new(10, 10);
        let style = Style::default();
        screen.fill(&style);

        // All cells should be spaces with the style
        let buf = screen.frame_buffer();
        for y in 0..10 {
            for x in 0..10 {
                assert_eq!(buf.get(x, y).unwrap().char, ' ');
            }
        }
    }

    #[test]
    fn test_content_area() {
        let screen = Screen::new(80, 24);
        let area = screen.content_area();
        assert_eq!(area.x, 0);
        assert_eq!(area.y, 0);
        assert_eq!(area.width, 80);
        assert_eq!(area.height, 24);
    }

    #[test]
    fn test_flush() {
        let mut screen = Screen::new(5, 1);
        screen.draw_str(0, 0, "Test", &Style::default());

        let mut output = Vec::new();
        screen.flush(&mut output).unwrap();

        let output_str = String::from_utf8_lossy(&output);
        assert!(output_str.contains("Test"));
    }

    #[test]
    fn test_enable_capture() {
        let mut screen = Screen::new(80, 24);
        let handle = screen.enable_capture();

        screen.draw_str(0, 0, "Captured", &Style::default());
        let mut output = Vec::new();
        screen.flush(&mut output).unwrap();

        let text = handle.to_plain_text().unwrap();
        assert!(text.starts_with("Captured"));
    }

    #[test]
    fn test_render_views() {
        let mut screen = Screen::new(80, 24);
        let views = vec![
            WindowView::new(WindowId::from_raw(1), Rect::new(0, 0, 40, 24)),
            WindowView::new(WindowId::from_raw(2), Rect::new(40, 0, 40, 24)),
        ];

        screen.render_views(&views, &Style::default());

        // Check that window borders were drawn
        let buf = screen.frame_buffer();
        assert_eq!(buf.get(0, 0).unwrap().char, '┌');
        assert_eq!(buf.get(40, 0).unwrap().char, '┌');
    }

    #[test]
    fn test_default() {
        let screen = Screen::default();
        assert_eq!(screen.size(), (80, 24));
    }

    // =========================================================================
    // Coverage tests for uncovered lines
    // =========================================================================

    /// Test `frame_buffer_mut` returns mutable back buffer (lines 99-101).
    #[test]
    fn test_frame_buffer_mut() {
        let mut screen = Screen::new(10, 10);
        let buf = screen.frame_buffer_mut();
        buf.set(0, 0, crate::frame::Cell::from_char('X'));
        // Verify through the immutable accessor
        assert_eq!(screen.frame_buffer().get(0, 0).unwrap().char, 'X');
    }

    /// Test `clear_rect` clears a rectangular region (lines 115-117).
    #[test]
    fn test_clear_rect() {
        let mut screen = Screen::new(10, 10);
        // Fill the screen with non-empty cells
        screen.fill(&Style::default());
        screen.draw_str(2, 2, "Hello", &Style::default());

        // Clear a rect over that region
        screen.clear_rect(2, 2, 5, 1);

        // The cleared cells should be empty
        let buf = screen.frame_buffer();
        assert!(buf.get(2, 2).unwrap().is_empty());
        assert!(buf.get(3, 2).unwrap().is_empty());
    }

    /// Test `disable_capture` (lines 128-130).
    #[test]
    fn test_disable_capture() {
        let mut screen = Screen::new(80, 24);
        let _handle = screen.enable_capture();
        screen.disable_capture();

        // After disabling capture, flush should still work without panicking
        screen.draw_str(0, 0, "Test", &Style::default());
        let mut output = Vec::new();
        screen.flush(&mut output).unwrap();
    }

    /// Test `render_views` with window too narrow for ID text (line 216).
    #[test]
    fn test_render_views_small_window() {
        let mut screen = Screen::new(80, 24);

        // Create a window too narrow for the "Win N" text indicator
        let views = vec![WindowView::new(
            WindowId::from_raw(999),
            Rect::new(0, 0, 4, 4),
        )];

        // Should not panic even if window is too narrow for the text
        screen.render_views(&views, &Style::default());

        // Check that borders were still drawn
        let buf = screen.frame_buffer();
        assert_eq!(buf.get(0, 0).unwrap().char, '┌');
    }

    /// Test `render_views` with one zero dimension (width=0, height>0).
    #[test]
    fn test_render_views_zero_width_nonzero_height() {
        let mut screen = Screen::new(80, 24);

        let views = vec![WindowView::new(
            WindowId::from_raw(0),
            Rect::new(0, 0, 0, 10),
        )];

        screen.render_views(&views, &Style::default());

        // Zero-width window should not draw anything
        let buf = screen.frame_buffer();
        assert_eq!(buf.get(0, 0).unwrap().char, ' ');
    }

    #[test]
    fn test_render_views_nonzero_width_zero_height() {
        // Line 191: b.width > 0 (true) && b.height > 0 (false)
        let mut screen = Screen::new(80, 24);

        let views = vec![WindowView::new(
            WindowId::from_raw(0),
            Rect::new(0, 0, 10, 0),
        )];

        screen.render_views(&views, &Style::default());

        // Width>0 but height=0: should not draw anything
        let buf = screen.frame_buffer();
        assert_eq!(buf.get(0, 0).unwrap().char, ' ');
    }

    /// Test `render_views` with zero-size window (line 216 else branch).
    #[test]
    fn test_render_views_zero_size_window() {
        let mut screen = Screen::new(80, 24);

        // Create a window with zero dimensions
        let views = vec![WindowView::new(
            WindowId::from_raw(0),
            Rect::new(0, 0, 0, 0),
        )];

        screen.render_views(&views, &Style::default());

        // Zero-size window should not draw anything (cells remain as fill)
        let buf = screen.frame_buffer();
        assert_eq!(buf.get(0, 0).unwrap().char, ' ');
    }
}
