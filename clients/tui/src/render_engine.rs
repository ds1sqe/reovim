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

use {
    reovim_arch::Color,
    reovim_driver_display::{
        Style,
        ui::{display_width, truncate_end},
    },
};

use crate::{LineNumberMode, SelectionState, TuiCoreState, render_backend::RenderBackend};

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

/// Dimmed CBF-8 palette for selection backgrounds.
///
/// Lower-intensity versions of the cursor palette, suitable for
/// background overlays that don't obscure text.
const CBF8_DIMMED: [Color; 8] = [
    Color::Rgb { r: 0, g: 45, b: 70 }, // Blue
    Color::Rgb { r: 75, g: 50, b: 0 }, // Orange
    Color::Rgb {
        r: 25,
        g: 60,
        b: 75,
    }, // Sky blue
    Color::Rgb { r: 0, g: 55, b: 35 }, // Green
    Color::Rgb {
        r: 70,
        g: 65,
        b: 20,
    }, // Yellow
    Color::Rgb { r: 70, g: 30, b: 0 }, // Vermilion
    Color::Rgb {
        r: 65,
        g: 38,
        b: 55,
    }, // Pink
    Color::Rgb {
        r: 30,
        g: 30,
        b: 30,
    }, // Dark grey (fallback)
];

/// Get a dimmed color for selection backgrounds.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub const fn dimmed_client_color(client_id: u64) -> Color {
    CBF8_DIMMED[(client_id as usize) % CBF8_DIMMED.len()]
}

/// Local selection background color (subtle blue).
const LOCAL_SELECTION_BG: Color = Color::Rgb {
    r: 50,
    g: 50,
    b: 100,
};

/// Render a complete frame to the backend.
///
/// This is the single entry point for all TUI rendering.
/// Both interactive and headless TUIs call this function.
///
/// # Current Implementation
///
/// This is a basic implementation that renders:
/// - Buffer content (simple text, no syntax highlighting yet)
/// - Remote selections (dimmed background overlay)
/// - Local selection (headless mode only)
/// - Remote cursors (CBF-8 colorblind-friendly palette)
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

    // Render selections (behind cursors — background overlay)
    render_remote_selections(backend, state, config.gutter_width, content_height);
    if config.render_self_cursor {
        render_local_selection(backend, state, config.gutter_width, content_height);
    }

    // Render cursors (on top of selections)
    render_remote_cursors(backend, state, config.gutter_width, content_height);
    render_remote_cursor_labels(backend, state, config.gutter_width, content_height);
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

    // TODO(#494): Multi-window — iterate all windows with tiling layout
    let buffer_id = state.windows.first().and_then(|w| w.buffer_id);

    let lines = buffer_id.and_then(|id| state.buffer_cache.get(&id));

    // TODO(#494): Per-window scroll tracking — compute from cursor position
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
    // TODO(#494): Syntax highlighting — accept token spans from tree-sitter driver
    let style = Style::default();

    for (col, ch) in line.chars().enumerate() {
        let col = col as u16;
        if col >= width {
            break;
        }
        backend.set_cell(x + col, y, ch, &style);
    }
}

/// Render remote clients' visual selections.
///
/// Overlays dimmed background colors on selected ranges. Rendered before
/// cursors so cursors appear on top.
#[allow(clippy::cast_possible_truncation)]
fn render_remote_selections<B: RenderBackend>(
    backend: &mut B,
    state: &TuiCoreState,
    gutter_width: u16,
    content_height: u16,
) {
    let (width, _) = backend.size();

    // TODO(#494): Multi-window — iterate all windows with tiling layout
    let current_buffer_id = state.windows.first().and_then(|w| w.buffer_id);

    for remote in state.other_clients.values() {
        if remote.buffer_id != current_buffer_id {
            continue;
        }
        let Some(sel) = &remote.selection else {
            continue;
        };

        let sel_color = dimmed_client_color(remote.client_id);
        render_selection_range(backend, sel, sel_color, gutter_width, content_height, width);
    }
}

/// Render local client's visual selection (headless mode only).
///
/// Interactive TUI uses terminal-level selection highlighting via crossterm.
#[allow(clippy::cast_possible_truncation)]
fn render_local_selection<B: RenderBackend>(
    backend: &mut B,
    state: &TuiCoreState,
    gutter_width: u16,
    content_height: u16,
) {
    let (width, _) = backend.size();
    let Some(sel) = state.window_selections.get(&state.focused_window_id) else {
        return;
    };

    render_selection_range(backend, sel, LOCAL_SELECTION_BG, gutter_width, content_height, width);
}

