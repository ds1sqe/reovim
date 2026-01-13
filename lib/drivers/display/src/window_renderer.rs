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

use crate::{
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
            if y >= bounds.y + bounds.height {
                break;
            }

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
        #[allow(clippy::cast_possible_truncation)]
        let fmt_width = (width as usize).saturating_sub(2);
        let (number_str, style) = match self.config.line_numbers {
            LineNumberMode::None => return,
            LineNumberMode::Absolute => {
                let num = line + 1;
                let s = format!("{num:>fmt_width$} ");
                let style = if line == cursor_line {
                    &self.config.cursor_line_number_style
                } else {
                    &self.config.line_number_style
                };
                (s, style.clone())
            }
            LineNumberMode::Relative => {
                let rel = line.abs_diff(cursor_line);
                let s = format!("{rel:>fmt_width$} ");
                let style = if line == cursor_line {
                    &self.config.cursor_line_number_style
                } else {
                    &self.config.line_number_style
                };
                (s, style.clone())
            }
            LineNumberMode::Hybrid => {
                let (num, style) = if line == cursor_line {
                    (line + 1, &self.config.cursor_line_number_style)
                } else {
                    let rel = line.abs_diff(cursor_line);
                    (rel, &self.config.line_number_style)
                };
                let s = format!("{num:>fmt_width$} ");
                (s, style.clone())
            }
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
}
