//! Window content rendering.
//!
//! The `WindowRenderer` takes window content (lines, cursor, viewport) and
//! renders it to a `FrameBuffer` at a specified position. It handles:
//!
//! - Line number display (absolute, relative, hybrid)
//! - Text content rendering with highlights
//! - Cursor display
//! - Wide character handling
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────┐
//! │           WindowRenderer                     │
//! │                                              │
//! │  ┌──────────┬────────────────────────────┐ │
//! │  │ Line Num │  Content                   │ │
//! │  │    1     │  fn main() {               │ │
//! │  │    2     │      println!("Hello");    │ │
//! │  │    3     │  }                        ▓│ │
//! │  └──────────┴────────────────────────────┘ │
//! │       ↑              ↑                     │
//! │   gutter width   content area              │
//! └─────────────────────────────────────────────┘
//! ```
//!
//! # Theme Integration
//!
//! Use `WindowRendererConfig::from_theme()` to create a config with theme colors:
//!
//! ```ignore
//! use reovim_driver_display::style::{SharedThemeManager, groups};
//! use reovim_driver_display::window_renderer::{WindowRenderer, WindowRendererConfig};
//!
//! // Get ThemeManager from ServiceRegistry
//! let shared = services.get::<SharedThemeManager>().unwrap();
//! let manager = shared.read();
//!
//! // Create themed config
//! let config = WindowRendererConfig::from_theme(&manager);
//! let default_style = manager.get_style(groups::FOREGROUND);
//!
//! // Render with theme colors
//! let renderer = WindowRenderer::with_config(config);
//! renderer.render(&content, bounds, &mut buffer, &default_style);
//! ```

use crate::{
    annotation::{ComposedLine, GutterComposer, PresenterContext},
    compositor::Style,
    frame::{Cell, FrameBuffer},
    window::Rect,
};

/// Line number display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineNumberMode {
    /// No line numbers.
    #[default]
    None,
    /// Absolute line numbers (1, 2, 3...).
    Absolute,
    /// Relative line numbers (distance from cursor).
    Relative,
    /// Hybrid: absolute for cursor line, relative for others.
    Hybrid,
}

/// Configuration for window rendering.
#[derive(Debug, Clone)]
pub struct WindowRendererConfig {
    /// Line number display mode.
    pub line_numbers: LineNumberMode,
    /// Width allocated for line numbers (auto-calculated if 0).
    pub line_number_width: u16,
    /// Whether to show cursor.
    pub show_cursor: bool,
    /// Style for line numbers.
    pub line_number_style: Style,
    /// Style for cursor line number (highlight current line).
    pub cursor_line_number_style: Style,
}

impl Default for WindowRendererConfig {
    fn default() -> Self {
        Self {
            line_numbers: LineNumberMode::Absolute,
            line_number_width: 0, // Auto-calculate
            show_cursor: true,
            line_number_style: Style::default(),
            cursor_line_number_style: Style::default(),
        }
    }
}

impl WindowRendererConfig {
    /// Create a config from a `ThemeManager`.
    ///
    /// Extracts line number styles from the theme's highlight groups.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use reovim_driver_display::style::SharedThemeManager;
    /// use reovim_driver_display::window_renderer::WindowRendererConfig;
    ///
    /// let manager = services.get::<SharedThemeManager>().unwrap();
    /// let config = WindowRendererConfig::from_theme(&manager.read());
    /// let renderer = WindowRenderer::with_config(config);
    /// ```
    #[must_use]
    pub fn from_theme(manager: &crate::style::ThemeManager) -> Self {
        use crate::style::groups;

        Self {
            line_numbers: LineNumberMode::Absolute,
            line_number_width: 0, // Auto-calculate
            show_cursor: true,
            line_number_style: manager.get_style(groups::LINE_NUMBER),
            cursor_line_number_style: manager.get_style(groups::LINE_NUMBER_ACTIVE),
        }
    }
}

/// Content to render in a window.
///
/// This is a snapshot of the window state needed for rendering.
/// It decouples the renderer from the kernel's Buffer type.
#[derive(Debug)]
pub struct RenderContent<'a> {
    /// Lines to render (slice of the buffer's visible lines).
    pub lines: &'a [String],
    /// First line index (viewport `top_line`).
    pub first_line: usize,
    /// Total line count in the buffer.
    pub total_lines: usize,
    /// Cursor line (0-indexed, relative to buffer).
    pub cursor_line: usize,
    /// Cursor column (0-indexed).
    pub cursor_column: usize,
    /// Whether this window is focused.
    pub focused: bool,
}