/// Render a selection range with a background color overlay.
///
/// Handles three visual modes:
/// - **char**: Contiguous character range (first/last line partial, middle lines full)
/// - **line**: Entire lines highlighted
/// - **block**: Rectangular column range on each line
#[allow(clippy::cast_possible_truncation)]
fn render_selection_range<B: RenderBackend>(
    backend: &mut B,
    sel: &SelectionState,
    color: Color,
    gutter_width: u16,
    content_height: u16,
    screen_width: u16,
) {
    let (start_line, start_col, end_line, end_col) = normalize_selection(sel);
    let content_width = screen_width.saturating_sub(gutter_width);

    for line in start_line..=end_line {
        if line as u16 >= content_height {
            break;
        }

        let (col_start, col_end) = match sel.mode.as_str() {
            "line" => (0u16, content_width),
            "block" => (start_col as u16, end_col as u16 + 1),
            _ => {
                // Char mode
                if start_line == end_line {
                    (start_col as u16, end_col as u16 + 1)
                } else if line == start_line {
                    (start_col as u16, content_width)
                } else if line == end_line {
                    (0, end_col as u16 + 1)
                } else {
                    (0, content_width)
                }
            }
        };

        let screen_y = line as u16;
        for col in col_start..col_end.min(content_width) {
            backend.overlay_bg(gutter_width + col, screen_y, color);
        }
    }
}

/// Normalize selection so start <= end.
fn normalize_selection(sel: &SelectionState) -> (u64, u64, u64, u64) {
    if (sel.start.line, sel.start.column) <= (sel.end.line, sel.end.column) {
        (sel.start.line, sel.start.column, sel.end.line, sel.end.column)
    } else {
        (sel.end.line, sel.end.column, sel.start.line, sel.start.column)
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

        // TODO(#494): Adjust remote cursor for scroll offset
        let screen_line = remote.cursor_line;
        let screen_col = remote.cursor_col as u16 + gutter_width;

        if screen_line < u64::from(content_height) && screen_col < width {
            #[allow(clippy::cast_possible_truncation)]
            let screen_y = screen_line as u16;
            backend.apply_style(screen_col, screen_y, &cursor_style);
        }
    }
}

/// Maximum display width for cursor label names.
const MAX_LABEL_WIDTH: usize = 16;

/// Prepare label text from a display name.
///
/// Returns a padded, truncated label string. Empty names show `" ? "`.
fn label_text(display_name: &str) -> String {
    if display_name.is_empty() {
        return " ? ".to_string();
    }
    format!(" {} ", truncate_end(display_name, MAX_LABEL_WIDTH))
}

