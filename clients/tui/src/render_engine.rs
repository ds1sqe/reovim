//! Unified render engine for TUI.
//!
//! This module provides a single rendering implementation that works with
//! any `RenderBackend` (`Screen` for interactive, `FrameBuffer` for headless).
//! This is the "thick" layer in the thin-TUI architecture.
//!
//! # Status
//!
//! This is a skeleton for the unified render engine. Full implementation
//! will be migrated from `app.rs` incrementally.

use {reovim_arch::Color, reovim_driver_display::Style};

use crate::{LineNumberMode, TuiCoreState, render_backend::RenderBackend};

/// Render configuration for a frame.
///
/// Controls what features are rendered and provides optional data
/// like syntax tokens and theme styles.
#[derive(Debug, Default)]
pub struct RenderConfig {
    /// Whether to show line numbers in the gutter.
    pub show_line_numbers: bool,
    /// Line number display mode.
    pub line_number_mode: LineNumberMode,
    /// Whether to render self cursor in the backend.
    ///
    /// - `false`: Interactive mode (uses terminal cursor)
    /// - `true`: Headless mode (cursor rendered in buffer)
    pub render_self_cursor: bool,
    /// Gutter width (for line numbers).
    pub gutter_width: u16,
}

/// CBF-8 colorblind-friendly palette for remote cursors.
///
/// These 8 colors are distinguishable by people with common color vision
/// deficiencies (protanopia, deuteranopia, tritanopia).
pub const CBF8_PALETTE: [Color; 8] = [
    Color::Rgb {
        r: 0,
        g: 114,
        b: 178,
    }, // Blue
    Color::Rgb {
        r: 230,
        g: 159,
        b: 0,
    }, // Orange
    Color::Rgb {
        r: 86,
        g: 180,
        b: 233,
    }, // Sky blue
    Color::Rgb {
        r: 0,
        g: 158,
        b: 115,
    }, // Green
    Color::Rgb {
        r: 240,
        g: 228,
        b: 66,
    }, // Yellow
    Color::Rgb {
        r: 213,
        g: 94,
        b: 0,
    }, // Vermilion
    Color::Rgb {
        r: 204,
        g: 121,
        b: 167,
    }, // Pink
    Color::Rgb { r: 0, g: 0, b: 0 }, // Black (fallback)
];

/// Get a color from the CBF-8 palette for a client ID.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub const fn client_color(client_id: u64) -> Color {
    // Safe: modulo ensures we stay within palette bounds
    CBF8_PALETTE[(client_id as usize) % CBF8_PALETTE.len()]
}

/// Render a complete frame to the backend.
///
/// This is the single entry point for all TUI rendering.
/// Both interactive and headless TUIs call this function.
///
/// # Current Implementation
///
/// This is a basic implementation that renders:
/// - Buffer content (simple text, no syntax highlighting yet)
/// - Remote cursors
/// - Self cursor (if `render_self_cursor` is true)
/// - Statusline
pub fn render_frame<B: RenderBackend>(
    backend: &mut B,
    state: &TuiCoreState,
    config: &RenderConfig,
) {
    // Clear the backend
    backend.clear();

    let (width, height) = backend.size();

    // Reserve space for statusline
    let content_height = height.saturating_sub(1);

    // Render buffer content
    render_buffer_content(backend, state, config, content_height);

    // Render remote cursors
    render_remote_cursors(backend, state, config.gutter_width, content_height);

    // Render self cursor if needed (headless mode)
    if config.render_self_cursor {
        render_self_cursor(backend, state, config.gutter_width, content_height);
    }

    // Render statusline
    render_statusline(backend, state, width, height);
}

/// Render buffer content.
#[allow(clippy::cast_possible_truncation)]
fn render_buffer_content<B: RenderBackend>(
    backend: &mut B,
    state: &TuiCoreState,
    config: &RenderConfig,
    content_height: u16,
) {
    let (width, _) = backend.size();
    let gutter_width = config.gutter_width;
    let content_x = gutter_width;

    // Get first window's buffer for now (simplified)
    let buffer_id = state.windows.first().and_then(|w| w.buffer_id);

    let lines = buffer_id.and_then(|id| state.buffer_cache.get(&id));

    // Scroll offset (simplified - use 0 for now)
    let scroll_top = 0usize;

    for row in 0..content_height {
        let line_idx = scroll_top + row as usize;
        let screen_y = row;

        // Render line number if enabled
        if config.show_line_numbers && gutter_width > 0 {
            // Use cursor line for highlighting; default to 0 if no cursor data yet
            let cursor_line = state.get_focused_cursor().map_or(0, |c| c.line as usize);
            render_line_number(
                backend,
                0,
                screen_y,
                gutter_width,
                line_idx,
                cursor_line,
                config.line_number_mode,
            );
        }

        // Render line content
        if let Some(lines) = lines {
            if line_idx < lines.len() {
                let line = &lines[line_idx];
                render_line_content(backend, content_x, screen_y, width - content_x, line);
            } else {
                // Empty line indicator
                let tilde_style = Style::default().fg(Color::DarkGrey);
                backend.set_cell(content_x, screen_y, '~', &tilde_style);
            }
        }
    }
}

/// Render a line number in the gutter.
fn render_line_number<B: RenderBackend>(
    backend: &mut B,
    x: u16,
    y: u16,
    width: u16,
    line_idx: usize,
    cursor_line: usize,
    mode: LineNumberMode,
) {
    let line_num = line_idx + 1; // 1-indexed display

    let (display_num, is_cursor_line) = match mode {
        LineNumberMode::Absolute | LineNumberMode::Hybrid => (line_num, line_idx == cursor_line),
        LineNumberMode::Relative => {
            let rel = if line_idx == cursor_line {
                line_num // Show absolute for cursor line
            } else {
                line_idx.abs_diff(cursor_line)
            };
            (rel, line_idx == cursor_line)
        }
        LineNumberMode::None => return,
    };

    let style = if is_cursor_line {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGrey)
    };

    // Right-align the number
    let num_str = display_num.to_string();
    let padding = (width as usize).saturating_sub(num_str.len() + 1);
    let display = format!("{:>width$} ", num_str, width = padding + num_str.len());

    backend.write_str(x, y, &display, &style);
}

