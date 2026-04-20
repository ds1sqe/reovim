//! Render pipeline and utilities.
//!
//! This module provides the render pipeline framework and utilities
//! for rendering window content to a frame buffer.
//!
//! # Components
//!
//! - [`line`]: Line rendering with syntax highlighting
//! - [`chrome`]: Chrome rendering (tab line, status line)
//! - [`separator`]: Window separator rendering
//! - [`pipeline`]: Render pipeline framework
//!
//! # Architecture
//!
//! The render pipeline processes window content through stages:
//!
//! ```text
//! ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
//! │   Buffer    │────▶│  Pipeline   │────▶│ FrameBuffer │
//! │  (content)  │     │  (stages)   │     │  (cells)    │
//! └─────────────┘     └─────────────┘     └─────────────┘
//!                           │
//!                     ┌─────┴─────┐
//!                     ▼           ▼
//!               RenderData   RenderContext
//! ```

mod chrome;
mod line;
mod pipeline;
mod separator;

// Line rendering
pub use line::{render_line, render_line_simple};

// Chrome rendering (tab line, status line)
pub use chrome::{
    TabInfo, TablineStyles, render_statusline, render_statusline_simple, render_tabline,
};

// Separator rendering
pub use separator::{
    SeparatorChars, render_grid_separators, render_hseparator, render_intersection,
    render_vseparator,
};

// Pipeline framework
pub use pipeline::{
    GutterDecoration, InlineDecoration, RenderContext, RenderData, RenderStage, execute_pipeline,
    execute_pipeline_sorted,
};
