//! Shared rendering core for TUI frame capture.
//!
//! This module provides the `RenderState` struct and `build_frame_content` function
//! that can be shared between interactive `TuiApp` and `HeadlessClient`.

use std::fmt::Write as _;

use {
    reovim_driver_display::{ColorMode, FrameBuffer, Style},
    reovim_protocol::v1::ScreenFormat,
};

/// Render state containing the information needed for frame capture.
///
/// This struct holds the minimal state required to build a frame capture,
/// allowing both interactive and headless TUI clients to produce identical
/// output.
#[derive(Debug, Clone, Default)]
pub struct RenderState {
    /// Terminal width.
    pub width: u16,
    /// Terminal height.
    pub height: u16,
    /// Current mode display string (e.g., "NORMAL", "INSERT").
    pub mode_display: Option<String>,
    /// Cursor line (0-indexed).
    pub cursor_line: usize,
    /// Cursor column (0-indexed).
    pub cursor_column: usize,
    /// Loaded modules list.
    pub modules: Vec<String>,
    /// Server address string.
    pub server_address: String,
    /// Whether log panel is visible.
    pub log_panel_visible: bool,
}

impl RenderState {
    /// Create a new render state with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Update the terminal size.
    pub const fn set_size(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
    }

    /// Update the mode display string.
    pub fn set_mode(&mut self, mode: Option<String>) {
        self.mode_display = mode;
    }

    /// Update the cursor position.
    pub const fn set_cursor(&mut self, line: usize, column: usize) {
        self.cursor_line = line;
        self.cursor_column = column;
    }
}

/// Build frame content in LLM-friendly format.
///
/// This function produces a standardized frame capture format with:
/// - Clear section markers with `===`
/// - Key-value metadata
/// - Explicit line numbers in `[N]` format
/// - ANSI codes preserved for styled content
///
/// # Arguments
///
/// * `state` - Render state with metadata
/// * `buffer` - Frame buffer containing screen content
/// * `format` - Output format (affects content section)
///
/// # Returns
///
/// Formatted frame capture string.
#[must_use]
pub fn build_frame_content(
    state: &RenderState,
    buffer: &FrameBuffer,
    format: ScreenFormat,
) -> String {
    let (width, height) = (state.width, state.height);
    if width == 0 || height == 0 {
        return String::new();
    }

    let mut output = String::new();

    // Metadata section (key: value format for easy parsing)
    let now = chrono::Local::now();
    let timestamp = now.format("%Y-%m-%d %H:%M:%S %z").to_string();
    let mode = state.mode_display.as_deref().unwrap_or("UNKNOWN");
    let cursor_line = state.cursor_line;
    let cursor_col = state.cursor_column;
    let modules_count = state.modules.len();

    let _ = writeln!(output, "=== FRAME CAPTURE ===");
    let _ = writeln!(output, "timestamp: {timestamp}");
    let _ = writeln!(output, "screen_size: {width}x{height}");
    let _ = writeln!(output, "server: {}", state.server_address);
    let _ = writeln!(output, "mode: {mode}");
    let _ = writeln!(output, "cursor: line={cursor_line}, col={cursor_col}");
    let _ = writeln!(output, "modules: {modules_count}");
    let _ = writeln!(output, "log_panel_visible: {}", state.log_panel_visible);

    // Content section based on format
    let buf_height = buffer.height();
    let _ = writeln!(output, "\n=== SCREEN CONTENT ({width} cols x {buf_height} rows) ===");

    match format {
        ScreenFormat::RawAnsi => {
            write_ansi_content(&mut output, buffer, buf_height);
        }
        ScreenFormat::PlainText => {
            write_plain_content(&mut output, buffer, buf_height);
        }
        ScreenFormat::CellGrid => {
            // For cell grid, we'd need JSON serialization
            // For now, fall back to plain text
            write_plain_content(&mut output, buffer, buf_height);
        }
    }

    let _ = writeln!(output, "=== END FRAME ===");

    output
}

