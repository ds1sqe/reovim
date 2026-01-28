//! Frame buffer and rendering infrastructure.
//!
//! This module provides the core rendering primitives for the TUI:
//! - `Cell` - A single character cell with style
//! - `FrameBuffer` - 2D grid of cells
//! - `FrameRenderer` - Double-buffer renderer with diff optimization

mod buffer;
mod cell;
mod renderer;

pub use {
    buffer::FrameBuffer,
    cell::{Cell, char_width},
    renderer::FrameRenderer,
};
