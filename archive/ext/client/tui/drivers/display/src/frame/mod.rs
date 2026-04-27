//! Frame module for display rendering.
//!
//! This module provides the core types for terminal-based rendering:
//!
//! - [`Cell`] - A single character cell with styling
//! - [`FrameBuffer`] - 2D grid of cells
//! - [`FrameRenderer`] - Double-buffer renderer with diff
//! - [`FrameBufferHandle`] - Thread-safe capture for RPC
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                    FrameRenderer                        │
//! │  ┌──────────────────┐    ┌──────────────────┐          │
//! │  │   Back Buffer    │    │   Front Buffer   │          │
//! │  │  (being drawn)   │───▶│   (displayed)    │          │
//! │  └──────────────────┘    └──────────────────┘          │
//! │           │                      │                      │
//! │           │     compute_diff()   │                      │
//! │           └──────────┬───────────┘                      │
//! │                      ▼                                  │
//! │             RenderCommands[]                            │
//! │                      │                                  │
//! │                      ▼                                  │
//! │               Terminal Output                           │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_display::frame::{FrameRenderer, Cell};
//!
//! let mut renderer = FrameRenderer::new(80, 24);
//!
//! // Draw to back buffer
//! renderer.buffer_mut().write_str(0, 0, "Hello, World!", &Style::default());
//!
//! // Flush to terminal (only changed cells)
//! renderer.flush(&mut stdout)?;
//! ```

mod buffer;
mod cell;
mod renderer;

pub use {
    buffer::FrameBuffer,
    cell::{Cell, char_width},
    renderer::{FrameBufferHandle, FrameRenderer},
};
