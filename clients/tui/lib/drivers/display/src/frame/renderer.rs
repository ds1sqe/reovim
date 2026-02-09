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
            self.write_command(writer, &cmd)?;
        }

        writer.flush()?;
        self.swap();
        self.initialized = true;

        // Update capture buffer if enabled
        if let Some(ref capture) = self.capture
            && let Ok(mut buf) = capture.write()
        {
            buf.copy_from(&self.front);
        }

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
                let Some(back_cell) = self.back.get(x, y) else {
                    continue;
                };
                let front_cell = self.front.get(x, y);

                // Skip continuation cells (2nd column of wide char)
                if back_cell.is_continuation {
                    continue;
                }

                let needs_update = !self.initialized
                    || front_cell.is_none()
                    || back_cell.differs_from(front_cell.unwrap());

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
                    let can_batch =
                        batch_start.is_some_and(|(bx, by)| by == y && bx + batch_width == x);

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

    /// Swap front and back buffers.
    fn swap(&mut self) {
        self.front.swap_with(&mut self.back);
    }

    /// Write a render command to the output.
    #[allow(clippy::unused_self)]
    fn write_command(&self, writer: &mut dyn Write, cmd: &RenderCommand) -> io::Result<()> {
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
    #[must_use]
    pub fn to_plain_text(&self) -> Option<String> {
        let buffer = self.buffer.read().ok()?;
        let mut result = String::new();

        for y in 0..buffer.height() {
            if let Some(row) = buffer.row(y) {
                for cell in row {
                    if !cell.is_continuation {
                        result.push(cell.char);
                    }
                }
            }
            if y + 1 < buffer.height() {
                result.push('\n');
            }
        }

        Some(result)
    }

    /// Convert the buffer contents to ANSI-colored text.
    #[must_use]
    pub fn to_ansi(&self) -> Option<String> {
        let buffer = self.buffer.read().ok()?;
        let mut result = String::new();
        let mut current_style: Option<Style> = None;

        for y in 0..buffer.height() {
            if let Some(row) = buffer.row(y) {
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
mod tests {
    use super::{super::cell::Cell, *};

    #[test]
    fn test_new() {
        let renderer = FrameRenderer::new(80, 24);
        assert_eq!(renderer.dimensions(), (80, 24));
    }

    #[test]
    fn test_resize() {
        let mut renderer = FrameRenderer::new(80, 24);
        renderer.resize(100, 50);
        assert_eq!(renderer.dimensions(), (100, 50));
    }

    #[test]
    fn test_buffer_mut() {
        let mut renderer = FrameRenderer::new(10, 10);
        renderer.buffer_mut().set(0, 0, Cell::from_char('x'));
        assert_eq!(renderer.buffer().get(0, 0).unwrap().char, 'x');
    }

    #[test]
    fn test_flush() {
        let mut renderer = FrameRenderer::new(5, 1);
        renderer
            .buffer_mut()
            .write_str(0, 0, "Hello", &Style::default());

        let mut output = Vec::new();
        renderer.flush(&mut output).unwrap();

        // Output should contain ANSI sequences
        let output_str = String::from_utf8_lossy(&output);
        assert!(output_str.contains("Hello"));
    }

    #[test]
    fn test_diff_only_changed() {
        let mut renderer = FrameRenderer::new(5, 1);

        // First render: "Hello"
        renderer
            .buffer_mut()
            .write_str(0, 0, "Hello", &Style::default());
        let mut output = Vec::new();
        renderer.flush(&mut output).unwrap();

        // Second render: "Hallo" (only 'e' -> 'a')
        renderer.buffer_mut().set(1, 0, Cell::from_char('a'));
        let mut output2 = Vec::new();
        renderer.flush(&mut output2).unwrap();

        // Second output should be shorter (only updating one char)
        let output_str = String::from_utf8_lossy(&output2);
        assert!(output_str.contains('a'));
    }

    #[test]
    fn test_capture() {
        let mut renderer = FrameRenderer::new(10, 1);
        let handle = renderer.enable_capture();

        renderer
            .buffer_mut()
            .write_str(0, 0, "Test", &Style::default());
        let mut output = Vec::new();
        renderer.flush(&mut output).unwrap();

        let text = handle.to_plain_text().unwrap();
        assert!(text.starts_with("Test"));
    }

    #[test]
    fn test_capture_dimensions() {
        let mut renderer = FrameRenderer::new(80, 24);
        let handle = renderer.enable_capture();

        let dims = handle.dimensions().unwrap();
        assert_eq!(dims, (80, 24));
    }

    #[test]
    fn test_style_batching() {
        let mut renderer = FrameRenderer::new(10, 1);

        // Write with same style - should batch
        renderer
            .buffer_mut()
            .write_str(0, 0, "Hello", &Style::default());

        let commands = renderer.compute_diff();

        // Should have: SetStyle, MoveTo, Print, ResetStyle
        // Not multiple MoveTo/Print pairs
        let print_count = commands
            .iter()
            .filter(|c| matches!(c, RenderCommand::Print(_)))
            .count();
        assert_eq!(print_count, 1); // All batched into one print
    }

    #[test]
    fn test_skip_continuation() {
        let mut renderer = FrameRenderer::new(10, 1);

        // Write wide char at position 0 (occupies columns 0-1)
        renderer
            .buffer_mut()
            .put_char(0, 0, '中', &Style::default());

        let commands = renderer.compute_diff();

        // Should batch everything into one print, not print continuation separately
        let print_cmds: Vec<_> = commands
            .iter()
            .filter_map(|c| {
                if let RenderCommand::Print(s) = c {
                    Some(s)
                } else {
                    None
                }
            })
            .collect();

        // One batched print (wide char + remaining spaces)
        assert_eq!(print_cmds.len(), 1);
        // Verify wide char is at the start, continuation cell not printed separately
        assert!(print_cmds[0].starts_with('中'), "Wide char should be at start");
        // Total content: '中' (width 2) + 8 spaces = 10 columns
        // Character count: 1 ('中') + 8 (' ') = 9 chars
        assert_eq!(print_cmds[0].chars().count(), 9);
    }

    #[test]
    fn test_default() {
        let renderer = FrameRenderer::default();
        assert_eq!(renderer.dimensions(), (80, 24));
    }

    #[test]
    fn test_disable_capture() {
        let mut renderer = FrameRenderer::new(10, 1);
        let _handle = renderer.enable_capture();

        renderer.disable_capture();
        // After disabling, flush should not update capture buffer
        renderer
            .buffer_mut()
            .write_str(0, 0, "Test", &Style::default());
        let mut output = Vec::new();
        renderer.flush(&mut output).unwrap();
        // Just verify no panic
    }

    #[test]
    fn test_capture_snapshot() {
        let mut renderer = FrameRenderer::new(10, 1);
        let handle = renderer.enable_capture();

        renderer
            .buffer_mut()
            .write_str(0, 0, "Snap", &Style::default());
        let mut output = Vec::new();
        renderer.flush(&mut output).unwrap();

        let snapshot = handle.snapshot();
        assert!(snapshot.is_some());
        let snap = snapshot.unwrap();
        assert_eq!(snap.width(), 10);
        assert_eq!(snap.height(), 1);
    }

    #[test]
    fn test_capture_to_ansi() {
        let mut renderer = FrameRenderer::new(10, 1);
        let handle = renderer.enable_capture();

        renderer
            .buffer_mut()
            .write_str(0, 0, "Color", &Style::new().bold());
        let mut output = Vec::new();
        renderer.flush(&mut output).unwrap();

        let ansi = handle.to_ansi();
        assert!(ansi.is_some());
        let ansi_str = ansi.unwrap();
        // Should contain the text
        assert!(ansi_str.contains("Color"));
        // Should contain ANSI codes
        assert!(ansi_str.contains("\x1b["));
    }

    #[test]
    fn test_flush_second_render_only_diffs() {
        let mut renderer = FrameRenderer::new(5, 1);

        // First render: all cells get written
        renderer
            .buffer_mut()
            .write_str(0, 0, "Hello", &Style::default());
        let mut output1 = Vec::new();
        renderer.flush(&mut output1).unwrap();

        // Second render: same content - should be minimal output
        renderer
            .buffer_mut()
            .write_str(0, 0, "Hello", &Style::default());
        let mut output2 = Vec::new();
        renderer.flush(&mut output2).unwrap();

        // Second render should be shorter (only style set/reset, no content changes)
        assert!(
            output2.len() <= output1.len(),
            "Second render ({} bytes) should be <= first ({} bytes)",
            output2.len(),
            output1.len()
        );
    }

    #[test]
    fn test_flush_writes_all_command_types() {
        let mut renderer = FrameRenderer::new(20, 3);

        // Write content with different styles to exercise multiple code paths
        renderer
            .buffer_mut()
            .write_str(0, 0, "Normal", &Style::default());
        renderer
            .buffer_mut()
            .write_str(10, 0, "Bold", &Style::new().bold());
        renderer
            .buffer_mut()
            .write_str(0, 1, "Line2", &Style::default());

        let mut output = Vec::new();
        renderer.flush(&mut output).unwrap();

        let output_str = String::from_utf8_lossy(&output);
        // Should contain MoveTo sequences (ESC[row;colH)
        assert!(output_str.contains("\x1b["));
        assert!(output_str.contains("Normal"));
        assert!(output_str.contains("Bold"));
    }

    #[test]
    fn test_style_to_ansi_default_produces_escape() {
        let ansi = style_to_ansi(&Style::default());
        // Default style should produce some ANSI escape sequence
        assert!(ansi.starts_with("\x1b["), "Expected ANSI escape, got: {ansi:?}");
    }

    #[test]
    fn test_style_to_ansi_with_color() {
        use reovim_arch::Color;
        let style = Style::new().fg(Color::Red).bold();
        let ansi = style_to_ansi(&style);
        // Should contain ANSI escape
        assert!(ansi.starts_with("\x1b["));
    }

    #[test]
    fn test_write_command_clear_rect() {
        let renderer = FrameRenderer::new(10, 5);
        let mut output = Vec::new();
        let cmd = RenderCommand::ClearRect {
            x: 0,
            y: 0,
            width: 5,
            height: 3,
        };
        renderer.write_command(&mut output, &cmd).unwrap();
        let output_str = String::from_utf8_lossy(&output);
        // Should contain MoveTo and spaces for each row
        assert!(!output_str.is_empty());
    }

    #[test]
    fn test_write_command_show_hide_cursor() {
        let renderer = FrameRenderer::new(10, 5);
        let mut output = Vec::new();
        renderer
            .write_command(&mut output, &RenderCommand::ShowCursor)
            .unwrap();
        let output_str = String::from_utf8_lossy(&output);
        assert!(output_str.contains("\x1b[?25h"));

        let mut output2 = Vec::new();
        renderer
            .write_command(&mut output2, &RenderCommand::HideCursor)
            .unwrap();
        let output_str2 = String::from_utf8_lossy(&output2);
        assert!(output_str2.contains("\x1b[?25l"));
    }

    #[test]
    fn test_write_command_clear_line() {
        let renderer = FrameRenderer::new(10, 5);
        let mut output = Vec::new();
        renderer
            .write_command(&mut output, &RenderCommand::ClearLine)
            .unwrap();
        let output_str = String::from_utf8_lossy(&output);
        assert!(output_str.contains("\x1b[2K"));
    }

    #[test]
    fn test_write_command_clear_to_end() {
        let renderer = FrameRenderer::new(10, 5);
        let mut output = Vec::new();
        renderer
            .write_command(&mut output, &RenderCommand::ClearToEndOfLine)
            .unwrap();
        let output_str = String::from_utf8_lossy(&output);
        assert!(output_str.contains("\x1b[K"));
    }

    #[test]
    fn test_resize_forces_full_redraw() {
        let mut renderer = FrameRenderer::new(5, 1);

        // First flush initializes
        renderer
            .buffer_mut()
            .write_str(0, 0, "Hello", &Style::default());
        let mut output = Vec::new();
        renderer.flush(&mut output).unwrap();

        // Resize
        renderer.resize(10, 2);
        assert_eq!(renderer.dimensions(), (10, 2));

        // Next flush should re-render everything (initialized is false)
        renderer
            .buffer_mut()
            .write_str(0, 0, "World", &Style::default());
        let mut output2 = Vec::new();
        renderer.flush(&mut output2).unwrap();

        let output_str = String::from_utf8_lossy(&output2);
        assert!(output_str.contains("World"));
    }

    #[test]
    fn test_compute_diff_style_change_midline() {
        use reovim_arch::Color;
        let mut renderer = FrameRenderer::new(10, 1);

        // Write "Hello" in default + "World" in bold
        renderer
            .buffer_mut()
            .write_str(0, 0, "Hello", &Style::default());
        renderer
            .buffer_mut()
            .write_str(5, 0, "World", &Style::new().fg(Color::Red));

        let commands = renderer.compute_diff();

        // Should have multiple SetStyle commands (style change at position 5)
        let set_style_count = commands
            .iter()
            .filter(|c| matches!(c, RenderCommand::SetStyle(_)))
            .count();
        assert!(
            set_style_count >= 2,
            "Should have at least 2 SetStyle commands, got {set_style_count}"
        );
    }
}
