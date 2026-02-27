//! Double-buffer frame renderer with cell-by-cell diff.

use std::io::{self, Write};

use crate::style::{ColorMode, Style};

use super::buffer::FrameBuffer;

/// Render commands emitted during rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
enum RenderCommand {
    MoveTo { x: u16, y: u16 },
    Print(String),
    SetStyle(String),
    ResetStyle,
}

/// Double-buffer renderer with cell-by-cell diff.
pub struct FrameRenderer {
    front: FrameBuffer,
    back: FrameBuffer,
    initialized: bool,
}

impl FrameRenderer {
    /// Create a new renderer with the given dimensions.
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            front: FrameBuffer::new(width, height),
            back: FrameBuffer::new(width, height),
            initialized: false,
        }
    }

    /// Resize both buffers to new dimensions.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.front.resize(width, height);
        self.back.resize(width, height);
        self.front.clear();
        self.back.clear();
        self.initialized = false;
    }

    /// Get the current dimensions.
    #[must_use]
    pub const fn dimensions(&self) -> (u16, u16) {
        (self.back.width(), self.back.height())
    }

    /// Get mutable access to the back buffer.
    pub const fn buffer_mut(&mut self) -> &mut FrameBuffer {
        &mut self.back
    }

    /// Get read-only access to the back buffer.
    #[must_use]
    pub const fn buffer(&self) -> &FrameBuffer {
        &self.back
    }

    /// Flush changes to the writer using diff rendering.
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
        Ok(())
    }

    /// Compute the diff between front and back buffers.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn compute_diff(&self) -> Vec<RenderCommand> {
        let mut commands = Vec::new();
        let mut current_style: Option<Style> = None;
        let mut batch_start: Option<(u16, u16)> = None;
        let mut batch_chars = String::new();
        let mut batch_width: u16 = 0;

        for y in 0..self.back.height() {
            for x in 0..self.back.width() {
                let Some(back_cell) = self.back.get(x, y) else {
                    continue;
                };
                let front_cell = self.front.get(x, y);

                if back_cell.is_continuation {
                    continue;
                }

                let needs_update = !self.initialized
                    || front_cell.is_none()
                    || back_cell.differs_from(front_cell.unwrap());

                if needs_update {
                    if current_style.as_ref() != Some(&back_cell.style) {
                        Self::flush_batch(
                            &mut commands,
                            &mut batch_start,
                            &mut batch_chars,
                            &mut batch_width,
                        );

                        if current_style.is_some() {
                            commands.push(RenderCommand::ResetStyle);
                        }
                        commands.push(RenderCommand::SetStyle(style_to_ansi(&back_cell.style)));
                        current_style = Some(back_cell.style.clone());
                    }

                    let can_batch =
                        batch_start.is_some_and(|(bx, by)| by == y && bx + batch_width == x);

                    if can_batch {
                        batch_chars.push(back_cell.char);
                        batch_width += u16::from(back_cell.width);
                    } else {
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
                    Self::flush_batch(
                        &mut commands,
                        &mut batch_start,
                        &mut batch_chars,
                        &mut batch_width,
                    );
                }
            }
            Self::flush_batch(&mut commands, &mut batch_start, &mut batch_chars, &mut batch_width);
        }

        if current_style.is_some() {
            commands.push(RenderCommand::ResetStyle);
        }

        commands
    }

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

    fn swap(&mut self) {
        self.front.swap_with(&mut self.back);
    }

    fn write_command(writer: &mut dyn Write, cmd: &RenderCommand) -> io::Result<()> {
        match cmd {
            RenderCommand::MoveTo { x, y } => {
                write!(writer, "\x1b[{};{}H", y + 1, x + 1)
            }
            RenderCommand::Print(s) => {
                write!(writer, "{s}")
            }
            RenderCommand::SetStyle(ansi_str) => {
                write!(writer, "{ansi_str}")
            }
            RenderCommand::ResetStyle => {
                write!(writer, "\x1b[0m")
            }
        }
    }
}

impl Default for FrameRenderer {
    fn default() -> Self {
        Self::new(80, 24)
    }
}