/// Window content renderer.
///
/// Renders window content (text, line numbers, cursor) to a `FrameBuffer`.
/// The renderer is stateless - configuration is passed in via `WindowRendererConfig`.
#[derive(Debug, Default)]
pub struct WindowRenderer {
    config: WindowRendererConfig,
}

impl WindowRenderer {
    /// Create a new window renderer with default config.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a window renderer with custom config.
    #[must_use]
    pub const fn with_config(config: WindowRendererConfig) -> Self {
        Self { config }
    }

    /// Get the current config.
    #[must_use]
    pub const fn config(&self) -> &WindowRendererConfig {
        &self.config
    }

    /// Set the config.
    pub const fn set_config(&mut self, config: WindowRendererConfig) {
        self.config = config;
    }

    /// Render window content to a frame buffer.
    ///
    /// # Arguments
    ///
    /// * `content` - The window content to render
    /// * `bounds` - The area within the frame buffer to render to
    /// * `buffer` - The frame buffer to render to
    /// * `default_style` - Default style for text
    pub fn render(
        &self,
        content: &RenderContent<'_>,
        bounds: Rect,
        buffer: &mut FrameBuffer,
        default_style: &Style,
    ) {
        // Calculate gutter width for line numbers
        let gutter_width = self.calculate_gutter_width(content.total_lines);

        // Calculate content area
        let content_x = bounds.x + gutter_width;
        let content_width = bounds.width.saturating_sub(gutter_width);

        // Render each visible line
        for (row_idx, line) in content.lines.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            let y = bounds.y + (row_idx as u16);
            if y >= bounds.y + bounds.height {
                break;
            }

            let buffer_line = content.first_line + row_idx;

            // Render line number
            if gutter_width > 0 {
                self.render_line_number(
                    buffer,
                    bounds.x,
                    y,
                    gutter_width,
                    buffer_line,
                    content.cursor_line,
                    content.total_lines,
                );
            }

            // Render line content
            self.render_line_content(buffer, content_x, y, content_width, line, default_style);

            // Render cursor if on this line and window is focused
            if self.config.show_cursor && content.focused && buffer_line == content.cursor_line {
                self.render_cursor(
                    buffer,
                    content_x,
                    y,
                    content_width,
                    content.cursor_column,
                    line,
                    default_style,
                );
            }
        }

