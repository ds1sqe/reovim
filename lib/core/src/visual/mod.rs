//! Visual representation of TUI state
//!
//! This module provides multiple formats for representing the screen:
//! - ASCII art for human debugging
//! - Structured snapshots for programmatic testing
//! - JSON serialization for external tools (like Claude)

mod ascii;
mod snapshot;

pub use {
    ascii::{AsciiRenderConfig, RegionAnnotation},
    snapshot::{BoundsInfo, CursorInfo, LayerInfo, VisualSnapshot},
};
