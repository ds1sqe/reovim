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
                );
            }

            // Render line content
            Self::render_line_content(buffer, content_x, y, content_width, line, default_style);

            // Render cursor if on this line and window is focused
            if self.config.show_cursor && content.focused && buffer_line == content.cursor_line {
                Self::render_cursor(buffer, content_x, y, content.cursor_column, line);
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
    fn render_line_number(
        &self,
        buffer: &mut FrameBuffer,
        x: u16,
        y: u16,
        width: u16,
        line: usize,
        cursor_line: usize,
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

        buffer.write_str(x, y, &number_str, &style);
    }

    /// Render line content.
    fn render_line_content(
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
    fn render_cursor(
        buffer: &mut FrameBuffer,
        content_x: u16,
        y: u16,
        cursor_col: usize,
        line: &str,
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
    pub fn render_gutter_annotated(
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
#[path = "window_renderer_tests.rs"]
mod tests;
