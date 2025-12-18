//! Frame renderer coordinator

use {
    super::{
        DirtyRegions, FrameBuffer,
        strategy::{
            CellDeltaStrategy, DirtyRegionStrategy, RenderCommand, RenderStrategy,
            VirtualBufferStrategy,
        },
    },
    crate::{constants::RESET_STYLE, highlight::ColorMode},
    reovim_sys::{
        cursor::MoveTo,
        queue,
        style::Print,
        terminal::{Clear, ClearType},
    },
    std::io::Write,
};

/// Configuration for which render strategy to use
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RenderStrategyConfig {
    /// Full virtual buffer diff (default, most robust)
    #[default]
    VirtualBuffer,
    /// Dirty region tracking (requires explicit marking)
    DirtyRegion,
    /// Cell-level delta (hybrid approach)
    CellDelta,
}

impl RenderStrategyConfig {
    /// Parse from string (for config/CLI)
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "virtual_buffer" | "virtualbuffer" | "vb" => Some(Self::VirtualBuffer),
            "dirty_region" | "dirtyregion" | "dr" => Some(Self::DirtyRegion),
            "cell_delta" | "celldelta" | "cd" => Some(Self::CellDelta),
            _ => None,
        }
    }

    /// Get the name of this strategy
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::VirtualBuffer => "virtual_buffer",
            Self::DirtyRegion => "dirty_region",
            Self::CellDelta => "cell_delta",
        }
    }
}

/// The frame renderer manages double-buffering and strategy selection
pub struct FrameRenderer {
    /// Current (front) frame buffer
    current: FrameBuffer,
    /// Previous (back) frame buffer for diffing
    previous: Option<FrameBuffer>,
    /// Dirty regions for region-based strategy
    dirty_regions: DirtyRegions,
    /// Active render strategy config
    strategy_config: RenderStrategyConfig,
    /// Color mode for style conversion
    color_mode: ColorMode,
    /// Whether the first frame has been rendered
    initialized: bool,
}

impl FrameRenderer {
    /// Create a new frame renderer with given dimensions
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            current: FrameBuffer::new(width, height),
            previous: None,
            dirty_regions: DirtyRegions::new(),
            strategy_config: RenderStrategyConfig::default(),
            color_mode: ColorMode::TrueColor,
            initialized: false,
        }
    }

    /// Set the render strategy
    pub fn set_strategy(&mut self, strategy: RenderStrategyConfig) {
        self.strategy_config = strategy;

        // Allocate previous buffer if needed by strategy
        if self.get_strategy().requires_previous_frame() && self.previous.is_none() {
            self.previous = Some(FrameBuffer::new(self.current.width(), self.current.height()));
        }
    }

    /// Get the current strategy config
    #[must_use]
    pub const fn strategy(&self) -> RenderStrategyConfig {
        self.strategy_config
    }

    /// Set the color mode
    #[allow(clippy::missing_const_for_fn)] // setter pattern
    pub fn set_color_mode(&mut self, mode: ColorMode) {
        self.color_mode = mode;
    }

    /// Resize the frame buffer
    pub fn resize(&mut self, width: u16, height: u16) {
        self.current.resize(width, height);
        if let Some(ref mut prev) = self.previous {
            prev.resize(width, height);
        }
        self.dirty_regions.mark_all(width, height);
        self.initialized = false; // Force full redraw
    }

    /// Get the width
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // delegates to non-const method
    pub fn width(&self) -> u16 {
        self.current.width()
    }

    /// Get the height
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // delegates to non-const method
    pub fn height(&self) -> u16 {
        self.current.height()
    }

    /// Get mutable access to the current frame buffer for rendering
    #[allow(clippy::missing_const_for_fn)] // returns mutable ref
    pub fn buffer_mut(&mut self) -> &mut FrameBuffer {
        &mut self.current
    }

    /// Get read-only access to the current buffer
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // returns ref
    pub fn buffer(&self) -> &FrameBuffer {
        &self.current
    }

    /// Mark a region as dirty
    pub fn mark_dirty(&mut self, x: u16, y: u16, width: u16, height: u16) {
        self.dirty_regions.mark_rect(x, y, width, height);
    }

    /// Mark a single row as dirty
    pub fn mark_row_dirty(&mut self, y: u16) {
        self.dirty_regions.mark_row(y, self.current.width());
    }

    /// Mark entire screen as dirty (forces full redraw)
    pub fn mark_all_dirty(&mut self) {
        self.dirty_regions
            .mark_all(self.current.width(), self.current.height());
    }

    /// Clear the frame buffer (fills with empty cells)
    pub fn clear(&mut self) {
        self.current.clear();
        self.mark_all_dirty();
    }

    /// Flush the frame to the terminal writer
    ///
    /// This computes the diff based on the selected strategy and
    /// writes only the necessary commands to update the display.
    ///
    /// # Errors
    /// Returns an error if writing to the terminal fails.
    pub fn flush<W: Write>(&mut self, writer: &mut W) -> std::io::Result<()> {
        let strategy = self.get_strategy();

        let commands = if self.initialized {
            strategy.compute_commands(
                &self.current,
                self.previous.as_ref(),
                Some(&self.dirty_regions),
            )
        } else {
            // First render - always full
            self.initialized = true;
            strategy.compute_commands(&self.current, None, None)
        };

        // Execute commands
        Self::execute_commands(writer, &commands)?;

        // Update previous buffer for next frame comparison
        if strategy.requires_previous_frame()
            && let Some(ref mut prev) = self.previous
        {
            prev.copy_from(&self.current);
        }

        // Clear dirty tracking
        self.dirty_regions.clear();

        writer.flush()
    }

    /// Force a full redraw on next flush
    pub fn invalidate(&mut self) {
        self.initialized = false;
        self.mark_all_dirty();
    }

    /// Get the strategy instance based on config
    fn get_strategy(&self) -> Box<dyn RenderStrategy> {
        match self.strategy_config {
            RenderStrategyConfig::VirtualBuffer => {
                Box::new(VirtualBufferStrategy::new(self.color_mode))
            }
            RenderStrategyConfig::DirtyRegion => {
                Box::new(DirtyRegionStrategy::new(self.color_mode))
            }
            RenderStrategyConfig::CellDelta => Box::new(CellDeltaStrategy::new(self.color_mode)),
        }
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
        assert_eq!(renderer.strategy(), RenderStrategyConfig::VirtualBuffer);
    }

    #[test]
    fn test_strategy_config_parse() {
        assert_eq!(
            RenderStrategyConfig::parse("virtual_buffer"),
            Some(RenderStrategyConfig::VirtualBuffer)
        );
        assert_eq!(
            RenderStrategyConfig::parse("dirty_region"),
            Some(RenderStrategyConfig::DirtyRegion)
        );
        assert_eq!(
            RenderStrategyConfig::parse("cell_delta"),
            Some(RenderStrategyConfig::CellDelta)
        );
        assert_eq!(RenderStrategyConfig::parse("invalid"), None);
    }

    #[test]
    fn test_set_strategy() {
        let mut renderer = FrameRenderer::new(80, 24);
        renderer.set_strategy(RenderStrategyConfig::DirtyRegion);
        assert_eq!(renderer.strategy(), RenderStrategyConfig::DirtyRegion);
    }

    #[test]
    fn test_buffer_access() {
        let mut renderer = FrameRenderer::new(10, 5);
        renderer.buffer_mut().set(0, 0, Cell::from_char('X'));
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
}
