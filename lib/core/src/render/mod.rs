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

use std::collections::{BTreeMap, HashSet};

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

    /// Per-line virtual text for end-of-line display (same length as lines)
    /// Populated by plugins via `RenderStage` transforms (e.g., LSP diagnostics)
    pub virtual_texts: Vec<Option<VirtualTextEntry>>,

    /// Virtual lines to render above/below buffer lines
    /// Key: (`line_index`, position), Value: virtual line entry
    /// Used for table borders, decorative separators, etc.
    pub virtual_lines: BTreeMap<(usize, VirtualLinePosition), VirtualLineEntry>,

    /// Metadata
    pub buffer_id: usize,
    pub window_id: usize,
    pub window_bounds: Bounds,

    /// Cursor position (line, column) for bracket matching etc.
    pub cursor: (usize, usize),

    /// Lines to skip decorations on (insert mode cursor line, visual selection lines)
    /// Stages should check this set and avoid adding decorations to these lines
    pub skip_decoration_lines: HashSet<usize>,
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

        // Build set of lines to skip decorations on
        let mut skip_decoration_lines = HashSet::new();

        // In insert mode, skip cursor line
        if mode.is_insert() {
            skip_decoration_lines.insert(buffer.cur.y as usize);
        }

        // In visual mode, skip ALL selected lines
        if mode.is_visual() && buffer.selection.active {
            use crate::buffer::SelectionOps;
            let (start, end) = buffer.selection_bounds();
            for line_idx in start.y as usize..=end.y as usize {
                skip_decoration_lines.insert(line_idx);
            }
        }

        // Clear cached decorations for skipped lines
        for &line_idx in &skip_decoration_lines {
            if line_idx < decorations.len() {
                decorations[line_idx].clear();
            }
        }

        tracing::trace!(
            "[RTT] from_buffer: decoration_cache_read={:?} lines={} skip_lines={}",
            deco_start.elapsed(),
            line_count,
            skip_decoration_lines.len()
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
            virtual_texts: vec![None; line_count],
            virtual_lines: BTreeMap::new(),
            buffer_id: buffer.id,
            window_id: window.id.raw(),
            window_bounds: Bounds {
                x: window.bounds.x,
                y: window.bounds.y,
                width: window.bounds.width,
                height: window.bounds.height,
            },
            cursor: (buffer.cur.y as usize, buffer.cur.x as usize),
            skip_decoration_lines,
        }
    }

    /// Get column mapping for cursor line if a full-line Conceal exists with mapping
    ///
    /// Returns the column mapping (`buffer_col` → `visual_col`) if the cursor line has
    /// a full-line Conceal decoration with a column mapping. This is used for
    /// positioning the cursor correctly when decorations change column positions.
    #[must_use]
    pub fn cursor_col_mapping(&self) -> Option<&[u16]> {
        let cursor_line = self.cursor.0;
        let line_len = self.lines.get(cursor_line)?.len();
        self.decorations.get(cursor_line)?.iter().find_map(|deco| {
            // Check for full-line conceal with column mapping
            if deco.start_col == 0
                && deco.end_col >= line_len
                && let DecorationKind::Conceal {
                    col_mapping: Some(m),
                    ..
                } = &deco.kind
            {
                return Some(m.as_slice());
            }
            None
        })
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
    Conceal {
        replacement: Option<String>,
        /// Optional column mapping for cursor position (`buffer_col` → `visual_col`)
        /// When present, maps each buffer column index to its visual column in the replacement
        /// Used for full-line conceals that change column positions (e.g., table expansion)
        col_mapping: Option<Vec<u16>>,
    },
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

/// Virtual text entry for end-of-line display
///
/// Used to display inline information after line content, such as:
/// - LSP diagnostic messages
/// - Type hints
/// - Git blame
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualTextEntry {
    /// The text to display
    pub text: String,
    /// Style for rendering
    pub style: Style,
    /// Priority (higher wins when multiple on same line)
    pub priority: u32,
}

/// Position for virtual line relative to a buffer line
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VirtualLinePosition {
    /// Insert above the buffer line
    Before,
    /// Insert below the buffer line
    After,
}

/// A virtual line entry (entire line not in buffer)
///
/// Used for rendering decorative elements that span full lines:
/// - Table top/bottom borders (┌───┬───┐, └───┴───┘)
/// - Section separators
/// - Fold previews
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualLineEntry {
    /// Text content of the virtual line
    pub text: String,
    /// Style for rendering
    pub style: Style,
    /// Priority (higher wins on conflict at same position)
    pub priority: u32,
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

    #[test]
    fn test_virtual_text_entry_creation() {
        let vt = VirtualTextEntry {
            text: "● error message".to_string(),
            style: Style::new(),
            priority: 404,
        };

        assert_eq!(vt.text, "● error message");
        assert_eq!(vt.priority, 404);
    }

    #[test]
    fn test_render_data_virtual_texts_initialization() {
        use crate::{buffer::Buffer, modd::ModeState};

        let buffer = Buffer::empty(0);
        let mode = ModeState::default();
        let window = crate::screen::window::tests::create_test_window(10);

        let render_data = RenderData::from_buffer(&window, &buffer, &mode);

        // Virtual texts should be initialized as empty vector with correct length
        assert_eq!(render_data.virtual_texts.len(), buffer.contents.len());
        assert!(render_data.virtual_texts.iter().all(Option::is_none));
    }

    #[test]
    fn test_render_data_virtual_texts_can_be_modified() {
        use crate::{
            buffer::{Buffer, Line},
            highlight::Style,
            modd::ModeState,
        };

        let mut buffer = Buffer::empty(0);
        buffer.contents.push(Line::from("line 1"));
        buffer.contents.push(Line::from("line 2"));

        let mode = ModeState::default();
        let window = crate::screen::window::tests::create_test_window(10);

        let mut render_data = RenderData::from_buffer(&window, &buffer, &mode);

        // Add virtual text to line 0
        render_data.virtual_texts[0] = Some(VirtualTextEntry {
            text: "◐ warning message".to_string(),
            style: Style::new(),
            priority: 403,
        });

        // Verify it was set
        assert!(render_data.virtual_texts[0].is_some());
        let vt = render_data.virtual_texts[0].as_ref().unwrap();
        assert_eq!(vt.text, "◐ warning message");
        assert_eq!(vt.priority, 403);
    }

    #[test]
    fn test_virtual_text_priority_resolution() {
        use crate::{
            buffer::{Buffer, Line},
            highlight::Style,
            modd::ModeState,
        };

        let mut buffer = Buffer::empty(0);
        buffer.contents.push(Line::from("let x = 1;"));

        let mode = ModeState::default();
        let window = crate::screen::window::tests::create_test_window(10);

        let mut render_data = RenderData::from_buffer(&window, &buffer, &mode);

        // Add lower-priority virtual text first
        render_data.virtual_texts[0] = Some(VirtualTextEntry {
            text: "· hint".to_string(),
            style: Style::new(),
            priority: 401,
        });

        // Higher priority should replace
        let should_replace = render_data.virtual_texts[0]
            .as_ref()
            .is_none_or(|existing| existing.priority < 404);
        assert!(should_replace);

        // Replace with higher priority
        render_data.virtual_texts[0] = Some(VirtualTextEntry {
            text: "● error".to_string(),
            style: Style::new(),
            priority: 404,
        });

        assert_eq!(render_data.virtual_texts[0].as_ref().unwrap().text, "● error");
    }
}