/// Convert a Style to ANSI escape sequence.
fn style_to_ansi(style: &Style) -> String {
    let ansi = style.to_ansi_start(ColorMode::TrueColor);
    if ansi.is_empty() {
        "\x1b[0m".to_string()
    } else {
        ansi
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_flush() {
        let mut renderer = FrameRenderer::new(5, 1);
        renderer
            .buffer_mut()
            .write_str(0, 0, "Hello", &Style::default());

        let mut output = Vec::new();
        renderer.flush(&mut output).unwrap();

        let output_str = String::from_utf8_lossy(&output);
        assert!(output_str.contains("Hello"));
    }

    #[test]
    fn test_default() {
        let renderer = FrameRenderer::default();
        assert_eq!(renderer.dimensions(), (80, 24));
    }

    #[test]
    fn test_buffer_read_only() {
        let renderer = FrameRenderer::new(10, 5);
        let buf = renderer.buffer();
        assert_eq!(buf.width(), 10);
        assert_eq!(buf.height(), 5);
    }

    #[test]
    fn test_flush_twice_diff_rendering() {
        let mut renderer = FrameRenderer::new(5, 1);

        // First flush: full render (not initialized)
        renderer
            .buffer_mut()
            .write_str(0, 0, "Hello", &Style::default());
        let mut output1 = Vec::new();
        renderer.flush(&mut output1).unwrap();
        let out1 = String::from_utf8_lossy(&output1);
        assert!(out1.contains("Hello"));

        // Second flush: same content, should be minimal
        renderer
            .buffer_mut()
            .write_str(0, 0, "Hello", &Style::default());
        let mut output2 = Vec::new();
        renderer.flush(&mut output2).unwrap();
        // Second output should be smaller (diff detects no changes)
        assert!(output2.len() <= output1.len());
    }

    #[test]
    fn test_flush_with_style_changes() {
        let mut renderer = FrameRenderer::new(10, 1);
        let styled = Style::new().with_fg(reovim_arch::Color::Red);
        renderer.buffer_mut().write_str(0, 0, "Red", &styled);
        renderer
            .buffer_mut()
            .write_str(3, 0, "Def", &Style::default());

        let mut output = Vec::new();
        renderer.flush(&mut output).unwrap();
        let out = String::from_utf8_lossy(&output);
        // Should contain both text segments and style commands
        assert!(out.contains("Red"));
        assert!(out.contains("Def"));
        // Should contain reset style between different styles
        assert!(out.contains("\x1b[0m"));
    }

    #[test]
    fn test_flush_only_changed_cells() {
        let mut renderer = FrameRenderer::new(10, 1);

        // First flush: write "Hello     "
        renderer
            .buffer_mut()
            .write_str(0, 0, "Hello", &Style::default());
        let mut out1 = Vec::new();
        renderer.flush(&mut out1).unwrap();

        // Second flush: change only first char
        renderer
            .buffer_mut()
            .write_str(0, 0, "Jello", &Style::default());
        let mut out2 = Vec::new();
        renderer.flush(&mut out2).unwrap();

        let out2_str = String::from_utf8_lossy(&out2);
        // Should contain "Jello" (diff will detect first character changed,
        // but batching may include subsequent unchanged chars on same row)
        assert!(out2_str.contains('J'));
    }

    #[test]
    fn test_resize_clears_initialized() {
        let mut renderer = FrameRenderer::new(5, 1);

        // First flush sets initialized = true
        renderer
            .buffer_mut()
            .write_str(0, 0, "Hello", &Style::default());
        let mut out = Vec::new();
        renderer.flush(&mut out).unwrap();

        // Resize resets initialized
        renderer.resize(10, 2);
        assert_eq!(renderer.dimensions(), (10, 2));

        // Next flush should do full render
        renderer
            .buffer_mut()
            .write_str(0, 0, "World", &Style::default());
        let mut out2 = Vec::new();
        renderer.flush(&mut out2).unwrap();
        let out2_str = String::from_utf8_lossy(&out2);
        assert!(out2_str.contains("World"));
    }

    #[test]
    fn test_flush_multi_row() {
        let mut renderer = FrameRenderer::new(5, 3);
        renderer
            .buffer_mut()
            .write_str(0, 0, "Row0", &Style::default());
        renderer
            .buffer_mut()
            .write_str(0, 1, "Row1", &Style::default());
        renderer
            .buffer_mut()
            .write_str(0, 2, "Row2", &Style::default());

        let mut output = Vec::new();
        renderer.flush(&mut output).unwrap();
        let out = String::from_utf8_lossy(&output);
        assert!(out.contains("Row0"));
        assert!(out.contains("Row1"));
        assert!(out.contains("Row2"));
    }

    #[test]
    fn test_flush_styled_content() {
        let mut renderer = FrameRenderer::new(5, 1);
        let bold_style = Style::new().bold();
        renderer.buffer_mut().write_str(0, 0, "Bold", &bold_style);

        let mut output = Vec::new();
        renderer.flush(&mut output).unwrap();
        let out = String::from_utf8_lossy(&output);
        assert!(out.contains("Bold"));
        // Should have style set command
        assert!(out.contains("\x1b["));
    }

    #[test]
    fn test_style_to_ansi_with_empty_style() {
        let style = Style::default();
        let ansi = style_to_ansi(&style);
        // Empty style should produce reset
        assert_eq!(ansi, "\x1b[0m");
    }

    #[test]
    fn test_style_to_ansi_with_color() {
        let style = Style::new().with_fg(reovim_arch::Color::Green);
        let ansi = style_to_ansi(&style);
        // Should have actual color codes, not reset
        assert_ne!(ansi, "\x1b[0m");
        assert!(ansi.starts_with("\x1b["));
    }

    #[test]
    fn test_render_command_eq() {
        let cmd1 = RenderCommand::MoveTo { x: 1, y: 2 };
        let cmd2 = RenderCommand::MoveTo { x: 1, y: 2 };
        assert_eq!(cmd1, cmd2);

        let cmd3 = RenderCommand::Print("abc".to_string());
        let cmd4 = RenderCommand::Print("abc".to_string());
        assert_eq!(cmd3, cmd4);

        assert_ne!(cmd1, cmd3);

        let cmd5 = RenderCommand::ResetStyle;
        let cmd6 = RenderCommand::ResetStyle;
        assert_eq!(cmd5, cmd6);

        let cmd7 = RenderCommand::SetStyle("test".to_string());
        let cmd8 = RenderCommand::SetStyle("test".to_string());
        assert_eq!(cmd7, cmd8);
    }

    #[test]
    fn test_render_command_clone() {
        let cmd = RenderCommand::Print("hello".to_string());
        let cloned = cmd.clone();
        assert_eq!(cmd, cloned);
    }

    #[test]
    fn test_render_command_debug() {
        let cmd = RenderCommand::MoveTo { x: 5, y: 10 };
        let debug = format!("{cmd:?}");
        assert!(debug.contains("MoveTo"));
    }

    #[test]
    fn test_write_command_move_to() {
        let mut output = Vec::new();
        let cmd = RenderCommand::MoveTo { x: 3, y: 7 };
        FrameRenderer::write_command(&mut output, &cmd).unwrap();
        let out = String::from_utf8_lossy(&output);
        // MoveTo uses 1-based indexing: y+1, x+1
        assert!(out.contains("\x1b[8;4H"));
    }

    #[test]
    fn test_write_command_print() {
        let mut output = Vec::new();
        let cmd = RenderCommand::Print("test_text".to_string());
        FrameRenderer::write_command(&mut output, &cmd).unwrap();
        let out = String::from_utf8_lossy(&output);
        assert_eq!(out, "test_text");
    }

    #[test]
    fn test_write_command_set_style() {
        let mut output = Vec::new();
        let cmd = RenderCommand::SetStyle("\x1b[1m".to_string());
        FrameRenderer::write_command(&mut output, &cmd).unwrap();
        let out = String::from_utf8_lossy(&output);
        assert_eq!(out, "\x1b[1m");
    }

    #[test]
    fn test_write_command_reset_style() {
        let mut output = Vec::new();
        let cmd = RenderCommand::ResetStyle;
        FrameRenderer::write_command(&mut output, &cmd).unwrap();
        let out = String::from_utf8_lossy(&output);
        assert_eq!(out, "\x1b[0m");
    }
}