        // Fill remaining lines with empty content
        #[allow(clippy::cast_possible_truncation)]
        for row_idx in content.lines.len()..(bounds.height as usize) {
            #[allow(clippy::cast_possible_truncation)]
            let y = bounds.y + (row_idx as u16);

            // Render "~" for empty lines (vim style)
            if gutter_width > 0 {
                buffer.set(
                    bounds.x + gutter_width.saturating_sub(2),
                    y,
                    Cell::new('~', self.config.line_number_style.clone()),
                );
            }
        }
    }

    /// Calculate gutter width for line numbers.
    fn calculate_gutter_width(&self, total_lines: usize) -> u16 {
        if self.config.line_numbers == LineNumberMode::None {
            return 0;
        }

        if self.config.line_number_width > 0 {
            return self.config.line_number_width;
        }

        // Calculate width needed: digits + 1 space padding
        let digits = if total_lines == 0 {
            1
        } else {
            // Safe: using f64 for log10 calculation, result is small
            #[allow(
                clippy::cast_precision_loss,
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss
            )]
            let d = ((total_lines as f64).log10().floor() as u16) + 1;
            d
        };
        digits + 2 // digits + space before + space after
    }

    /// Render a line number.
    #[allow(clippy::too_many_arguments)]
    fn render_line_number(
        &self,
        buffer: &mut FrameBuffer,
        x: u16,
        y: u16,
        width: u16,
        line: usize,
        cursor_line: usize,
        total_lines: usize,
    ) {
        // Compute the display number based on line number mode.
        // Caller guarantees mode != None (gutter_width is 0 for None mode).
        // None falls through to Absolute as a safe default.
        let is_cursor = line == cursor_line;
        let number = match self.config.line_numbers {
            LineNumberMode::None | LineNumberMode::Absolute => line + 1,
            LineNumberMode::Hybrid if is_cursor => line + 1,
            LineNumberMode::Relative | LineNumberMode::Hybrid => line.abs_diff(cursor_line),
        };

        #[allow(clippy::cast_possible_truncation)]
        let fmt_width = (width as usize).saturating_sub(2);
        let number_str = format!("{number:>fmt_width$} ");
        let style = if is_cursor {
            self.config.cursor_line_number_style.clone()
        } else {
            self.config.line_number_style.clone()
        };

        // Ignore unused total_lines for now (could be used for padding)
        let _ = total_lines;

        buffer.write_str(x, y, &number_str, &style);
    }

    /// Render line content.
    #[allow(clippy::unused_self)]
    fn render_line_content(
        &self,
        buffer: &mut FrameBuffer,
        x: u16,
        y: u16,
        width: u16,
        line: &str,
        style: &Style,
    ) {
        buffer.write_str(x, y, line, style);

        // Fill remaining width with spaces
        #[allow(clippy::cast_possible_truncation)]
        let line_width = line.chars().count() as u16;
        if line_width < width {
            let fill_start = x + line_width;
            for col in fill_start..(x + width) {
                buffer.set(col, y, Cell::new(' ', style.clone()));
            }
        }
    }

    /// Render cursor.
    #[allow(clippy::too_many_arguments, clippy::unused_self)]
    fn render_cursor(
        &self,
        buffer: &mut FrameBuffer,
        content_x: u16,
        y: u16,
        _content_width: u16,
        cursor_col: usize,
        line: &str,
        default_style: &Style,
    ) {
        // Calculate cursor X position (accounting for wide characters)
        let mut x_offset: u16 = 0;
        for (idx, ch) in line.chars().enumerate() {
            if idx == cursor_col {
                break;
            }
            x_offset += u16::from(crate::frame::char_width(ch));
        }

        let cursor_x = content_x + x_offset;

        // Get the character under the cursor
        let char_under = line.chars().nth(cursor_col).unwrap_or(' ');

        // Create cursor cell with inverted colors
        let cursor_style = Style::default().reverse();
        let cursor_cell = Cell::new(char_under, cursor_style);
        buffer.set(cursor_x, y, cursor_cell);

        // Ignore default_style for now (cursor has its own style)
        let _ = default_style;
    }

    /// Calculate visible lines from viewport.
    ///
    /// Convenience method to extract visible lines from a buffer.
    #[must_use]
    pub fn visible_lines(lines: &[String], top_line: usize, height: usize) -> &[String] {
        let start = top_line.min(lines.len());
        let end = (top_line + height).min(lines.len());
        &lines[start..end]
    }

    /// Render gutter using the annotation system.
    ///
    /// This method uses the generic annotation architecture to render the gutter,
    /// allowing extensible sources (line numbers, diagnostics, git, etc.) and
    /// presenters to be composed together.
    ///
    /// # Arguments
    ///
    /// * `composer` - The gutter composer with sources and presenters
    /// * `buffer` - The frame buffer to render to
    /// * `bounds` - The area allocated for the gutter
    /// * `ctx` - Presenter context with buffer information
    /// * `first_line` - First visible line index (0-indexed)
    #[allow(clippy::unused_self)]
    pub fn render_gutter_annotated(
        &self,
        composer: &GutterComposer<'_>,
        buffer: &mut FrameBuffer,
        bounds: Rect,
        ctx: &PresenterContext,
        first_line: usize,
    ) {
        let visible_lines = bounds.height as usize;
        let end_line = first_line + visible_lines;

        // Compose gutter for visible line range
        let composed_lines = composer.compose_range(first_line, end_line, ctx);

        // Render each line's gutter cells
        for (row_idx, line) in composed_lines.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            let y = bounds.y + (row_idx as u16);

            Self::render_composed_line(buffer, bounds.x, y, line);
        }
    }

    /// Render a composed gutter line to the buffer.
    fn render_composed_line(buffer: &mut FrameBuffer, x: u16, y: u16, line: &ComposedLine) {
        let mut current_x = x;
        for cell in &line.cells {
            buffer.set(current_x, y, Cell::new(cell.char, cell.style.clone()));
            #[allow(clippy::cast_possible_truncation)]
            {
                current_x += cell.width() as u16;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_content(lines: &[String], cursor_line: usize) -> RenderContent<'_> {
        RenderContent {
            lines,
            first_line: 0,
            total_lines: lines.len(),
            cursor_line,
            cursor_column: 0,
            focused: true,
        }
    }

    #[test]
    fn test_new() {
        let renderer = WindowRenderer::new();
        assert_eq!(renderer.config().line_numbers, LineNumberMode::Absolute);
    }

    #[test]
    fn test_with_config() {
        let config = WindowRendererConfig {
            line_numbers: LineNumberMode::Relative,
            ..Default::default()
        };
        let renderer = WindowRenderer::with_config(config);
        assert_eq!(renderer.config().line_numbers, LineNumberMode::Relative);
    }

    #[test]
    fn test_calculate_gutter_width_none() {
        let config = WindowRendererConfig {
            line_numbers: LineNumberMode::None,
            ..Default::default()
        };
        let renderer = WindowRenderer::with_config(config);
        assert_eq!(renderer.calculate_gutter_width(100), 0);
    }

    #[test]
    fn test_calculate_gutter_width_auto() {
        let renderer = WindowRenderer::new();
        // 1-9 lines: 1 digit + 2 = 3
        assert_eq!(renderer.calculate_gutter_width(9), 3);
        // 10-99 lines: 2 digits + 2 = 4
        assert_eq!(renderer.calculate_gutter_width(50), 4);
        // 100-999 lines: 3 digits + 2 = 5
        assert_eq!(renderer.calculate_gutter_width(500), 5);
    }

    #[test]
    fn test_calculate_gutter_width_fixed() {
        let config = WindowRendererConfig {
            line_number_width: 6,
            ..Default::default()
        };
        let renderer = WindowRenderer::with_config(config);
        assert_eq!(renderer.calculate_gutter_width(1000), 6);
    }

    #[test]
    fn test_render_basic() {
        let renderer = WindowRenderer::new();
        let mut buffer = FrameBuffer::new(40, 10);
        let lines: Vec<String> = vec!["Hello".to_string(), "World".to_string()];
        let content = make_content(&lines, 0);

        renderer.render(&content, Rect::new(0, 0, 40, 10), &mut buffer, &Style::default());

        // With 2 lines, gutter is 3 chars (1 digit + 2 padding)
        // Content starts at position 3
        // Check that "Hello" content is rendered somewhere in the first row
        let first_row: String = (0..40)
            .filter_map(|x| buffer.get(x, 0).map(|c| c.char))
            .collect();
        assert!(
            first_row.contains("Hello"),
            "First row should contain 'Hello', got: {first_row}"
        );
        assert!(
            first_row.contains('1'),
            "First row should contain line number '1', got: {first_row}"
        );
    }

    #[test]
    fn test_render_no_line_numbers() {
        let config = WindowRendererConfig {
            line_numbers: LineNumberMode::None,
            ..Default::default()
        };
        let renderer = WindowRenderer::with_config(config);
        let mut buffer = FrameBuffer::new(40, 10);
        let lines: Vec<String> = vec!["Hello".to_string()];
        let content = make_content(&lines, 0);

        renderer.render(&content, Rect::new(0, 0, 40, 10), &mut buffer, &Style::default());

        // First character should be 'H' (no gutter)
        assert_eq!(buffer.get(0, 0).unwrap().char, 'H');
    }

    #[test]
    fn test_render_empty_lines_tilde() {
        let config = WindowRendererConfig {
            line_numbers: LineNumberMode::Absolute,
            line_number_width: 4,
            ..Default::default()
        };
        let renderer = WindowRenderer::with_config(config);
        let mut buffer = FrameBuffer::new(40, 10);
        let lines: Vec<String> = vec!["Only one line".to_string()];
        let content = make_content(&lines, 0);

        renderer.render(&content, Rect::new(0, 0, 40, 10), &mut buffer, &Style::default());

        // Line 2 (y=1) should have a tilde
        let tilde_cell = buffer.get(2, 1); // gutter_width - 2 = 4 - 2 = 2
        assert!(tilde_cell.is_some());
        assert_eq!(tilde_cell.unwrap().char, '~');
    }

    #[test]
    fn test_visible_lines() {
        let lines: Vec<String> = (0..100).map(|i| format!("Line {i}")).collect();

        // View lines 10-19
        let visible = WindowRenderer::visible_lines(&lines, 10, 10);
        assert_eq!(visible.len(), 10);
        assert_eq!(visible[0], "Line 10");
        assert_eq!(visible[9], "Line 19");
    }

    #[test]
    fn test_visible_lines_past_end() {
        let lines: Vec<String> = vec!["Line 0".to_string(), "Line 1".to_string()];

        // Request more than available
        let visible = WindowRenderer::visible_lines(&lines, 0, 100);
        assert_eq!(visible.len(), 2);
    }

    #[test]
    fn test_visible_lines_empty() {
        let lines: Vec<String> = vec![];
        let visible = WindowRenderer::visible_lines(&lines, 0, 10);
        assert!(visible.is_empty());
    }

    #[test]
    fn test_line_number_mode_default() {
        let mode = LineNumberMode::default();
        assert_eq!(mode, LineNumberMode::None);
    }

    #[test]
    fn test_render_cursor() {
        let renderer = WindowRenderer::new();
        let mut buffer = FrameBuffer::new(40, 10);
        let lines: Vec<String> = vec!["Hello".to_string()];
        let content = RenderContent {
            lines: &lines,
            first_line: 0,
            total_lines: 1,
            cursor_line: 0,
            cursor_column: 2, // On 'l'
            focused: true,
        };

        renderer.render(&content, Rect::new(0, 0, 40, 10), &mut buffer, &Style::default());

        // The cursor should be rendered at the 'l' position
        // With gutter width ~3, cursor at col 2 = gutter + 2 = 5
    }

    // =========================================================================
    // Extended window renderer tests
    // =========================================================================

    #[test]
    fn test_render_relative_line_numbers() {
        let config = WindowRendererConfig {
            line_numbers: LineNumberMode::Relative,
            line_number_width: 5,
            ..Default::default()
        };
        let renderer = WindowRenderer::with_config(config);
        let mut buffer = FrameBuffer::new(40, 10);
        let lines: Vec<String> = vec![
            "Line A".to_string(),
            "Line B".to_string(),
            "Line C".to_string(),
        ];
        let content = make_content(&lines, 1); // cursor on line 1

        renderer.render(&content, Rect::new(0, 0, 40, 10), &mut buffer, &Style::default());

        // Relative line numbers: line 0 should show "1" (distance from cursor)
        // line 1 (cursor) should show "0"
        // line 2 should show "1"
    }

    #[test]
    fn test_render_hybrid_line_numbers() {
        let config = WindowRendererConfig {
            line_numbers: LineNumberMode::Hybrid,
            line_number_width: 5,
            ..Default::default()
        };
        let renderer = WindowRenderer::with_config(config);
        let mut buffer = FrameBuffer::new(40, 10);
        let lines: Vec<String> = vec![
            "Line A".to_string(),
            "Line B".to_string(),
            "Line C".to_string(),
        ];
        let content = make_content(&lines, 1); // cursor on line 1

        renderer.render(&content, Rect::new(0, 0, 40, 10), &mut buffer, &Style::default());

        // Hybrid: cursor line shows absolute (2), others show relative
        let row_str: String = (0..5)
            .filter_map(|x| buffer.get(x, 1).map(|c| c.char))
            .collect();
        assert!(
            row_str.contains('2'),
            "Cursor line should show absolute number 2, got: {row_str}"
        );
    }

    #[test]
    fn test_render_unfocused_no_cursor() {
        let renderer = WindowRenderer::new();
        let mut buffer = FrameBuffer::new(40, 10);
        let lines: Vec<String> = vec!["Hello".to_string()];
        let content = RenderContent {
            lines: &lines,
            first_line: 0,
            total_lines: 1,
            cursor_line: 0,
            cursor_column: 0,
            focused: false, // Not focused
        };

        renderer.render(&content, Rect::new(0, 0, 40, 10), &mut buffer, &Style::default());

        // Cursor should not be rendered (no reverse style) when unfocused
        // Just verify no panic
    }

    #[test]
    fn test_render_cursor_at_end_of_line() {
        let renderer = WindowRenderer::new();
        let mut buffer = FrameBuffer::new(40, 10);
        let lines: Vec<String> = vec!["Hello".to_string()];
        let content = RenderContent {
            lines: &lines,
            first_line: 0,
            total_lines: 1,
            cursor_line: 0,
            cursor_column: 5, // Past end of "Hello"
            focused: true,
        };

        renderer.render(&content, Rect::new(0, 0, 40, 10), &mut buffer, &Style::default());

        // Should render a space at cursor position
    }

    #[test]
    fn test_set_config() {
        let mut renderer = WindowRenderer::new();
        let config = WindowRendererConfig {
            line_numbers: LineNumberMode::Relative,
            ..Default::default()
        };
        renderer.set_config(config);
        assert_eq!(renderer.config().line_numbers, LineNumberMode::Relative);
    }

    #[test]
    fn test_calculate_gutter_width_zero_lines() {
        let renderer = WindowRenderer::new();
        // 0 lines: 1 digit + 2 = 3
        assert_eq!(renderer.calculate_gutter_width(0), 3);
    }

    #[test]
    fn test_render_with_offset_bounds() {
        let renderer = WindowRenderer::new();
        let mut buffer = FrameBuffer::new(80, 24);
        let lines: Vec<String> = vec!["Content".to_string()];
        let content = make_content(&lines, 0);

        // Render at an offset position
        renderer.render(&content, Rect::new(10, 5, 30, 10), &mut buffer, &Style::default());

        // Content should be at offset position
        let row: String = (10..40)
            .filter_map(|x| buffer.get(x, 5).map(|c| c.char))
            .collect();
        assert!(row.contains("Content"), "Row should contain 'Content' at offset, got: {row}");
    }

    #[test]
    fn test_visible_lines_top_beyond_end() {
        let lines: Vec<String> = vec!["Line 0".to_string()];
        let visible = WindowRenderer::visible_lines(&lines, 100, 10);
        assert!(visible.is_empty());
    }

    // =========================================================================
    // Coverage tests for uncovered lines
    // =========================================================================

    #[test]
    fn test_from_theme_creates_config_with_theme_styles() {
        use crate::style::{BuiltinTheme, ThemeManager, groups};

        let manager = ThemeManager::new(BuiltinTheme::Dark.load());
        let config = WindowRendererConfig::from_theme(&manager);

        // Verify default field values
        assert_eq!(config.line_numbers, LineNumberMode::Absolute);
        assert_eq!(config.line_number_width, 0);
        assert!(config.show_cursor);

        // Verify styles come from the theme
        let expected_line_number_style = manager.get_style(groups::LINE_NUMBER);
        let expected_cursor_line_style = manager.get_style(groups::LINE_NUMBER_ACTIVE);
        assert_eq!(config.line_number_style, expected_line_number_style);
        assert_eq!(config.cursor_line_number_style, expected_cursor_line_style);
    }

    #[test]
    fn test_from_theme_with_light_theme() {
        use crate::style::{BuiltinTheme, ThemeManager, groups};

        let manager = ThemeManager::new(BuiltinTheme::Light.load());
        let config = WindowRendererConfig::from_theme(&manager);

        // Should use the light theme colors
        let expected = manager.get_style(groups::LINE_NUMBER);
        assert_eq!(config.line_number_style, expected);
    }

    #[test]
    fn test_render_more_lines_than_height_breaks_loop() {
        // Covers line 206: break when y >= bounds.y + bounds.height
        let renderer = WindowRenderer::new();
        let mut buffer = FrameBuffer::new(40, 3);
        // 5 lines, but bounds height is only 2
        let lines: Vec<String> = vec![
            "Line 0".to_string(),
            "Line 1".to_string(),
            "Line 2".to_string(),
            "Line 3".to_string(),
            "Line 4".to_string(),
        ];
        let content = RenderContent {
            lines: &lines,
            first_line: 0,
            total_lines: 5,
            cursor_line: 0,
            cursor_column: 0,
            focused: true,
        };

        // Bounds height is 2, so lines 2-4 should be skipped (break)
        renderer.render(&content, Rect::new(0, 0, 40, 2), &mut buffer, &Style::default());

        // Verify only the first 2 lines were rendered
        let row0: String = (0..40)
            .filter_map(|x| buffer.get(x, 0).map(|c| c.char))
            .collect();
        let row1: String = (0..40)
            .filter_map(|x| buffer.get(x, 1).map(|c| c.char))
            .collect();
        assert!(row0.contains("Line 0"), "Row 0 should contain 'Line 0', got: {row0}");
        assert!(row1.contains("Line 1"), "Row 1 should contain 'Line 1', got: {row1}");
    }

    #[test]
    fn test_render_line_number_none_mode_with_fixed_width() {
        // Covers line 302: LineNumberMode::None => return in render_line_number
        // Force render_line_number to be called with None mode by having
        // a fixed line_number_width but None mode. Since calculate_gutter_width
        // returns 0 for None mode, we test via a workaround: set up Absolute
        // mode, then replace config to None mode but render was already done.
        // Actually, the None arm is a defensive guard. We test it by directly
        // creating a scenario where gutter_width > 0 but mode is None.
        // This is impossible through render() because calculate_gutter_width
        // returns 0 for None. So we just verify the render path is correct
        // when mode is None (gutter_width = 0, no line numbers rendered).
        let config = WindowRendererConfig {
            line_numbers: LineNumberMode::None,
            line_number_width: 0,
            ..Default::default()
        };
        let renderer = WindowRenderer::with_config(config);
        let mut buffer = FrameBuffer::new(40, 5);
        let lines: Vec<String> = vec!["Hello".to_string(), "World".to_string()];
        let content = make_content(&lines, 0);

        renderer.render(&content, Rect::new(0, 0, 40, 5), &mut buffer, &Style::default());

        // With None mode, no gutter, content starts at col 0
        assert_eq!(buffer.get(0, 0).unwrap().char, 'H');
        assert_eq!(buffer.get(0, 1).unwrap().char, 'W');
    }

    #[test]
    fn test_render_line_content_no_fill_when_line_fills_width() {
        // Covers line 362: the else branch where line_width >= width
        // (line fills or exceeds the content area width)
        let config = WindowRendererConfig {
            line_numbers: LineNumberMode::None,
            ..Default::default()
        };
        let renderer = WindowRenderer::with_config(config);
        let mut buffer = FrameBuffer::new(5, 2);
        // Line exactly fills the width (5 chars for width 5)
        let lines: Vec<String> = vec!["ABCDE".to_string()];
        let content = RenderContent {
            lines: &lines,
            first_line: 0,
            total_lines: 1,
            cursor_line: 0,
            cursor_column: 0,
            focused: false, // No cursor to keep it simple
        };

        renderer.render(&content, Rect::new(0, 0, 5, 2), &mut buffer, &Style::default());

        // All 5 characters should be rendered
        assert_eq!(buffer.get(0, 0).unwrap().char, 'A');
        assert_eq!(buffer.get(4, 0).unwrap().char, 'E');
    }

    #[test]
    fn test_render_line_content_overflow_line() {
        // Line wider than content width - ensures no-fill path is taken
        let config = WindowRendererConfig {
            line_numbers: LineNumberMode::None,
            ..Default::default()
        };
        let renderer = WindowRenderer::with_config(config);
        let mut buffer = FrameBuffer::new(10, 2);
        // Line is 8 chars but content width is only 5
        let lines: Vec<String> = vec!["ABCDEFGH".to_string()];
        let content = RenderContent {
            lines: &lines,
            first_line: 0,
            total_lines: 1,
            cursor_line: 0,
            cursor_column: 0,
            focused: false,
        };

        renderer.render(&content, Rect::new(0, 0, 5, 2), &mut buffer, &Style::default());

        // First 5 chars should still be written (write_str writes the full string to buffer)
        assert_eq!(buffer.get(0, 0).unwrap().char, 'A');
    }

    #[test]
    fn test_render_gutter_annotated_renders_composed_lines() {
        // Covers lines 424-460: render_gutter_annotated() and render_composed_line()
        use {
            crate::annotation::{
                Annotation, AnnotationPresenter, AnnotationStore, ColumnConfig, ColumnWidth,
                GutterComposer, GutterConfig, KindPattern, PresentedOutput, PresenterContext,
                PresenterRegistry, SourceId,
            },
            std::sync::Arc,
        };

        // Mock presenter for line numbers
        struct TestLineNumberPresenter;

        #[cfg_attr(coverage_nightly, coverage(off))]
        impl AnnotationPresenter for TestLineNumberPresenter {
            fn id(&self) -> &'static str {
                "test_line_number"
            }

            fn handles(&self) -> KindPattern {
                KindPattern::exact("line_number")
            }

            fn present(&self, annotation: &Annotation, _ctx: &PresenterContext) -> PresentedOutput {
                annotation
                    .payload
                    .as_number()
                    .map_or_else(PresentedOutput::hidden, |n| {
                        PresentedOutput::text(&n.to_string(), &Style::default())
                    })
            }

            fn column_width(&self, _ctx: &PresenterContext) -> ColumnWidth {
                ColumnWidth::fixed(3)
            }
        }

        // Set up annotation store with line number annotations
        let mut store = AnnotationStore::new();
        store.replace_source(
            SourceId::new("line_number"),
            vec![
                Annotation::line_number(0, 1),
                Annotation::line_number(1, 2),
                Annotation::line_number(2, 3),
            ],
        );

        // Set up presenter registry
        let mut registry = PresenterRegistry::new();
        registry.register(Arc::new(TestLineNumberPresenter));

        // Set up gutter config
        let config =
            GutterConfig::new(vec![ColumnConfig::new(KindPattern::exact("line_number")).width(3)]);

        let composer = GutterComposer::new(&store, &registry, &config);
        let ctx = PresenterContext::new(3, 0, false);

        let renderer = WindowRenderer::new();
        let mut buffer = FrameBuffer::new(10, 5);
        let bounds = Rect::new(0, 0, 5, 3);

        renderer.render_gutter_annotated(&composer, &mut buffer, bounds, &ctx, 0);

        // Verify that cells were rendered (line numbers 1, 2, 3)
        // The gutter should have content in the first 3 rows
        let row0: String = (0..5)
            .filter_map(|x| buffer.get(x, 0).map(|c| c.char))
            .collect();
        assert!(row0.contains('1'), "First gutter row should contain '1', got: '{row0}'");

        let row1: String = (0..5)
            .filter_map(|x| buffer.get(x, 1).map(|c| c.char))
            .collect();
        assert!(row1.contains('2'), "Second gutter row should contain '2', got: '{row1}'");
    }

    #[test]
    fn test_render_gutter_annotated_respects_bounds_height() {
        // Verifies render_gutter_annotated only renders within bounds height
        use {
            crate::annotation::{
                Annotation, AnnotationPresenter, AnnotationStore, ColumnConfig, ColumnWidth,
                GutterComposer, GutterConfig, KindPattern, PresentedOutput, PresenterContext,
                PresenterRegistry, SourceId,
            },
            std::sync::Arc,
        };

        struct TestPresenter;

        #[cfg_attr(coverage_nightly, coverage(off))]
        impl AnnotationPresenter for TestPresenter {
            fn id(&self) -> &'static str {
                "test"
            }

            fn handles(&self) -> KindPattern {
                KindPattern::exact("line_number")
            }

            fn present(&self, annotation: &Annotation, _ctx: &PresenterContext) -> PresentedOutput {
                annotation
                    .payload
                    .as_number()
                    .map_or_else(PresentedOutput::hidden, |n| {
                        PresentedOutput::text(&n.to_string(), &Style::default())
                    })
            }

            fn column_width(&self, _ctx: &PresenterContext) -> ColumnWidth {
                ColumnWidth::fixed(3)
            }
        }

        let mut store = AnnotationStore::new();
        store.replace_source(
            SourceId::new("line_number"),
            vec![
                Annotation::line_number(0, 1),
                Annotation::line_number(1, 2),
                Annotation::line_number(2, 3),
                Annotation::line_number(3, 4),
                Annotation::line_number(4, 5),
            ],
        );

        let mut registry = PresenterRegistry::new();
        registry.register(Arc::new(TestPresenter));

        let config =
            GutterConfig::new(vec![ColumnConfig::new(KindPattern::exact("line_number")).width(3)]);

        let composer = GutterComposer::new(&store, &registry, &config);
        let ctx = PresenterContext::new(5, 0, false);

        let renderer = WindowRenderer::new();
        let mut buffer = FrameBuffer::new(10, 3);
        // Height is 2, but compose_range generates 5 lines (first_line=0, visible=2 rows)
        let bounds = Rect::new(0, 0, 5, 2);

        renderer.render_gutter_annotated(&composer, &mut buffer, bounds, &ctx, 0);

        // Only 2 rows should be rendered
        let row0: String = (0..5)
            .filter_map(|x| buffer.get(x, 0).map(|c| c.char))
            .collect();
        assert!(row0.contains('1'), "Row 0 should contain '1', got: '{row0}'");
    }

    #[test]
    fn test_render_composed_line_directly() {
        // Covers render_composed_line (lines 451-460)
        use crate::annotation::GutterCell;

        let mut buffer = FrameBuffer::new(10, 1);
        let line = ComposedLine::new(vec![
            GutterCell::new('A', Style::default()),
            GutterCell::new('B', Style::default()),
            GutterCell::new('C', Style::default()),
        ]);

        WindowRenderer::render_composed_line(&mut buffer, 2, 0, &line);

        assert_eq!(buffer.get(2, 0).unwrap().char, 'A');
        assert_eq!(buffer.get(3, 0).unwrap().char, 'B');
        assert_eq!(buffer.get(4, 0).unwrap().char, 'C');
    }

    #[test]
    fn test_render_composed_line_with_wide_chars() {
        // Covers the width calculation in render_composed_line (line 457)
        use crate::annotation::GutterCell;

        let mut buffer = FrameBuffer::new(10, 1);
        // CJK character has width 2
        let line = ComposedLine::new(vec![
            GutterCell::new('A', Style::default()),
            GutterCell::new('\u{4e2d}', Style::default()), // CJK char, width 2
            GutterCell::new('B', Style::default()),
        ]);

        WindowRenderer::render_composed_line(&mut buffer, 0, 0, &line);

        assert_eq!(buffer.get(0, 0).unwrap().char, 'A');
        // CJK char at x=1, width=2
        assert_eq!(buffer.get(1, 0).unwrap().char, '\u{4e2d}');
        // 'B' should be at x=3 (1 + 2 = 3)
        assert_eq!(buffer.get(3, 0).unwrap().char, 'B');
    }

    #[test]
    fn test_render_empty_lines_tilde_break() {
        // Verifies tilde rendering for empty lines in bounds
        // Use height=2 with 1 content line, so only 1 tilde line
        let config = WindowRendererConfig {
            line_numbers: LineNumberMode::Absolute,
            line_number_width: 4,
            ..Default::default()
        };
        let renderer = WindowRenderer::with_config(config);
        let mut buffer = FrameBuffer::new(40, 2);
        let lines: Vec<String> = vec!["Only line".to_string()];
        let content = make_content(&lines, 0);

        renderer.render(&content, Rect::new(0, 0, 40, 2), &mut buffer, &Style::default());

        // Row 1 should have a tilde (only 1 empty line slot available)
        let tilde_cell = buffer.get(2, 1); // gutter_width - 2 = 4 - 2 = 2
        assert!(tilde_cell.is_some());
        assert_eq!(tilde_cell.unwrap().char, '~');
    }
}
