//! Unified rendering pipeline for buffer content
//!
//! This module provides types and traits for the composable render pipeline.
//! The pipeline transforms buffer content through multiple stages:
//! `Buffer` → `Visibility` → `Highlighting` → `Decorations` → `Visual` → `Indent` → `FrameBuffer`

mod registry;
mod stage;

pub use {registry::RenderStageRegistry, stage::RenderStage};

use crate::highlight::Style;

/// Data flowing through the render pipeline
#[derive(Debug, Clone)]
pub struct RenderData {
    /// Base content (lines of text)
    pub lines: Vec<String>,

    /// Per-line visibility state (for folding)
    pub visibility: Vec<LineVisibility>,

    /// Per-line highlights (syntax, search, etc.)
    pub highlights: Vec<Vec<LineHighlight>>,

    /// Per-line decorations (conceals, backgrounds)
    pub decorations: Vec<Vec<Decoration>>,

    /// Metadata
    pub buffer_id: usize,
    pub window_id: usize,
    pub window_bounds: Bounds,
}

impl RenderData {
    /// Create render data from a buffer
    #[must_use]
    pub fn from_buffer(
        window: &crate::screen::window::Window,
        buffer: &crate::buffer::Buffer,
    ) -> Self {
        let line_count = buffer.contents.len();

        Self {
            lines: buffer
                .contents
                .iter()
                .map(|line| line.inner.clone())
                .collect(),
            visibility: vec![LineVisibility::Visible; line_count],
            highlights: vec![Vec::new(); line_count],
            decorations: vec![Vec::new(); line_count],
            buffer_id: 0, // TODO: Get from window
            window_id: window.id,
            window_bounds: Bounds {
                x: window.anchor.x,
                y: window.anchor.y,
                width: window.width,
                height: window.height,
            },
        }
    }
}

/// Visibility state for a single line
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineVisibility {
    /// Line is visible
    Visible,
    /// Line is hidden (folded)
    Hidden,
    /// Line is a fold marker showing preview text
    FoldMarker {
        /// Preview text to display
        preview: String,
        /// Number of hidden lines
        hidden_lines: u32,
    },
}

/// Highlight span for a portion of a line
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineHighlight {
    /// Starting column (0-indexed)
    pub start_col: usize,
    /// Ending column (exclusive)
    pub end_col: usize,
    /// Style to apply
    pub style: Style,
}

/// Decoration for a portion of a line
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoration {
    /// Starting column (0-indexed)
    pub start_col: usize,
    /// Ending column (exclusive)
    pub end_col: usize,
    /// Type of decoration
    pub kind: DecorationKind,
}

/// Type of decoration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecorationKind {
    /// Conceal text with replacement
    Conceal { replacement: Option<String> },
    /// Background highlight
    Background { style: Style },
    /// Inline virtual text
    VirtualText { text: String, style: Style },
}

/// Bounding box for a window
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}