/// Render floating name labels for remote cursors.
///
/// Shows a colored tag with the client's display name near each remote
/// cursor. Labels appear one line above the cursor, or one line below
/// if the cursor is at line 0. Skipped if neither position is in viewport.
#[allow(clippy::cast_possible_truncation)]
fn render_remote_cursor_labels<B: RenderBackend>(
    backend: &mut B,
    state: &TuiCoreState,
    gutter_width: u16,
    content_height: u16,
) {
    let (width, _) = backend.size();

    let current_buffer_id = state.windows.first().and_then(|w| w.buffer_id);

    for remote in state.other_clients.values() {
        if remote.buffer_id != current_buffer_id {
            continue;
        }

        let cursor_line = remote.cursor_line;

        // Determine label y position: prefer above, fallback below
        let label_y = if cursor_line > 0 {
            (cursor_line - 1) as u16
        } else if cursor_line + 1 < u64::from(content_height) {
            (cursor_line + 1) as u16
        } else {
            continue; // No room (single-line viewport)
        };

        if label_y >= content_height {
            continue;
        }

        let label = label_text(&remote.display_name);
        let label_width = display_width(&label) as u16;

        // Skip if terminal is too narrow for the label
        if label_width + gutter_width > width {
            continue;
        }

        // Clamp x position: start at cursor column, keep label within screen
        let label_x = (remote.cursor_col as u16 + gutter_width)
            .min(width.saturating_sub(label_width))
            .max(gutter_width);

        let label_color = client_color(remote.client_id);
        let label_style = Style::default().bg(label_color).fg(Color::White);

        // Use apply_style to overlay colored background without hiding buffer text.
        // Characters underneath remain visible (white on colored bg).
        for i in 0..label_width {
            let x = label_x + i;
            if x < width {
                backend.apply_style(x, label_y, &label_style);
            }
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

    // TODO(#494): Adjust self cursor for scroll offset
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
// TODO(#494): Theme integration — use theme palette instead of hardcoded colors
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
    use {
        super::*,
        crate::{CursorPosition, RemoteClient},
        reovim_driver_display::FrameBuffer,
        reovim_protocol::v2::WindowInfo,
    };

    /// Helper: create a `WindowInfo` with a buffer.
    fn window(id: u64, buffer_id: u64) -> WindowInfo {
        WindowInfo {
            window_id: id,
            buffer_id: Some(buffer_id),
            rect: None,
            focused: true,
        }
    }

    /// Helper: create a `SelectionState`.
    fn selection(
        start_line: u64,
        start_col: u64,
        end_line: u64,
        end_col: u64,
        mode: &str,
    ) -> SelectionState {
        SelectionState {
            start: CursorPosition {
                line: start_line,
                column: start_col,
            },
            end: CursorPosition {
                line: end_line,
                column: end_col,
            },
            mode: mode.to_string(),
        }
    }

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
    fn test_dimmed_client_color() {
        let d0 = dimmed_client_color(0);
        let d1 = dimmed_client_color(1);
        let d8 = dimmed_client_color(8); // Should wrap to d0

        assert_ne!(d0, d1);
        assert_eq!(d0, d8);

        // Dimmed color should differ from full-intensity color
        let c0 = client_color(0);
        assert_ne!(d0, c0);
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

    #[test]
    fn test_normalize_selection_already_ordered() {
        let sel = selection(1, 5, 3, 10, "char");
        let (sl, sc, el, ec) = normalize_selection(&sel);
        assert_eq!((sl, sc, el, ec), (1, 5, 3, 10));
    }

    #[test]
    fn test_normalize_selection_reversed() {
        let sel = selection(5, 10, 2, 3, "char");
        let (sl, sc, el, ec) = normalize_selection(&sel);
        assert_eq!((sl, sc, el, ec), (2, 3, 5, 10));
    }

    #[test]
    fn test_render_char_selection() {
        // Render a char-mode selection from (0,2) to (0,5) on a single line.
        let mut fb = FrameBuffer::new(40, 10);
        let mut state = TuiCoreState::new(1);
        state.windows.push(window(1, 100));
        state.focused_window_id = 1;

        // Add remote client in same buffer with a char selection
        let remote = RemoteClient {
            client_id: 2,
            display_name: "Remote".to_string(),
            cursor_line: 0,
            cursor_col: 5,
            buffer_id: Some(100),
            mode: "VISUAL".to_string(),
            selection: Some(selection(0, 2, 0, 5, "char")),
        };
        state.add_remote_client(remote);

        let config = RenderConfig::default();
        render_frame(&mut fb, &state, &config);

        // Columns 2..=4 should have dimmed selection bg.
        // Column 5 is the cursor position — cursor overwrites selection bg.
        let expected_bg = Some(dimmed_client_color(2));
        for col in 2..=4u16 {
            let cell = fb.get(col, 0).unwrap();
            assert_eq!(cell.style.bg, expected_bg, "col {col} should have selection bg");
        }

        // Column 0 should NOT have selection bg
        let before = fb.get(0, 0).unwrap();
        assert_ne!(before.style.bg, expected_bg, "col 0 should not have selection bg");
    }

    #[test]
    fn test_render_line_selection() {
        // Render a line-mode selection spanning lines 1..=2.
        let mut fb = FrameBuffer::new(40, 10);
        let mut state = TuiCoreState::new(1);
        state.windows.push(window(1, 100));
        state.focused_window_id = 1;

        state.add_remote_client(RemoteClient {
            client_id: 3,
            display_name: "Remote".to_string(),
            cursor_line: 2,
            cursor_col: 0,
            buffer_id: Some(100),
            mode: "VISUAL LINE".to_string(),
            selection: Some(selection(1, 0, 2, 5, "line")),
        });

        let config = RenderConfig::default();
        render_frame(&mut fb, &state, &config);

        let expected_bg = Some(dimmed_client_color(3));

        // Entire line 1 and 2 should be highlighted (col 0..40).
        // Cursor is at (2, 0), so skip that exact cell.
        // Label " Remote " (8 chars) renders on row 1 at col 0, overwriting selection bg.
        for row in 1..=2u16 {
            // Use col 20 (safely past label region) to check selection bg
            let cell_mid = fb.get(20, row).unwrap();
            assert_eq!(
                cell_mid.style.bg, expected_bg,
                "row {row}, col 20 should have line selection bg"
            );
        }

        // Row 0 should NOT be highlighted
        let above = fb.get(0, 0).unwrap();
        assert_ne!(above.style.bg, expected_bg, "row 0 should not have selection bg");
    }

    #[test]
    fn test_render_block_selection() {
        // Render a block-mode selection: columns 3..=7 on lines 0..=2.
        let mut fb = FrameBuffer::new(40, 10);
        let mut state = TuiCoreState::new(1);
        state.windows.push(window(1, 100));
        state.focused_window_id = 1;

        state.add_remote_client(RemoteClient {
            client_id: 4,
            display_name: "Remote".to_string(),
            cursor_line: 2,
            cursor_col: 7,
            buffer_id: Some(100),
            mode: "VISUAL BLOCK".to_string(),
            selection: Some(selection(0, 3, 2, 7, "block")),
        });

        let config = RenderConfig::default();
        render_frame(&mut fb, &state, &config);

        let expected_bg = Some(dimmed_client_color(4));

        // Columns 3..=7 on rows 0..=2 should be highlighted.
        // Cursor is at (2, 7), so skip that cell.
        // Label " Remote " (8 chars) renders on row 1 at col 7, overwriting selection bg there.
        for row in 0..=2u16 {
            for col in 3..=7u16 {
                if row == 2 && col == 7 {
                    continue; // cursor overwrites selection bg here
                }
                if row == 1 && col == 7 {
                    continue; // label overwrites selection bg here
                }
                let cell = fb.get(col, row).unwrap();
                assert_eq!(
                    cell.style.bg, expected_bg,
                    "row {row}, col {col} should have block selection bg"
                );
            }
        }

        // Column 2 on row 0 should NOT be highlighted
        let before = fb.get(2, 0).unwrap();
        assert_ne!(before.style.bg, expected_bg, "col 2 should not have block bg");

        // Column 8 on row 0 should NOT be highlighted
        let after = fb.get(8, 0).unwrap();
        assert_ne!(after.style.bg, expected_bg, "col 8 should not have block bg");
    }

    #[test]
    fn test_render_remote_selection_different_buffer() {
        // Remote client in a different buffer — no selection rendered.
        let mut fb = FrameBuffer::new(40, 10);
        let mut state = TuiCoreState::new(1);
        state.windows.push(window(1, 100));
        state.focused_window_id = 1;

        state.add_remote_client(RemoteClient {
            client_id: 5,
            display_name: "Remote".to_string(),
            cursor_line: 0,
            cursor_col: 0,
            buffer_id: Some(999), // Different buffer
            mode: "VISUAL".to_string(),
            selection: Some(selection(0, 0, 0, 10, "char")),
        });

        let config = RenderConfig::default();
        render_frame(&mut fb, &state, &config);

        // No selection background should appear — cells should have default bg
        let wrong_bg = Some(dimmed_client_color(5));
        for col in 0..=10u16 {
            let cell = fb.get(col, 0).unwrap();
            assert_ne!(
                cell.style.bg, wrong_bg,
                "col {col} should NOT have selection bg (different buffer)"
            );
        }
    }

    #[test]
    fn test_render_local_selection() {
        // Local visual selection rendered with LOCAL_SELECTION_BG.
        let mut fb = FrameBuffer::new(40, 10);
        let mut state = TuiCoreState::new(1);
        state.windows.push(window(1, 100));
        state.focused_window_id = 1;
        state
            .window_selections
            .insert(1, selection(0, 3, 0, 8, "char"));

        // Must enable render_self_cursor for local selection rendering
        let config = RenderConfig {
            render_self_cursor: true,
            ..RenderConfig::default()
        };
        render_frame(&mut fb, &state, &config);

        let expected_bg = Some(LOCAL_SELECTION_BG);
        for col in 3..=8u16 {
            let cell = fb.get(col, 0).unwrap();
            assert_eq!(cell.style.bg, expected_bg, "col {col} should have local selection bg");
        }

        // Column 2 should NOT have selection bg
        let before = fb.get(2, 0).unwrap();
        assert_ne!(before.style.bg, expected_bg, "col 2 should not have local selection bg");
    }

    #[test]
    fn test_render_multiline_char_selection() {
        // Multi-line char selection: first line partial start, middle full, last line partial end.
        let mut fb = FrameBuffer::new(20, 10);
        let mut state = TuiCoreState::new(1);
        state.windows.push(window(1, 100));
        state.focused_window_id = 1;

        state.add_remote_client(RemoteClient {
            client_id: 6,
            display_name: "Remote".to_string(),
            cursor_line: 2,
            cursor_col: 5,
            buffer_id: Some(100),
            mode: "VISUAL".to_string(),
            selection: Some(selection(0, 10, 2, 5, "char")),
        });

        let config = RenderConfig::default();
        render_frame(&mut fb, &state, &config);

        let expected_bg = Some(dimmed_client_color(6));

        // Row 0: columns 10..end should be highlighted (first line, from start_col)
        let cell_before = fb.get(9, 0).unwrap();
        assert_ne!(cell_before.style.bg, expected_bg, "row 0 col 9 should not be highlighted");
        let cell_start = fb.get(10, 0).unwrap();
        assert_eq!(cell_start.style.bg, expected_bg, "row 0 col 10 should be highlighted");

        // Row 1: entire line should be highlighted (middle line)
        let cell_mid = fb.get(0, 1).unwrap();
        assert_eq!(cell_mid.style.bg, expected_bg, "row 1 col 0 should be highlighted");

        // Row 2: columns 0..=5 should be highlighted (last line, up to end_col).
        // Cursor at (2, 5), so check col 4 instead of col 5.
        let cell_end = fb.get(4, 2).unwrap();
        assert_eq!(cell_end.style.bg, expected_bg, "row 2 col 4 should be highlighted");
        let cell_after = fb.get(6, 2).unwrap();
        assert_ne!(cell_after.style.bg, expected_bg, "row 2 col 6 should not be highlighted");
    }

    // ── Cursor Label Tests ──────────────────────────────────────────

    #[test]
    fn test_remote_cursor_label_rendered() {
        let mut fb = FrameBuffer::new(80, 24);
        let mut state = TuiCoreState::new(1);
        state.windows.push(window(1, 100));
        state.focused_window_id = 1;

        state.add_remote_client(RemoteClient {
            client_id: 2,
            display_name: "alice".to_string(),
            cursor_line: 3,
            cursor_col: 5,
            buffer_id: Some(100),
            mode: "NORMAL".to_string(),
            selection: None,
        });

        let config = RenderConfig::default();
        render_frame(&mut fb, &state, &config);

        // Label is a background overlay on row 2 (one above cursor line 3).
        // Characters are preserved; only bg+fg change.
        let expected_bg = Some(client_color(2));
        let label_x = 5u16; // cursor_col
        #[allow(clippy::cast_possible_truncation)]
        let label_width = display_width(&label_text("alice")) as u16;

        for i in 0..label_width {
            let cell = fb.get(label_x + i, 2).unwrap();
            assert_eq!(cell.style.bg, expected_bg, "col {} should have label bg", label_x + i);
        }

        // Cell before label should NOT have label bg
        if label_x > 0 {
            let before = fb.get(label_x - 1, 2).unwrap();
            assert_ne!(before.style.bg, expected_bg, "cell before label should not have label bg");
        }
    }

    #[test]
    fn test_remote_cursor_label_at_line_zero() {
        let mut fb = FrameBuffer::new(80, 24);
        let mut state = TuiCoreState::new(1);
        state.windows.push(window(1, 100));
        state.focused_window_id = 1;

        state.add_remote_client(RemoteClient {
            client_id: 3,
            display_name: "bob".to_string(),
            cursor_line: 0,
            cursor_col: 0,
            buffer_id: Some(100),
            mode: "NORMAL".to_string(),
            selection: None,
        });

        let config = RenderConfig::default();
        render_frame(&mut fb, &state, &config);

        // Label bg should appear on row 1 (one below cursor line 0)
        let expected_bg = Some(client_color(3));
        #[allow(clippy::cast_possible_truncation)]
        let label_width = display_width(&label_text("bob")) as u16;

        for i in 0..label_width {
            let cell = fb.get(i, 1).unwrap();
            assert_eq!(cell.style.bg, expected_bg, "col {i} on row 1 should have label bg");
        }
    }

    #[test]
    fn test_remote_cursor_label_truncated() {
        let mut fb = FrameBuffer::new(80, 24);
        let mut state = TuiCoreState::new(1);
        state.windows.push(window(1, 100));
        state.focused_window_id = 1;

        state.add_remote_client(RemoteClient {
            client_id: 4,
            display_name: "very-long-username-that-exceeds-limit".to_string(),
            cursor_line: 5,
            cursor_col: 0,
            buffer_id: Some(100),
            mode: "NORMAL".to_string(),
            selection: None,
        });

        let config = RenderConfig::default();
        render_frame(&mut fb, &state, &config);

        // Label bg width should be based on truncated name, not the full name.
        let expected_bg = Some(client_color(4));
        let truncated_label = label_text("very-long-username-that-exceeds-limit");
        #[allow(clippy::cast_possible_truncation)]
        let label_width = display_width(&truncated_label) as u16;

        // Full name would be 39 chars; truncated should be <= MAX_LABEL_WIDTH + 2 + 3
        assert!(label_width < 39, "Label should be truncated, got width {label_width}");

        // Verify bg applied for the truncated label width
        for i in 0..label_width {
            let cell = fb.get(i, 4).unwrap();
            assert_eq!(cell.style.bg, expected_bg, "col {i} should have label bg");
        }
    }

    #[test]
    fn test_remote_cursor_label_clamped_to_screen() {
        let mut fb = FrameBuffer::new(30, 10);
        let mut state = TuiCoreState::new(1);
        state.windows.push(window(1, 100));
        state.focused_window_id = 1;

        // Cursor near right edge
        state.add_remote_client(RemoteClient {
            client_id: 5,
            display_name: "charlie".to_string(),
            cursor_line: 3,
            cursor_col: 25,
            buffer_id: Some(100),
            mode: "NORMAL".to_string(),
            selection: None,
        });

        let config = RenderConfig::default();
        render_frame(&mut fb, &state, &config);

        // Label bg should be visible (clamped to fit within 30 columns)
        let expected_bg = Some(client_color(5));
        #[allow(clippy::cast_possible_truncation)]
        let label_width = display_width(&label_text("charlie")) as u16;

        // Label clamped: starts at min(25, 30 - label_width), must end before col 30
        let mut found_label_cells = 0u16;
        for col in 0..30u16 {
            if fb.get(col, 2).map(|c| c.style.bg) == Some(expected_bg) {
                found_label_cells += 1;
            }
        }
        assert_eq!(
            found_label_cells, label_width,
            "Should find exactly {label_width} label cells on row 2"
        );
    }

    #[test]
    fn test_remote_cursor_label_different_buffer_not_shown() {
        let mut fb = FrameBuffer::new(80, 24);
        let mut state = TuiCoreState::new(1);
        state.windows.push(window(1, 100));
        state.focused_window_id = 1;

        state.add_remote_client(RemoteClient {
            client_id: 6,
            display_name: "eve".to_string(),
            cursor_line: 3,
            cursor_col: 0,
            buffer_id: Some(999), // Different buffer
            mode: "NORMAL".to_string(),
            selection: None,
        });

        let config = RenderConfig::default();
        render_frame(&mut fb, &state, &config);

        // No label bg should appear anywhere (client is in a different buffer)
        let label_bg = Some(client_color(6));
        for y in 0..23u16 {
            for x in 0..80u16 {
                let cell = fb.get(x, y).unwrap();
                assert_ne!(
                    cell.style.bg, label_bg,
                    "No cell should have label bg at ({x}, {y}) for different-buffer client"
                );
            }
        }
    }

    #[test]
    fn test_label_text_helper() {
        // Normal name
        assert_eq!(label_text("alice"), " alice ");

        // Empty name
        assert_eq!(label_text(""), " ? ");

        // Long name triggers truncation
        let long = label_text("very-long-username-that-exceeds");
        assert!(long.contains("..."), "Long name should be truncated with '...'");
        assert!(long.starts_with(' '), "Label should have leading space");
        assert!(long.ends_with(' '), "Label should have trailing space");

        // Exact limit (16 chars)
        assert_eq!(label_text("exactly16chars!!"), " exactly16chars!! ");
    }
}