/// Render a line of buffer content.
#[allow(clippy::cast_possible_truncation)]
fn render_line_content<B: RenderBackend>(backend: &mut B, x: u16, y: u16, width: u16, line: &str) {
    let style = Style::default();

    for (col, ch) in line.chars().enumerate() {
        let col = col as u16;
        if col >= width {
            break;
        }
        backend.set_cell(x + col, y, ch, &style);
    }
}

/// Render remote client cursors.
#[allow(clippy::cast_possible_truncation)]
fn render_remote_cursors<B: RenderBackend>(
    backend: &mut B,
    state: &TuiCoreState,
    gutter_width: u16,
    content_height: u16,
) {
    let (width, _) = backend.size();

    // Get current buffer ID (simplified)
    let current_buffer_id = state.windows.first().and_then(|w| w.buffer_id);

    for remote in state.other_clients.values() {
        // Only render if in same buffer
        if remote.buffer_id != current_buffer_id {
            continue;
        }

        let cursor_color = client_color(remote.client_id);
        let cursor_style = Style::default().bg(cursor_color).fg(Color::White);

        // Calculate screen position (simplified - no scroll offset)
        let screen_line = remote.cursor_line;
        let screen_col = remote.cursor_col as u16 + gutter_width;

        if screen_line < u64::from(content_height) && screen_col < width {
            #[allow(clippy::cast_possible_truncation)]
            let screen_y = screen_line as u16;
            backend.apply_style(screen_col, screen_y, &cursor_style);
        }
    }
}

/// Render self cursor in the backend (for headless mode).
#[allow(clippy::cast_possible_truncation)]
fn render_self_cursor<B: RenderBackend>(
    backend: &mut B,
    state: &TuiCoreState,
    gutter_width: u16,
    content_height: u16,
) {
    // Skip rendering if no cursor data yet (e.g., before first CursorMoved notification)
    let Some(cursor) = state.get_focused_cursor() else {
        return;
    };

    let (width, _) = backend.size();
    let screen_line = cursor.line;
    let screen_col = cursor.column as u16 + gutter_width;

    if screen_line < u64::from(content_height) && screen_col < width {
        // Use inverse video for self cursor
        let cursor_style = Style::default().bg(Color::White).fg(Color::Black);
        #[allow(clippy::cast_possible_truncation)]
        let screen_y = screen_line as u16;
        backend.apply_style(screen_col, screen_y, &cursor_style);
    }
}

/// Render the statusline at the bottom of the screen.
#[allow(clippy::cast_possible_truncation)]
fn render_statusline<B: RenderBackend>(
    backend: &mut B,
    state: &TuiCoreState,
    width: u16,
    height: u16,
) {
    let status_y = height.saturating_sub(1);

    // Mode indicator
    let mode_style = mode_style(&state.mode_display);
    let mode_str = format!(" {} ", state.mode_display);
    backend.write_str(0, status_y, &mode_str, &mode_style);

    // Cursor position (right side) - use get_focused_cursor() as single source of truth
    // Show "?:?" if no cursor data yet (e.g., before first CursorMoved notification)
    let pos_str = state.get_focused_cursor().map_or_else(
        || "?:?".to_string(),
        |cursor| format!("{}:{}", cursor.line + 1, cursor.column + 1),
    );
    let pos_x = width.saturating_sub(pos_str.len() as u16 + 1);
    let pos_style = Style::default();
    backend.write_str(pos_x, status_y, &pos_str, &pos_style);
}

/// Get the style for a mode indicator.
fn mode_style(mode: &str) -> Style {
    let mode_lower = mode.to_lowercase();
    let (fg, bg) = if mode_lower.contains("insert") {
        (Color::Black, Color::Green)
    } else if mode_lower.contains("visual") {
        (Color::Black, Color::Magenta)
    } else if mode_lower.contains("command") || mode_lower.contains("cmdline") {
        (Color::Black, Color::Yellow)
    } else if mode_lower.contains("replace") {
        (Color::Black, Color::Red)
    } else {
        // Normal mode
        (Color::Black, Color::Blue)
    };
    Style::default().fg(fg).bg(bg)
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_driver_display::FrameBuffer};

    #[test]
    fn test_client_color() {
        // Should cycle through palette
        let c0 = client_color(0);
        let c1 = client_color(1);
        let c8 = client_color(8); // Should wrap to c0

        assert_ne!(c0, c1);
        assert_eq!(c0, c8);
    }

    #[test]
    fn test_render_frame_basic() {
        let mut fb = FrameBuffer::new(80, 24);
        let state = TuiCoreState::new(1);
        let config = RenderConfig::default();

        render_frame(&mut fb, &state, &config);

        // Should have rendered statusline
        let last_row = fb.row(23).unwrap();
        // At minimum, some cells should be non-empty
        assert!(last_row.iter().any(|c| c.char != ' '));
    }

    #[test]
    fn test_mode_style() {
        let insert_style = mode_style("INSERT");
        assert_eq!(insert_style.bg, Some(Color::Green));

        let normal_style = mode_style("NORMAL");
        assert_eq!(normal_style.bg, Some(Color::Blue));

        let visual_style = mode_style("VISUAL");
        assert_eq!(visual_style.bg, Some(Color::Magenta));
    }
}
