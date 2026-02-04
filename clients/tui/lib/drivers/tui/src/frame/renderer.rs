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
            self.write_command(writer, &cmd)?;
        }

        writer.flush()?;
        self.swap();
        self.initialized = true;
        Ok(())
    }

    /// Compute the diff between front and back buffers.
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

    #[allow(clippy::unused_self)]
    fn write_command(&self, writer: &mut dyn Write, cmd: &RenderCommand) -> io::Result<()> {
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
}
