//! Double-buffer frame renderer with cell-by-cell diff.
//!
//! The renderer maintains two buffers:
//! - **Front buffer**: Currently displayed on screen
//! - **Back buffer**: Being rendered to
//!
//! On flush, only changed cells are sent to the terminal (diff rendering).

use std::{
    io::{self, Write},
    sync::{Arc, RwLock},
};

use crate::{command::RenderCommand, compositor::Style, highlight::ColorMode};

use super::buffer::FrameBuffer;

/// Double-buffer renderer with cell-by-cell diff.
///
/// The renderer tracks changes between frames and only outputs the minimum
/// terminal commands needed to update the display, reducing flicker.
pub struct FrameRenderer {
    /// Currently displayed buffer.
    front: FrameBuffer,
    /// Buffer being rendered to.
    back: FrameBuffer,
    /// Whether we've done an initial full render.
    initialized: bool,
    /// Optional capture buffer for RPC clients.
    capture: Option<Arc<RwLock<FrameBuffer>>>,
}

impl FrameRenderer {
    /// Create a new renderer with the given dimensions.
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            front: FrameBuffer::new(width, height),
            back: FrameBuffer::new(width, height),
            initialized: false,
            capture: None,
        }
    }

    /// Resize both buffers to new dimensions.
    ///
    /// Clears both buffers after resize to prevent ghost artifacts from
    /// stale content at old row positions (e.g., statuslines from before resize).
    pub fn resize(&mut self, width: u16, height: u16) {
        self.front.resize(width, height);
        self.back.resize(width, height);
        // Clear both buffers to prevent ghost artifacts from stale content
        self.front.clear();
        self.back.clear();
        self.initialized = false; // Force full redraw after resize
    }

    /// Get the current dimensions.
    #[must_use]
    pub const fn dimensions(&self) -> (u16, u16) {
        (self.back.width(), self.back.height())
    }

    /// Get mutable access to the back buffer for rendering.
    #[must_use]
    pub const fn buffer_mut(&mut self) -> &mut FrameBuffer {
        &mut self.back
    }

    /// Get read-only access to the back buffer.
    #[must_use]
    pub const fn buffer(&self) -> &FrameBuffer {
        &self.back
    }

    /// Enable frame capture and return a handle for RPC clients.
    ///
    /// The capture buffer is updated on each flush.
    pub fn enable_capture(&mut self) -> FrameBufferHandle {
        let buffer = Arc::new(RwLock::new(self.back.clone()));
        self.capture = Some(Arc::clone(&buffer));
        FrameBufferHandle { buffer }
    }

    /// Disable frame capture.
    pub fn disable_capture(&mut self) {
        self.capture = None;
    }

    /// Update the capture buffer from the front buffer (best-effort).
    ///
    /// Silently ignores poisoned locks since capture is non-critical.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn update_capture(&self) {
        if let Some(ref capture) = self.capture
            && let Ok(mut buf) = capture.write()
        {
            buf.copy_from(&self.front);
        }
    }

    /// Flush changes to the writer using diff rendering.
    ///
    /// Computes the diff between front and back buffers, generates minimal
    /// render commands, writes them to the output, and swaps the buffers.
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the output fails.
    pub fn flush(&mut self, writer: &mut dyn Write) -> io::Result<()> {
        let commands = self.compute_diff();

        for cmd in commands {
            Self::write_command(writer, &cmd)?;
        }

        writer.flush()?;
        self.swap();
        self.initialized = true;

        // Update capture buffer if enabled (best-effort, ignores poisoned lock)
        self.update_capture();

        Ok(())
    }

    /// Compute the diff between front and back buffers.
    ///
    /// Key patterns:
    /// 1. **Style batching**: Accumulate consecutive same-style cells
    /// 2. **Reset-before-set**: Always `ResetStyle` before `SetStyle` (prevents bleed)
    /// 3. **Skip continuation cells**: Width=0 cells are never rendered
    /// 4. **Width tracking**: Track display width (columns) not character count for batching
    fn compute_diff(&self) -> Vec<RenderCommand> {
        let mut commands = Vec::new();
        let mut current_style: Option<Style> = None;
        let mut batch_start: Option<(u16, u16)> = None;
        let mut batch_chars = String::new();
        let mut batch_width: u16 = 0; // Track display columns, not character count

        for y in 0..self.back.height() {
            for x in 0..self.back.width() {
                // get(x, y) always returns Some since x < width() and y < height()
                let back_cell = self.back.get(x, y).expect("cell within bounds");
                let front_cell = self.front.get(x, y);

                // Skip continuation cells (2nd column of wide char)
                if back_cell.is_continuation {
                    continue;
                }

                // Front and back buffers always have the same dimensions,
                // so get() always returns Some for valid coordinates
                let front_cell = front_cell.expect("same-sized buffers");
                let needs_update = !self.initialized || back_cell.differs_from(front_cell);

                if needs_update {
                    // Style changed? Flush batch and reset
                    if current_style.as_ref() != Some(&back_cell.style) {
                        Self::flush_batch(
                            &mut commands,
                            &mut batch_start,
                            &mut batch_chars,
                            &mut batch_width,
                        );

                        // Reset-before-set pattern (prevents underline/bold bleed)
                        if current_style.is_some() {
                            commands.push(RenderCommand::ResetStyle);
                        }
                        commands.push(RenderCommand::SetStyle(style_to_ansi(&back_cell.style)));
                        current_style = Some(back_cell.style.clone());
                    }

                    // Check if we can continue the current batch
                    // Use batch_width (display columns) not batch_chars.len() (character count)
                    let can_batch = Self::is_batch_contiguous(batch_start, batch_width, x, y);

                    if can_batch {
                        batch_chars.push(back_cell.char);
                        batch_width += u16::from(back_cell.width);
                    } else {
                        // Start new batch
                        Self::flush_batch(
                            &mut commands,
                            &mut batch_start,
                            &mut batch_chars,
                            &mut batch_width,
                        );
                        batch_start = Some((x, y));
                        batch_chars.push(back_cell.char);
                        batch_width = u16::from(back_cell.width);
                    }
                } else {
                    // Cell unchanged, flush any pending batch
                    Self::flush_batch(
                        &mut commands,
                        &mut batch_start,
                        &mut batch_chars,
                        &mut batch_width,
                    );
                }
            }
            // End of row: flush batch (don't carry across rows)
            Self::flush_batch(&mut commands, &mut batch_start, &mut batch_chars, &mut batch_width);
        }

        // Final reset if we had a style set
        if current_style.is_some() {
            commands.push(RenderCommand::ResetStyle);
        }

        commands
    }

    /// Flush a pending batch of characters.
    fn flush_batch(
        commands: &mut Vec<RenderCommand>,
        start: &mut Option<(u16, u16)>,
        chars: &mut String,
        width: &mut u16,
    ) {
        if let Some((x, y)) = start.take() {
            commands.push(RenderCommand::MoveTo { x, y });
            commands.push(RenderCommand::Print(std::mem::take(chars)));
            *width = 0;
        }
    }

    /// Check if the current cell can extend the active batch.
    ///
    /// A batch continues when the current cell is on the same row and
    /// immediately follows the batch content. The adjacency false branch
    /// is structurally unreachable: in the left-to-right scan, unchanged
    /// cells flush the batch before any non-adjacent cell is reached.
    #[cfg_attr(coverage_nightly, coverage(off))]
    const fn is_batch_contiguous(
        batch_start: Option<(u16, u16)>,
        batch_width: u16,
        x: u16,
        y: u16,
    ) -> bool {
        match batch_start {
            Some((bx, by)) => by == y && bx + batch_width == x,
            None => false,
        }
    }

    /// Swap front and back buffers.
    fn swap(&mut self) {
        self.front.swap_with(&mut self.back);
    }

    /// Write a render command to the output.
    fn write_command(writer: &mut dyn Write, cmd: &RenderCommand) -> io::Result<()> {
        match cmd {
            RenderCommand::MoveTo { x, y } => {
                // ANSI: ESC[{row};{col}H (1-indexed)
                write!(writer, "\x1b[{};{}H", y + 1, x + 1)
            }
            RenderCommand::Print(s) => {
                write!(writer, "{s}")
            }
            RenderCommand::SetStyle(ansi_str) => {
                // SetStyle contains pre-computed ANSI escape sequence
                write!(writer, "{ansi_str}")
            }
            RenderCommand::ResetStyle => {
                write!(writer, "\x1b[0m")
            }
            RenderCommand::ShowCursor => {
                write!(writer, "\x1b[?25h")
            }
            RenderCommand::HideCursor => {
                write!(writer, "\x1b[?25l")
            }
            RenderCommand::ClearToEndOfLine => {
                write!(writer, "\x1b[K")
            }
            RenderCommand::ClearLine => {
                write!(writer, "\x1b[2K")
            }
            RenderCommand::ClearRect {
                x,
                y,
                width,
                height,
            } => {
                // Clear by writing spaces
                for row in *y..y.saturating_add(*height) {
                    write!(writer, "\x1b[{};{}H", row + 1, x + 1)?;
                    for _ in 0..*width {
                        write!(writer, " ")?;
                    }
                }
                Ok(())
            }
        }
    }
}

