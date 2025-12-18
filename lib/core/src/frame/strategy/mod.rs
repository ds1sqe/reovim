//! Render strategies for frame buffer output

mod cell_delta;
mod dirty_region;
mod virtual_buffer;

pub use {
    cell_delta::CellDeltaStrategy, dirty_region::DirtyRegionStrategy,
    virtual_buffer::VirtualBufferStrategy,
};

use super::{DirtyRegions, FrameBuffer};

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

/// Trait for render strategies
///
/// Each strategy determines how to compute the minimal set of terminal
/// commands needed to update the display from the current frame.
pub trait RenderStrategy: Send + Sync {
    /// Name of this strategy (for debugging/logging)
    fn name(&self) -> &'static str;

    /// Whether this strategy requires keeping a previous frame copy
    fn requires_previous_frame(&self) -> bool;

    /// Compute the commands needed to update the display
    ///
    /// # Arguments
    /// - `current`: The new frame to display
    /// - `previous`: The frame currently on screen (None for first render)
    /// - `dirty_regions`: Optional region hints from the renderer
    fn compute_commands(
        &self,
        current: &FrameBuffer,
        previous: Option<&FrameBuffer>,
        dirty_regions: Option<&DirtyRegions>,
    ) -> Vec<RenderCommand>;
}
