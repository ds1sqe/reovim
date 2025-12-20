//! Frame renderer with double-buffer diff-based rendering

use {
    super::FrameBuffer,
    crate::{constants::RESET_STYLE, highlight::ColorMode},
    reovim_sys::{
        cursor::MoveTo,
        queue,
        style::Print,
        terminal::{Clear, ClearType},
    },
    std::{
        io::Write,
        sync::{Arc, RwLock},
    },
};

/// Commands to emit to the terminal
#[derive(Debug, Clone)]
pub enum RenderCommand {
    /// Move cursor to position
    MoveTo(u16, u16),
    /// Print a string at current position
    Print(String),
    /// Set style (ANSI escape sequence)
    SetStyle(String),
    /// Reset style to default
    ResetStyle,
    /// Clear to end of line
    ClearToEndOfLine,
}

/// Handle to read captured frame buffer from a `FrameRenderer`
///
/// Provides thread-safe, read-only access to the latest complete frame.
/// Use this for RPC `CellGrid` format to get actual cell data instead of raw ANSI.
#[derive(Clone)]
pub struct FrameBufferHandle {
    buffer: Arc<RwLock<FrameBuffer>>,
}

impl FrameBufferHandle {
    /// Get a snapshot of the current frame buffer
    ///
    /// Returns a clone of the frame buffer for safe external use.
    ///
    /// # Panics
    ///
    /// Panics if the `RwLock` is poisoned.
    #[must_use]
    pub fn snapshot(&self) -> FrameBuffer {
        (*self.buffer.read().unwrap()).clone()
    }

    /// Get dimensions of the frame buffer
    ///
    /// # Panics
    ///
    /// Panics if the `RwLock` is poisoned.
    #[must_use]
    pub fn dimensions(&self) -> (u16, u16) {
        let buf = self.buffer.read().unwrap();
        (buf.width(), buf.height())
    }

    /// Check if the buffer is empty (all spaces)
    ///
    /// # Panics
    ///
    /// Panics if the `RwLock` is poisoned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        let buf = self.buffer.read().unwrap();
        buf.width() == 0 || buf.height() == 0
    }

    /// Get the frame buffer as plain text (characters only, no formatting)
    ///
    /// Returns a string with newline-separated rows, trailing whitespace trimmed.
    ///
    /// # Panics
    ///
    /// Panics if the `RwLock` is poisoned.
    #[must_use]
    pub fn to_plain_text(&self) -> String {
        self.buffer.read().unwrap().to_plain_text()
    }

    /// Get the frame buffer as ASCII art (for debugging)
    ///
    /// Alias for [`to_plain_text`](Self::to_plain_text).
    ///
    /// # Panics
    ///
    /// Panics if the `RwLock` is poisoned.
    #[must_use]
    pub fn to_ascii(&self) -> String {
        self.to_plain_text()
    }

    /// Get the frame buffer as ANSI-formatted text (with escape codes for colors/styles)
    ///
    /// Returns a string with ANSI escape sequences for styling and newline-separated rows.
    ///
    /// # Panics
    ///
    /// Panics if the `RwLock` is poisoned.
    #[must_use]
    pub fn to_ansi(&self, color_mode: ColorMode) -> String {
        self.buffer.read().unwrap().to_ansi(color_mode)
    }
}

/// Double-buffer frame renderer with cell-by-cell diff
///
/// Uses two buffers with swap pattern:
/// - `front`: Latest complete frame (external readers access this)
/// - `back`: Currently being rendered to
///
/// Rendering writes to `back`, then diff compares `back` vs `front` before swap.
/// After flush, buffers swap so `front` contains the just-rendered frame.
///
/// Optionally maintains a shared capture buffer for external readers (RPC clients).
pub struct FrameRenderer {
    /// Front buffer - latest complete frame (external readers access this)
    front: FrameBuffer,
    /// Back buffer - currently being rendered to
    back: FrameBuffer,
    /// Color mode for style conversion
    color_mode: ColorMode,
    /// Whether first frame has been rendered
    initialized: bool,
    /// Optional shared capture buffer for RPC clients
    capture: Option<Arc<RwLock<FrameBuffer>>>,
}

