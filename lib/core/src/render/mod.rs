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
    #[allow(clippy::cast_possible_truncation)]
    pub fn from_buffer(
        window: &crate::screen::window::Window,
        buffer: &crate::buffer::Buffer,
    ) -> Self {
        let line_count = buffer.contents.len();
        let content: String = buffer
            .contents
            .iter()
            .map(|line| line.inner.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        // Generate syntax highlights if syntax provider is attached
        let mut highlights: Vec<Vec<LineHighlight>> = vec![Vec::new(); line_count];
        if let Some(syntax) = buffer.syntax() {
            let all_highlights = syntax.highlight_range(&content, 0, line_count as u32);

            // Group highlights by line
            for hl in all_highlights {
                let line_idx = hl.span.start_line as usize;
                if line_idx < highlights.len() {
                    // For single-line highlights
                    if hl.span.start_line == hl.span.end_line {
                        highlights[line_idx].push(LineHighlight {
                            start_col: hl.span.start_col as usize,
                            end_col: hl.span.end_col as usize,
                            style: hl.style,
                        });
                    } else {
                        // Multi-line highlight: split across lines
                        let line_len = buffer
                            .contents
                            .get(line_idx)
                            .map_or(0, |l| l.inner.len());
                        highlights[line_idx].push(LineHighlight {
                            start_col: hl.span.start_col as usize,
                            end_col: line_len,
                            style: hl.style.clone(),
                        });

                        // Middle lines
                        for mid_line in (hl.span.start_line + 1)..hl.span.end_line {
                            let mid_idx = mid_line as usize;
                            if mid_idx < highlights.len() {
                                let mid_len = buffer
                                    .contents
                                    .get(mid_idx)
                                    .map_or(0, |l| l.inner.len());
                                highlights[mid_idx].push(LineHighlight {
                                    start_col: 0,
                                    end_col: mid_len,
                                    style: hl.style.clone(),
                                });
                            }
                        }

                        // End line
                        let end_idx = hl.span.end_line as usize;
                        if end_idx < highlights.len() {
                            highlights[end_idx].push(LineHighlight {
                                start_col: 0,
                                end_col: hl.span.end_col as usize,
                                style: hl.style,
                            });
                        }
                    }
                }
            }

            // Sort highlights by start column for each line
            for line_hl in &mut highlights {
                line_hl.sort_by_key(|h| h.start_col);
            }
        }

        Self {
            lines: buffer
                .contents
                .iter()
                .map(|line| line.inner.clone())
                .collect(),
            visibility: vec![LineVisibility::Visible; line_count],
            highlights,
            decorations: vec![Vec::new(); line_count],
            buffer_id: buffer.id,
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