/// Write frame buffer content with ANSI color codes.
fn write_ansi_content(output: &mut String, buffer: &FrameBuffer, buf_height: u16) {
    for y in 0..buf_height {
        let mut line = String::new();
        let mut current_style: Option<Style> = None;

        if let Some(row) = buffer.row(y) {
            for cell in row {
                if cell.is_continuation {
                    continue;
                }

                // Track style changes for ANSI output
                let cell_style = &cell.style;
                if current_style.as_ref() != Some(cell_style) {
                    // Close previous style if any
                    if current_style.is_some() {
                        line.push_str("\x1b[0m");
                    }
                    // Open new style if not default
                    let ansi = cell_style.to_ansi_start(ColorMode::TrueColor);
                    if !ansi.is_empty() {
                        line.push_str(&ansi);
                    }
                    current_style = Some(cell_style.clone());
                }

                line.push(cell.char);
            }

            // Close any open style
            if current_style.is_some() {
                line.push_str("\x1b[0m");
            }
        }

        // Use [N] format for easy regex matching
        let _ = writeln!(output, "[{}] {}", y + 1, line);
    }
}

/// Write frame buffer content as plain text (no ANSI codes).
fn write_plain_content(output: &mut String, buffer: &FrameBuffer, buf_height: u16) {
    for y in 0..buf_height {
        let mut line = String::new();

        if let Some(row) = buffer.row(y) {
            for cell in row {
                if cell.is_continuation {
                    continue;
                }
                line.push(cell.char);
            }
        }

        // Use [N] format for easy regex matching
        let _ = writeln!(output, "[{}] {}", y + 1, line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_state_default() {
        let state = RenderState::default();
        assert_eq!(state.width, 0);
        assert_eq!(state.height, 0);
        assert!(state.mode_display.is_none());
        assert_eq!(state.cursor_line, 0);
        assert_eq!(state.cursor_column, 0);
        assert!(state.modules.is_empty());
    }

    #[test]
    fn test_render_state_set_size() {
        let mut state = RenderState::new();
        state.set_size(80, 24);
        assert_eq!(state.width, 80);
        assert_eq!(state.height, 24);
    }

    #[test]
    fn test_render_state_set_mode() {
        let mut state = RenderState::new();
        state.set_mode(Some("NORMAL".to_string()));
        assert_eq!(state.mode_display, Some("NORMAL".to_string()));
    }

    #[test]
    fn test_render_state_set_cursor() {
        let mut state = RenderState::new();
        state.set_cursor(10, 5);
        assert_eq!(state.cursor_line, 10);
        assert_eq!(state.cursor_column, 5);
    }

    #[test]
    fn test_build_frame_content_empty_size() {
        let state = RenderState::default();
        let buffer = FrameBuffer::new(0, 0);
        let content = build_frame_content(&state, &buffer, ScreenFormat::PlainText);
        assert!(content.is_empty());
    }

    #[test]
    fn test_build_frame_content_has_header() {
        let mut state = RenderState::new();
        state.set_size(80, 24);
        state.set_mode(Some("NORMAL".to_string()));
        state.server_address = "127.0.0.1:12521".to_string();

        let buffer = FrameBuffer::new(80, 24);
        let content = build_frame_content(&state, &buffer, ScreenFormat::PlainText);

        assert!(content.contains("=== FRAME CAPTURE ==="));
        assert!(content.contains("screen_size: 80x24"));
        assert!(content.contains("mode: NORMAL"));
        assert!(content.contains("server: 127.0.0.1:12521"));
        assert!(content.contains("=== END FRAME ==="));
    }

    #[test]
    fn test_build_frame_content_has_line_numbers() {
        let mut state = RenderState::new();
        state.set_size(80, 5);

        let buffer = FrameBuffer::new(80, 5);
        let content = build_frame_content(&state, &buffer, ScreenFormat::PlainText);

        assert!(content.contains("[1]"));
        assert!(content.contains("[5]"));
    }
}
