//! Unified rendering pipeline for buffer content
//!
//! This module provides types and traits for the composable render pipeline.
//! The pipeline transforms buffer content through multiple stages:
//! `Buffer` → `Visibility` → `Highlighting` → `Decorations` → `Visual` → `Indent` → `FrameBuffer`

mod decoration_cache;
mod highlight_cache;
mod registry;
mod stage;

pub use {
    decoration_cache::DecorationCache, highlight_cache::HighlightCache,
    registry::RenderStageRegistry, stage::RenderStage,
};

use crate::{highlight::Style, sign::LineSign};

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

    /// Per-line signs for gutter (same length as lines)
    /// Populated by plugins via `RenderStage` transforms
    pub signs: Vec<LineSign>,

    /// Metadata
    pub buffer_id: usize,
    pub window_id: usize,
    pub window_bounds: Bounds,

    /// Cursor position (line, column) for bracket matching etc.
    pub cursor: (usize, usize),
}

impl RenderData {
    /// Create render data from a buffer
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::too_many_lines)]
    pub fn from_buffer(
        window: &crate::screen::window::Window,
        buffer: &crate::buffer::Buffer,
        mode: &crate::modd::ModeState,
    ) -> Self {
        let line_count = buffer.contents.len();

        // READ ONLY: Get highlights from cache
        // Saturator (update_highlights) has already computed and cached them
        // If empty, just render without syntax - that's fine, still fast
        let hl_start = std::time::Instant::now();
        let highlights = buffer.highlight_cache.get_ready_highlights(line_count);
        tracing::trace!(
            "[RTT] from_buffer: highlight_cache_read={:?} lines={}",
            hl_start.elapsed(),
            line_count
        );

        // READ ONLY: Get decorations from cache
        // Saturator (update_decorations) has already computed and cached them
        // If empty, just render without decorations - that's fine, still fast
        let deco_start = std::time::Instant::now();
        let mut decorations = buffer.decoration_cache.get_ready_decorations(line_count);

        // In insert mode, disable decorations on cursor line to show raw syntax
        if mode.is_insert() {
            let cursor_line = buffer.cur.y as usize;
            if cursor_line < decorations.len() {
                decorations[cursor_line].clear();
            }
        }

        tracing::trace!(
            "[RTT] from_buffer: decoration_cache_read={:?} lines={} insert_mode={}",
            deco_start.elapsed(),
            line_count,
            mode.is_insert()
        );

        Self {
            lines: buffer
                .contents
                .iter()
                .map(|line| line.inner.clone())
                .collect(),
            visibility: vec![LineVisibility::Visible; line_count],
            highlights,
            decorations,
            signs: vec![None; line_count],
            buffer_id: buffer.id,
            window_id: window.id,
            window_bounds: Bounds {
                x: window.anchor.x,
                y: window.anchor.y,
                width: window.width,
                height: window.height,
            },
            cursor: (buffer.cur.y as usize, buffer.cur.x as usize),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_data_signs_initialization() {
        // Create minimal test data
        use crate::{buffer::Buffer, modd::ModeState};

        let buffer = Buffer::empty(0);
        let mode = ModeState::default();
        let window = crate::screen::window::tests::create_test_window(10);

        let render_data = RenderData::from_buffer(&window, &buffer, &mode);

        // Signs should be initialized as empty vector with correct length
        assert_eq!(render_data.signs.len(), buffer.contents.len());
        assert!(render_data.signs.iter().all(Option::is_none));
    }

    #[test]
    fn test_render_data_signs_can_be_modified() {
        use crate::{
            buffer::{Buffer, Line},
            highlight::Style,
            modd::ModeState,
            sign::Sign,
        };

        let mut buffer = Buffer::empty(0);
        // Add some lines to the buffer so we can test sign modification
        buffer.contents.push(Line::from("line 1"));
        buffer.contents.push(Line::from("line 2"));
        buffer.contents.push(Line::from("line 3"));

        let mode = ModeState::default();
        let window = crate::screen::window::tests::create_test_window(10);

        let mut render_data = RenderData::from_buffer(&window, &buffer, &mode);

        // Add a sign to line 0
        let sign = Sign::new("●".to_string(), Style::new(), 304);
        render_data.signs[0] = Some(sign);

        // Verify it was set
        assert!(render_data.signs[0].is_some());
        assert_eq!(render_data.signs[0].as_ref().unwrap().icon, "●");
        assert_eq!(render_data.signs[0].as_ref().unwrap().priority, 304);
    }
}