impl Default for FrameRenderer {
    fn default() -> Self {
        Self::new(80, 24)
    }
}

/// Thread-safe handle for RPC clients to capture frame buffer.
#[derive(Clone)]
pub struct FrameBufferHandle {
    buffer: Arc<RwLock<FrameBuffer>>,
}

impl FrameBufferHandle {
    /// Get a snapshot of the current frame buffer.
    #[must_use]
    pub fn snapshot(&self) -> Option<FrameBuffer> {
        self.buffer.read().ok().map(|b| b.clone())
    }

    /// Get the current dimensions.
    #[must_use]
    pub fn dimensions(&self) -> Option<(u16, u16)> {
        self.buffer.read().ok().map(|b| (b.width(), b.height()))
    }

    /// Convert the buffer contents to plain text.
    ///
    /// # Panics
    ///
    /// Panics if internal buffer row access is out of bounds (should not happen).
    #[must_use]
    pub fn to_plain_text(&self) -> Option<String> {
        let buffer = self.buffer.read().ok()?;
        let mut result = String::new();

        for y in 0..buffer.height() {
            // row(y) always returns Some for y < height(), which is guaranteed by the loop
            let row = buffer.row(y).expect("row within bounds");
            for cell in row {
                if !cell.is_continuation {
                    result.push(cell.char);
                }
            }
            if y + 1 < buffer.height() {
                result.push('\n');
            }
        }

        Some(result)
    }

