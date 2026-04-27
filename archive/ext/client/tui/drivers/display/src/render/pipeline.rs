//! Render pipeline for composable rendering stages.
//!
//! The render pipeline provides an extensible framework for processing
//! window content through multiple stages (syntax highlighting, selection,
//! diagnostics, etc.) before final rendering.
//!
//! # Architecture
//!
//! ```text
//! Buffer Content → [Stage 1] → [Stage 2] → [Stage 3] → RenderData → FrameBuffer
//!                    │            │            │
//!                  Extract    Highlight     Decorate
//! ```

use crate::{compositor::Style, window::Rect};

/// Render data produced by the pipeline.
///
/// This is an intermediate representation that accumulates
/// the results of all render stages before final rendering.
#[derive(Debug, Clone, Default)]
pub struct RenderData {
    /// Lines of text to render.
    pub lines: Vec<String>,
    /// Highlight spans for each line: (`start_col`, `end_col`, style).
    pub highlights: Vec<Vec<(usize, usize, Style)>>,
    /// Cursor position (x, y) relative to content area.
    pub cursor_pos: Option<(u16, u16)>,
    /// First visible line in the buffer (viewport offset).
    pub viewport_start: usize,
    /// Decorations for the gutter (line-indexed).
    pub gutter_decorations: Vec<Option<GutterDecoration>>,
    /// Inline decorations (virtual text, etc.).
    pub inline_decorations: Vec<InlineDecoration>,
}

/// Decoration for the gutter (sign column, diagnostics, etc.).
#[derive(Debug, Clone)]
pub struct GutterDecoration {
    /// Character to display.
    pub char: char,
    /// Style for the decoration.
    pub style: Style,
    /// Priority (higher = displayed on top).
    pub priority: u8,
}

/// Inline decoration (virtual text inserted into lines).
#[derive(Debug, Clone)]
pub struct InlineDecoration {
    /// Line number (0-indexed).
    pub line: usize,
    /// Column position.
    pub column: usize,
    /// Text to insert.
    pub text: String,
    /// Style for the text.
    pub style: Style,
    /// Whether this is virtual (doesn't affect cursor position).
    pub is_virtual: bool,
}

impl RenderData {
    /// Create new render data with the given lines.
    #[must_use]
    pub fn new(lines: Vec<String>) -> Self {
        let line_count = lines.len();
        Self {
            lines,
            highlights: vec![Vec::new(); line_count],
            cursor_pos: None,
            viewport_start: 0,
            gutter_decorations: vec![None; line_count],
            inline_decorations: Vec::new(),
        }
    }

    /// Create empty render data.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Add a highlight span to a line.
    pub fn add_highlight(&mut self, line: usize, start: usize, end: usize, style: Style) {
        if line < self.highlights.len() {
            self.highlights[line].push((start, end, style));
        }
    }

    /// Set a gutter decoration for a line.
    pub fn set_gutter(&mut self, line: usize, decoration: GutterDecoration) {
        if line < self.gutter_decorations.len() {
            // Only replace if higher priority or no existing decoration
            if self.gutter_decorations[line]
                .as_ref()
                .is_none_or(|existing| decoration.priority >= existing.priority)
            {
                self.gutter_decorations[line] = Some(decoration);
            }
        }
    }

    /// Add an inline decoration.
    pub fn add_inline(&mut self, decoration: InlineDecoration) {
        self.inline_decorations.push(decoration);
    }

    /// Get the number of lines.
    #[must_use]
    pub const fn line_count(&self) -> usize {
        self.lines.len()
    }
}

/// Context provided to render stages.
///
/// Contains all the information a stage needs to process content.
#[derive(Debug, Clone)]
pub struct RenderContext {
    /// Bounds of the window being rendered.
    pub bounds: Rect,
    /// Whether this window is focused.
    pub focused: bool,
    /// Default text style.
    pub default_style: Style,
    /// Cursor line (0-indexed, buffer coordinates).
    pub cursor_line: usize,
    /// Cursor column (0-indexed).
    pub cursor_column: usize,
    /// Total lines in the buffer.
    pub total_lines: usize,
    /// First visible line (viewport top).
    pub viewport_top: usize,
    /// Number of visible lines.
    pub visible_lines: usize,
}

impl RenderContext {
    /// Create a new render context.
    #[must_use]
    pub fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            focused: false,
            default_style: Style::default(),
            cursor_line: 0,
            cursor_column: 0,
            total_lines: 0,
            viewport_top: 0,
            visible_lines: bounds.height as usize,
        }
    }

    /// Set whether the window is focused.
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Set the cursor position.
    #[must_use]
    pub const fn cursor(mut self, line: usize, column: usize) -> Self {
        self.cursor_line = line;
        self.cursor_column = column;
        self
    }

    /// Set the viewport.
    #[must_use]
    pub const fn viewport(mut self, top: usize, visible: usize) -> Self {
        self.viewport_top = top;
        self.visible_lines = visible;
        self
    }

    /// Set the total line count.
    #[must_use]
    pub const fn total_lines(mut self, total: usize) -> Self {
        self.total_lines = total;
        self
    }

    /// Check if a buffer line is visible.
    #[must_use]
    pub const fn is_line_visible(&self, line: usize) -> bool {
        line >= self.viewport_top && line < self.viewport_top + self.visible_lines
    }

    /// Convert buffer line to screen row.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn line_to_row(&self, line: usize) -> Option<u16> {
        if self.is_line_visible(line) {
            // Truncation safe: viewport height is bounded by terminal size (u16)
            Some((line - self.viewport_top) as u16)
        } else {
            None
        }
    }
}

/// A render stage that processes render data.
///
/// Stages are executed in priority order (lower priority = earlier).
/// Each stage can modify the `RenderData` to add highlights,
/// decorations, or transform content.
///
/// # Example Stages
///
/// - `LineExtract` (priority 0): Extract visible lines from buffer
/// - `SyntaxHighlight` (priority 10): Apply syntax highlighting
/// - `Selection` (priority 20): Highlight selected text
/// - `Search` (priority 30): Highlight search matches
/// - `Diagnostics` (priority 40): Show error/warning indicators
pub trait RenderStage: Send + Sync {
    /// Stage name for debugging.
    fn name(&self) -> &str;

    /// Process render data.
    ///
    /// Modify `data` in place to add highlights, decorations, etc.
    fn process(&self, data: &mut RenderData, context: &RenderContext);

    /// Priority for stage ordering.
    ///
    /// Lower values run first. Default is 100.
    fn priority(&self) -> u32 {
        100
    }
}

/// Execute the render pipeline.
///
/// Runs all stages in priority order, each stage modifying
/// the render data in place.
///
/// # Arguments
///
/// * `stages` - List of render stages to execute
/// * `data` - Initial render data
/// * `context` - Render context
pub fn execute_pipeline(
    stages: &mut [Box<dyn RenderStage>],
    data: &mut RenderData,
    context: &RenderContext,
) {
    // Sort stages by priority
    stages.sort_by_key(|s| s.priority());

    // Execute each stage
    for stage in stages.iter() {
        stage.process(data, context);
    }
}

/// Execute pipeline with immutable stage list (stages already sorted).
pub fn execute_pipeline_sorted(
    stages: &[Box<dyn RenderStage>],
    data: &mut RenderData,
    context: &RenderContext,
) {
    for stage in stages {
        stage.process(data, context);
    }
}

#[cfg(test)]
#[path = "pipeline_tests.rs"]
mod tests;
