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
mod tests {
    use super::*;

    #[test]
    fn test_render_data_new() {
        let lines = vec!["Hello".to_string(), "World".to_string()];
        let data = RenderData::new(lines);

        assert_eq!(data.line_count(), 2);
        assert_eq!(data.highlights.len(), 2);
        assert_eq!(data.gutter_decorations.len(), 2);
    }

    #[test]
    fn test_render_data_empty() {
        let data = RenderData::empty();
        assert_eq!(data.line_count(), 0);
    }

    #[test]
    fn test_render_data_add_highlight() {
        let lines = vec!["Hello World".to_string()];
        let mut data = RenderData::new(lines);

        data.add_highlight(0, 0, 5, Style::default());
        data.add_highlight(0, 6, 11, Style::default());

        assert_eq!(data.highlights[0].len(), 2);
        assert_eq!(data.highlights[0][0], (0, 5, Style::default()));
    }

    #[test]
    fn test_render_data_add_highlight_out_of_bounds() {
        let lines = vec!["Hello".to_string()];
        let mut data = RenderData::new(lines);

        // Should not panic, just ignored
        data.add_highlight(10, 0, 5, Style::default());

        assert_eq!(data.highlights[0].len(), 0);
    }

    #[test]
    fn test_render_data_set_gutter() {
        let lines = vec!["Line 1".to_string(), "Line 2".to_string()];
        let mut data = RenderData::new(lines);

        let decoration = GutterDecoration {
            char: 'E',
            style: Style::default(),
            priority: 10,
        };

        data.set_gutter(0, decoration);

        assert!(data.gutter_decorations[0].is_some());
        assert_eq!(data.gutter_decorations[0].as_ref().unwrap().char, 'E');
    }

    #[test]
    fn test_render_data_gutter_priority() {
        let lines = vec!["Line 1".to_string()];
        let mut data = RenderData::new(lines);

        // Set low priority decoration
        data.set_gutter(
            0,
            GutterDecoration {
                char: 'W',
                style: Style::default(),
                priority: 5,
            },
        );

        // Higher priority should replace
        data.set_gutter(
            0,
            GutterDecoration {
                char: 'E',
                style: Style::default(),
                priority: 10,
            },
        );

        assert_eq!(data.gutter_decorations[0].as_ref().unwrap().char, 'E');

        // Lower priority should not replace
        data.set_gutter(
            0,
            GutterDecoration {
                char: 'I',
                style: Style::default(),
                priority: 1,
            },
        );

        assert_eq!(data.gutter_decorations[0].as_ref().unwrap().char, 'E');
    }

    #[test]
    fn test_render_data_add_inline() {
        let lines = vec!["Hello".to_string()];
        let mut data = RenderData::new(lines);

        data.add_inline(InlineDecoration {
            line: 0,
            column: 5,
            text: " (world)".to_string(),
            style: Style::default(),
            is_virtual: true,
        });

        assert_eq!(data.inline_decorations.len(), 1);
        assert!(data.inline_decorations[0].is_virtual);
    }

    #[test]
    fn test_render_context_new() {
        let bounds = Rect::new(0, 0, 80, 24);
        let ctx = RenderContext::new(bounds);

        assert_eq!(ctx.bounds.width, 80);
        assert_eq!(ctx.bounds.height, 24);
        assert!(!ctx.focused);
    }

    #[test]
    fn test_render_context_builder() {
        let bounds = Rect::new(0, 0, 80, 24);
        let ctx = RenderContext::new(bounds)
            .focused(true)
            .cursor(10, 5)
            .viewport(5, 20)
            .total_lines(100);

        assert!(ctx.focused);
        assert_eq!(ctx.cursor_line, 10);
        assert_eq!(ctx.cursor_column, 5);
        assert_eq!(ctx.viewport_top, 5);
        assert_eq!(ctx.visible_lines, 20);
        assert_eq!(ctx.total_lines, 100);
    }

    #[test]
    fn test_render_context_is_line_visible() {
        let bounds = Rect::new(0, 0, 80, 24);
        let ctx = RenderContext::new(bounds).viewport(10, 20);

        assert!(!ctx.is_line_visible(9)); // Before viewport
        assert!(ctx.is_line_visible(10)); // First visible
        assert!(ctx.is_line_visible(20)); // Middle
        assert!(ctx.is_line_visible(29)); // Last visible
        assert!(!ctx.is_line_visible(30)); // After viewport
    }

    #[test]
    fn test_render_context_line_to_row() {
        let bounds = Rect::new(0, 0, 80, 24);
        let ctx = RenderContext::new(bounds).viewport(10, 20);

        assert_eq!(ctx.line_to_row(9), None);
        assert_eq!(ctx.line_to_row(10), Some(0));
        assert_eq!(ctx.line_to_row(15), Some(5));
        assert_eq!(ctx.line_to_row(29), Some(19));
        assert_eq!(ctx.line_to_row(30), None);
    }

    // Test stage for pipeline testing
    struct TestStage {
        name: &'static str,
        priority: u32,
        highlight: Option<(usize, usize, usize)>, // line, start, end
    }

    impl RenderStage for TestStage {
        fn name(&self) -> &str {
            self.name
        }

        fn process(&self, data: &mut RenderData, _context: &RenderContext) {
            if let Some((line, start, end)) = self.highlight {
                data.add_highlight(line, start, end, Style::default());
            }
        }

        fn priority(&self) -> u32 {
            self.priority
        }
    }

    #[test]
    fn test_execute_pipeline() {
        let lines = vec!["Hello World".to_string()];
        let mut data = RenderData::new(lines);
        let bounds = Rect::new(0, 0, 80, 24);
        let context = RenderContext::new(bounds);

        let mut stages: Vec<Box<dyn RenderStage>> = vec![
            Box::new(TestStage {
                name: "Stage1",
                priority: 20,
                highlight: Some((0, 6, 11)),
            }),
            Box::new(TestStage {
                name: "Stage2",
                priority: 10,
                highlight: Some((0, 0, 5)),
            }),
        ];

        execute_pipeline(&mut stages, &mut data, &context);

        // Both highlights should be added
        assert_eq!(data.highlights[0].len(), 2);
    }

    #[test]
    fn test_execute_pipeline_sorted() {
        let lines = vec!["Test".to_string()];
        let mut data = RenderData::new(lines);
        let bounds = Rect::new(0, 0, 80, 24);
        let context = RenderContext::new(bounds);

        let stages: Vec<Box<dyn RenderStage>> = vec![Box::new(TestStage {
            name: "TestStage",
            priority: 50,
            highlight: Some((0, 0, 4)),
        })];

        execute_pipeline_sorted(&stages, &mut data, &context);

        assert_eq!(data.highlights[0].len(), 1);
    }

    #[test]
    fn test_gutter_decoration() {
        let decoration = GutterDecoration {
            char: '●',
            style: Style::default(),
            priority: 5,
        };

        assert_eq!(decoration.char, '●');
        assert_eq!(decoration.priority, 5);
    }

    #[test]
    fn test_inline_decoration() {
        let decoration = InlineDecoration {
            line: 5,
            column: 10,
            text: "virtual text".to_string(),
            style: Style::default(),
            is_virtual: true,
        };

        assert_eq!(decoration.line, 5);
        assert_eq!(decoration.column, 10);
        assert!(decoration.is_virtual);
    }
}