    /// Convert the buffer contents to ANSI-colored text.
    ///
    /// # Panics
    ///
    /// Panics if internal buffer row access is out of bounds (should not happen).
    #[must_use]
    pub fn to_ansi(&self) -> Option<String> {
        let buffer = self.buffer.read().ok()?;
        let mut result = String::new();
        let mut current_style: Option<Style> = None;

        for y in 0..buffer.height() {
            // row(y) always returns Some for y < height(), which is guaranteed by the loop
            let row = buffer.row(y).expect("row within bounds");
            for cell in row {
                if cell.is_continuation {
                    continue;
                }

                // Style change?
                if current_style.as_ref() != Some(&cell.style) {
                    if current_style.is_some() {
                        result.push_str("\x1b[0m");
                    }
                    result.push_str(&style_to_ansi(&cell.style));
                    current_style = Some(cell.style.clone());
                }

                result.push(cell.char);
            }
            if y + 1 < buffer.height() {
                result.push('\n');
            }
        }

        if current_style.is_some() {
            result.push_str("\x1b[0m");
        }

        Some(result)
    }
}

/// Convert a Style to ANSI escape sequence.
///
/// Uses `TrueColor` mode for maximum color fidelity. The Style type from
/// `reovim_core` already provides `to_ansi_start()` which handles all
/// color modes, attributes, and extended underline styles.
#[cfg_attr(coverage_nightly, coverage(off))]
fn style_to_ansi(style: &Style) -> String {
    let ansi = style.to_ansi_start(ColorMode::TrueColor);
    if ansi.is_empty() {
        // Default style - emit reset to clear any previous styling
        "\x1b[0m".to_string()
    } else {
        ansi
    }
}

#[cfg(test)]
#[path = "renderer_tests.rs"]
mod tests;
