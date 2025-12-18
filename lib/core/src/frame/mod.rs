//! Frame buffer abstraction for flicker-free rendering
//!
//! This module provides a virtual frame buffer that sits between logical
//! rendering and terminal output, enabling differential updates to eliminate
//! screen jittering.

mod buffer;
mod cell;
mod dirty;
mod renderer;
pub mod strategy;

pub use {
    buffer::FrameBuffer,
    cell::Cell,
    dirty::{DirtyCells, DirtyRect, DirtyRegions},
    renderer::{FrameRenderer, RenderStrategyConfig},
};
