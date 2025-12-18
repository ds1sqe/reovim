//! Compositor for z-layer based rendering
//!
//! The compositor collects all layers, sorts them by z-order,
//! renders them to a frame buffer, and outputs only changed cells.

use std::io::Write;

use crate::{
    constants::RESET_STYLE,
    frame::{
        FrameBuffer,
        strategy::{RenderStrategy, VirtualBufferStrategy},
    },
    highlight::{ColorMode, Theme},
};

use super::layer::Layer;

/// Compositor that manages z-ordered layer rendering
///
/// Layers are rendered to a virtual buffer in z-order (low to high).
/// Higher z-order layers occlude lower ones. After composition,
/// only changed cells are output to the terminal.
pub struct Compositor {
    /// Current frame buffer
    buffer: FrameBuffer,
    /// Previous frame for diffing
    prev_buffer: FrameBuffer,
    /// Whether first frame has been rendered
    initialized: bool,
    /// Color mode for style conversion
    color_mode: ColorMode,
}

impl Compositor {
    /// Create a new compositor with given dimensions
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            buffer: FrameBuffer::new(width, height),
            prev_buffer: FrameBuffer::new(width, height),
            initialized: false,
            color_mode: ColorMode::TrueColor,
        }
    }

    /// Set the color mode
    pub const fn set_color_mode(&mut self, color_mode: ColorMode) {
        self.color_mode = color_mode;
    }

    /// Resize the compositor buffers
    pub fn resize(&mut self, width: u16, height: u16) {
        self.buffer.resize(width, height);
        self.prev_buffer.resize(width, height);
        // Force full redraw after resize
        self.initialized = false;
    }

    /// Compose all layers and flush to output
    ///
    /// Layers are rendered in z-order (sorted by `z_order()`).
    /// Returns the cursor position from the highest layer that specifies one.
    pub fn compose_and_flush<W: Write>(
        &mut self,
        layers: &mut [&dyn Layer],
        theme: &Theme,
        out: &mut W,
    ) -> std::io::Result<(u16, u16)> {
        // Clear the buffer
        self.buffer.clear();

        // Sort layers by z-order (stable sort preserves insertion order for equal z)
        layers.sort_by_key(|l| l.z_order());

        // Render each visible layer (higher z overwrites lower)
        let mut cursor_pos = (0u16, 0u16);
        for layer in layers.iter() {
            if layer.is_visible() {
                layer.render_to_buffer(&mut self.buffer, theme, self.color_mode);
                if let Some(pos) = layer.cursor_position() {
                    cursor_pos = pos;
                }
            }
        }

        // Compute diff and output
        let strategy = VirtualBufferStrategy::new(self.color_mode);
        let prev = if self.initialized {
            Some(&self.prev_buffer)
        } else {
            None
        };

        let commands = strategy.compute_commands(&self.buffer, prev, None);

        // Execute render commands
        for cmd in commands {
            Self::execute_command(cmd, out)?;
        }

        // Swap buffers
        std::mem::swap(&mut self.buffer, &mut self.prev_buffer);
        self.initialized = true;

        Ok(cursor_pos)
    }

    /// Execute a single render command
    fn execute_command<W: Write>(
        cmd: crate::frame::strategy::RenderCommand,
        out: &mut W,
    ) -> std::io::Result<()> {
        use {
            crate::frame::strategy::RenderCommand,
            reovim_sys::{
                cursor::MoveTo,
                queue,
                style::Print,
                terminal::{Clear, ClearType},
            },
        };

        match cmd {
            RenderCommand::MoveTo(x, y) => {
                queue!(out, MoveTo(x, y))?;
            }
            RenderCommand::Print(s) => {
                queue!(out, Print(s))?;
            }
            RenderCommand::SetStyle(s) => {
                queue!(out, Print(s))?;
            }
            RenderCommand::ResetStyle => {
                queue!(out, Print(RESET_STYLE))?;
            }
            RenderCommand::ClearToEndOfLine => {
                queue!(out, Clear(ClearType::UntilNewLine))?;
            }
        }
        Ok(())
    }

    /// Get buffer dimensions
    #[must_use]
    pub const fn size(&self) -> (u16, u16) {
        (self.buffer.width(), self.buffer.height())
    }

    /// Get mutable reference to current buffer (for direct rendering)
    pub const fn buffer_mut(&mut self) -> &mut FrameBuffer {
        &mut self.buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compositor_new() {
        let comp = Compositor::new(80, 24);
        assert_eq!(comp.size(), (80, 24));
    }

    #[test]
    fn test_compositor_resize() {
        let mut comp = Compositor::new(80, 24);
        comp.resize(120, 40);
        assert_eq!(comp.size(), (120, 40));
    }
}