impl FrameRenderer {
    /// Create a new frame renderer with given dimensions
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            front: FrameBuffer::new(width, height),
            back: FrameBuffer::new(width, height),
            color_mode: ColorMode::TrueColor,
            initialized: false,
            capture: None,
        }
    }

    /// Enable frame buffer capture and return a handle for external readers
    ///
    /// The handle provides thread-safe access to the latest complete frame.
    /// After each flush, the capture buffer is updated with the new frame.
    #[must_use]
    pub fn enable_capture(&mut self) -> FrameBufferHandle {
        let buffer =
            Arc::new(RwLock::new(FrameBuffer::new(self.front.width(), self.front.height())));
        self.capture = Some(Arc::clone(&buffer));
        FrameBufferHandle { buffer }
    }

    /// Get a capture handle if capture is already enabled
    #[must_use]
    pub fn capture_handle(&self) -> Option<FrameBufferHandle> {
        self.capture.as_ref().map(|buffer| FrameBufferHandle {
            buffer: Arc::clone(buffer),
        })
    }

    /// Set the color mode
    #[allow(clippy::missing_const_for_fn)] // setter pattern
    pub fn set_color_mode(&mut self, mode: ColorMode) {
        self.color_mode = mode;
    }

    /// Resize the frame buffers
    ///
    /// # Panics
    ///
    /// Panics if the capture buffer's `RwLock` is poisoned.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.front.resize(width, height);
        self.back.resize(width, height);
        self.initialized = false; // Force full redraw

        // Also resize capture buffer if enabled
        if let Some(ref capture) = self.capture {
            capture.write().unwrap().resize(width, height);
        }
    }

    /// Get the width
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn width(&self) -> u16 {
        self.front.width()
    }

    /// Get the height
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn height(&self) -> u16 {
        self.front.height()
    }

    /// Get mutable access to the render buffer
    ///
    /// Content is rendered to the back buffer, which becomes the front
    /// buffer after flush.
    #[allow(clippy::missing_const_for_fn)]
    pub fn buffer_mut(&mut self) -> &mut FrameBuffer {
        &mut self.back
    }

    /// Get read-only access to the latest complete frame
    ///
    /// Returns the front buffer which always contains the most recently
    /// flushed frame. Safe to call during or after rendering.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn buffer(&self) -> &FrameBuffer {
        &self.front
    }

    /// Clear the render buffer (fills with empty cells)
    pub fn clear(&mut self) {
        self.back.clear();
    }

    /// Flush the frame to the terminal writer
    ///
    /// This computes the diff between back (new) and front (previous) buffers,
    /// writes only changed cells to the terminal, then swaps buffers.
    ///
    /// After flush:
    /// - front contains the just-rendered frame (was back)
    /// - back contains the previous frame (was front), ready for next render
    /// - capture buffer (if enabled) is updated with the new frame
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the terminal fails.
    ///
    /// # Panics
    ///
    /// Panics if the capture buffer's `RwLock` is poisoned.
    pub fn flush<W: Write>(&mut self, writer: &mut W) -> std::io::Result<()> {
        let commands = if self.initialized {
            self.compute_diff()
        } else {
            self.initialized = true;
            self.emit_full_frame()
        };

        Self::execute_commands(writer, &commands)?;

        // Swap buffers: front ↔ back
        // After swap:
        // - front = just-rendered frame (was back)
        // - back = previous frame (was front), will be overwritten next render
        std::mem::swap(&mut self.front, &mut self.back);

        // Update capture buffer with the new frame (front now has the latest)
        if let Some(ref capture) = self.capture {
            capture.write().unwrap().copy_from(&self.front);
        }

        writer.flush()
    }

    /// Force a full redraw on next flush
    pub const fn invalidate(&mut self) {
        self.initialized = false;
    }

    /// Compute diff between back (current) and front (previous) buffers
    ///
    /// Returns commands for only changed cells, with batching for
    /// consecutive cells with the same style.
    fn compute_diff(&self) -> Vec<RenderCommand> {
        let mut commands = Vec::new();
        let mut pending_chars = String::new();
        let mut pending_start_x: u16 = 0;
        let mut pending_y: u16 = 0;
        let mut pending_style: Option<String> = None;
        let mut last_emitted_style: Option<String> = None;

        // Flush pending characters to commands
        let flush_pending = |commands: &mut Vec<RenderCommand>,
                             pending: &mut String,
                             start_x: u16,
                             y: u16,
                             style: &Option<String>,
                             last_emitted: &mut Option<String>| {
            if !pending.is_empty() {
                commands.push(RenderCommand::MoveTo(start_x, y));

                if let Some(s) = style {
                    // Check if style changed from last emitted
                    let style_changed = last_emitted.as_ref() != Some(s);

                    if style_changed {
                        // Always reset before setting new style to clear any lingering attributes
                        // (e.g., underline from previous cell)
                        if last_emitted.as_ref().is_some_and(|prev| !prev.is_empty()) {
                            commands.push(RenderCommand::ResetStyle);
                        }
                        if !s.is_empty() {
                            commands.push(RenderCommand::SetStyle(s.clone()));
                        }
                        *last_emitted = Some(s.clone());
                    }
                }

                commands.push(RenderCommand::Print(std::mem::take(pending)));
            }
        };

        for y in 0..self.back.height() {
            for x in 0..self.back.width() {
                let Some(curr_cell) = self.back.get(x, y) else {
                    continue;
                };
                let prev_cell = self.front.get(x, y);

                // Check if cell differs
                let differs = prev_cell.is_none_or(|p| curr_cell.differs_from(p));

                if differs {
                    let style_str = curr_cell.style.to_ansi_start(self.color_mode);

                    // Check if we can batch with pending (same row, consecutive, same style)
                    let can_batch = pending_y == y
                        && u16::try_from(pending_start_x as usize + pending_chars.len())
                            .is_ok_and(|expected| expected == x)
                        && pending_style.as_ref() == Some(&style_str);

                    if can_batch {
                        pending_chars.push(curr_cell.char);
                    } else {
                        // Flush previous batch
                        flush_pending(
                            &mut commands,
                            &mut pending_chars,
                            pending_start_x,
                            pending_y,
                            &pending_style,
                            &mut last_emitted_style,
                        );

                        // Start new batch
                        pending_chars.push(curr_cell.char);
                        pending_start_x = x;
                        pending_y = y;
                        pending_style = Some(style_str);
                    }
                }
            }

            // Flush at end of each row
            flush_pending(
                &mut commands,
                &mut pending_chars,
                pending_start_x,
                pending_y,
                &pending_style,
                &mut last_emitted_style,
            );
        }

        // Final reset if any style was emitted
        if last_emitted_style.as_ref().is_some_and(|s| !s.is_empty()) {
            commands.push(RenderCommand::ResetStyle);
        }

        commands
    }

    /// Emit commands for the entire frame (used on first render)
    fn emit_full_frame(&self) -> Vec<RenderCommand> {
        let mut commands = Vec::new();
        let mut last_style: Option<String> = None;

        for y in 0..self.back.height() {
            commands.push(RenderCommand::MoveTo(0, y));
            let mut line_content = String::new();

            if let Some(row) = self.back.row(y) {
                for cell in row {
                    let style_str = cell.style.to_ansi_start(self.color_mode);

                    if last_style.as_ref() != Some(&style_str) {
                        // Flush pending content
                        if !line_content.is_empty() {
                            commands.push(RenderCommand::Print(std::mem::take(&mut line_content)));
                        }

                        // Reset before setting new style to clear lingering attributes
                        if last_style.as_ref().is_some_and(|s| !s.is_empty()) {
                            commands.push(RenderCommand::ResetStyle);
                        }

                        // Set new style if non-empty
                        if !style_str.is_empty() {
                            commands.push(RenderCommand::SetStyle(style_str.clone()));
                        }
                        last_style = Some(style_str);
                    }

                    line_content.push(cell.char);
                }
            }

            // Flush remaining content
            if !line_content.is_empty() {
                commands.push(RenderCommand::Print(line_content));
            }
        }

        // Reset at end
        if last_style.is_some() {
            commands.push(RenderCommand::ResetStyle);
        }

        commands
    }

    /// Execute render commands on the writer
    fn execute_commands<W: Write>(
        writer: &mut W,
        commands: &[RenderCommand],
    ) -> std::io::Result<()> {
        for cmd in commands {
            match cmd {
                RenderCommand::MoveTo(x, y) => {
                    queue!(writer, MoveTo(*x, *y))?;
                }
                RenderCommand::Print(s) => {
                    queue!(writer, Print(s))?;
                }
                RenderCommand::SetStyle(s) => {
                    queue!(writer, Print(s))?;
                }
                RenderCommand::ResetStyle => {
                    queue!(writer, Print(RESET_STYLE))?;
                }
                RenderCommand::ClearToEndOfLine => {
                    queue!(writer, Clear(ClearType::UntilNewLine))?;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::frame::Cell};

    #[test]
    fn test_renderer_new() {
        let renderer = FrameRenderer::new(80, 24);
        assert_eq!(renderer.width(), 80);
        assert_eq!(renderer.height(), 24);
    }

    #[test]
    fn test_buffer_access() {
        let mut renderer = FrameRenderer::new(10, 5);
        renderer.buffer_mut().set(0, 0, Cell::from_char('X'));
        // Before flush, buffer() returns front which is empty
        assert_eq!(renderer.buffer().get(0, 0).map(|c| c.char), Some(' '));

        // After flush, buffer() returns front which has the rendered content
        let mut output = Vec::new();
        renderer.flush(&mut output).unwrap();
        assert_eq!(renderer.buffer().get(0, 0).map(|c| c.char), Some('X'));
    }

    #[test]
    fn test_flush_to_vec() {
        let mut renderer = FrameRenderer::new(10, 5);
        renderer.buffer_mut().set(0, 0, Cell::from_char('H'));
        renderer.buffer_mut().set(1, 0, Cell::from_char('i'));

        let mut output = Vec::new();
        renderer.flush(&mut output).unwrap();

        // Should have written something
        assert!(!output.is_empty());
    }

    #[test]
    fn test_resize() {
        let mut renderer = FrameRenderer::new(80, 24);
        renderer.resize(120, 40);
        assert_eq!(renderer.width(), 120);
        assert_eq!(renderer.height(), 40);
    }

    #[test]
    fn test_diff_only_changed_cells() {
        let mut renderer = FrameRenderer::new(10, 2);

        // First render - full frame
        renderer.buffer_mut().set(0, 0, Cell::from_char('A'));
        let mut output = Vec::new();
        renderer.flush(&mut output).unwrap();
        let first_len = output.len();

        // Second render - change one cell
        renderer.buffer_mut().set(1, 0, Cell::from_char('B'));
        output.clear();
        renderer.flush(&mut output).unwrap();

        // Diff render should be smaller than full render
        assert!(output.len() < first_len);
    }

    #[test]
    fn test_double_buffer_swap() {
        let mut renderer = FrameRenderer::new(10, 5);

        // Render frame 1
        renderer.buffer_mut().set(0, 0, Cell::from_char('1'));
        let mut output = Vec::new();
        renderer.flush(&mut output).unwrap();
        assert_eq!(renderer.buffer().get(0, 0).map(|c| c.char), Some('1'));

        // Render frame 2
        renderer.clear();
        renderer.buffer_mut().set(0, 0, Cell::from_char('2'));
        renderer.flush(&mut output).unwrap();
        assert_eq!(renderer.buffer().get(0, 0).map(|c| c.char), Some('2'));

        // Render frame 3
        renderer.clear();
        renderer.buffer_mut().set(0, 0, Cell::from_char('3'));
        renderer.flush(&mut output).unwrap();
        assert_eq!(renderer.buffer().get(0, 0).map(|c| c.char), Some('3'));
    }
}
