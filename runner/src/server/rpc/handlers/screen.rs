//! Screen content RPC handler.
//!
//! Handler for `state/screen_content` method.
//!
//! # Server vs TUI Rendering (#447)
//!
//! This handler provides **raw buffer content** without ANSI styling.
//! For properly rendered output with ANSI colors and syntax highlighting,
//! use `tui/capture` which relays requests to a connected TUI client.
//!
//! - **`state/screen_content`**: Raw buffer text, window layout, no styling
//! - **`tui/capture`**: Full rendered output with ANSI colors (requires TUI)
//!
//! Note: The headless server doesn't have an actual terminal screen.
//! This handler returns the content of the active buffer formatted
//! according to the requested format.
//!
//! # Multi-Window Support
//!
//! When a compositor is available, renders content for each window within its
//! bounds from the compositor's layout. Currently all windows share the active
//! buffer (vim-like behavior where splits show the same content initially).

use std::collections::HashMap;

use {
    reovim_driver_display::{BufferDecorationSourceRegistry, Decoration, LineNumberMode, Rect},
    reovim_kernel::api::v1::{BufferId, OptionRegistry, OptionScopeId, OptionValue},
    reovim_protocol::v1::{RpcError, ScreenContentResult, ScreenFormat, StateScreenContentParams},
};

use super::super::dispatcher::{HandlerFuture, RpcContext};

/// Determine line number mode from option registry (#445).
///
/// - `number` only → Absolute
/// - `relativenumber` only → Relative
/// - Both `number` and `relativenumber` → Hybrid
/// - Neither → None
fn line_number_mode_from_options(registry: &OptionRegistry) -> LineNumberMode {
    let number = registry
        .get("number", OptionScopeId::Global)
        .is_some_and(|v| matches!(v, OptionValue::Bool(true)));
    let relativenumber = registry
        .get("relativenumber", OptionScopeId::Global)
        .is_some_and(|v| matches!(v, OptionValue::Bool(true)));

    match (number, relativenumber) {
        (true, true) => LineNumberMode::Hybrid,
        (true, false) => LineNumberMode::Absolute,
        (false, true) => LineNumberMode::Relative,
        (false, false) => LineNumberMode::None,
    }
}

/// Calculate gutter width for line numbers.
fn calculate_gutter_width(mode: LineNumberMode, total_lines: usize) -> usize {
    if mode == LineNumberMode::None {
        return 0;
    }
    // Digits needed + 1 space padding
    let digits = if total_lines == 0 {
        1
    } else {
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        let d = ((total_lines as f64).log10().floor() as usize) + 1;
        d
    };
    digits + 1 // digits + space after
}

/// Format a line number based on mode.
fn format_line_number(
    mode: LineNumberMode,
    line_idx: usize,
    cursor_line: usize,
    width: usize,
) -> String {
    match mode {
        LineNumberMode::None => String::new(),
        LineNumberMode::Absolute => format!("{:>width$} ", line_idx + 1),
        LineNumberMode::Relative => {
            let rel = line_idx.abs_diff(cursor_line);
            format!("{rel:>width$} ")
        }
        LineNumberMode::Hybrid => {
            if line_idx == cursor_line {
                format!("{:>width$} ", line_idx + 1)
            } else {
                let rel = line_idx.abs_diff(cursor_line);
                format!("{rel:>width$} ")
            }
        }
    }
}

/// Create the selection style (REVERSE for visibility).
fn selection_style() -> reovim_driver_display::Style {
    use reovim_driver_display::{Attributes, Style};
    let mut attrs = Attributes::new();
    attrs.set(Attributes::REVERSE);
    Style {
        fg: None,
        bg: None,
        underline_color: None,
        attributes: attrs,
    }
}

/// Generate character-wise selection decorations.
#[allow(clippy::cast_possible_truncation)]
fn char_selection_decorations(
    start: reovim_kernel::api::v1::Position,
    end: reovim_kernel::api::v1::Position,
    style: reovim_driver_display::Style,
) -> Vec<Decoration> {
    use reovim_driver_display::Span;
    let mut decorations = Vec::new();

    if start.line == end.line {
        // Single line selection
        let span = Span::line(start.line as u32, start.column as u32, (end.column + 1) as u32);
        decorations.push(Decoration::InlineStyle { span, style });
    } else {
        // Multi-line: first line
        let first = Span::new(start.line as u32, start.column as u32, start.line as u32, u32::MAX);
        decorations.push(Decoration::InlineStyle {
            span: first,
            style: style.clone(),
        });
        // Middle lines
        for line in (start.line + 1)..end.line {
            let mid = Span::new(line as u32, 0, line as u32, u32::MAX);
            decorations.push(Decoration::InlineStyle {
                span: mid,
                style: style.clone(),
            });
        }
        // Last line
        let last = Span::line(end.line as u32, 0, (end.column + 1) as u32);
        decorations.push(Decoration::InlineStyle { span: last, style });
    }
    decorations
}

/// Generate visual selection decorations (#440).
///
/// Creates decorations for the currently selected text range in visual mode.
/// Uses REVERSE attribute for visibility (inverts fg/bg colors).
fn generate_selection_decorations(
    state: &crate::session::SessionState,
    buffer_id: BufferId,
    cursor: (usize, usize),
) -> Vec<Decoration> {
    use reovim_kernel::api::v1::{Position, Selection, SelectionMode};

    // Get selection state from buffer (read lock released immediately)
    let selection: Selection = {
        let Some(buffer_arc) = state.app.kernel.buffers.get(buffer_id) else {
            return Vec::new();
        };
        buffer_arc.read().selection()
    };

    if !selection.is_active() {
        return Vec::new();
    }

    let cursor_pos = Position::new(cursor.0, cursor.1);
    let style = selection_style();
    let mut decorations = Vec::new();

    match selection.mode() {
        SelectionMode::Character => {
            if let Some((start, end)) = selection.bounds(cursor_pos) {
                decorations = char_selection_decorations(start, end, style);
            }
        }
        SelectionMode::Line => {
            if let Some((start_line, end_line)) = selection.line_bounds(cursor_pos) {
                for line in start_line..=end_line {
                    #[allow(clippy::cast_possible_truncation)]
                    let span =
                        reovim_driver_display::Span::new(line as u32, 0, line as u32, u32::MAX);
                    decorations.push(Decoration::InlineStyle {
                        span,
                        style: style.clone(),
                    });
                }
            }
        }
        SelectionMode::Block => {
            if let Some((top_left, bottom_right)) = selection.block_bounds(cursor_pos) {
                for line in top_left.line..=bottom_right.line {
                    #[allow(clippy::cast_possible_truncation)]
                    let span = reovim_driver_display::Span::line(
                        line as u32,
                        top_left.column as u32,
                        (bottom_right.column + 1) as u32,
                    );
                    decorations.push(Decoration::InlineStyle {
                        span,
                        style: style.clone(),
                    });
                }
            }
        }
    }
    decorations
}

/// Generate decorations for a buffer from all registered decoration sources (#440).
///
/// Queries `BufferDecorationSourceRegistry` from `ServiceRegistry` and collects
/// decorations from all registered sources (e.g., rainbow brackets, search highlights).
/// Also generates visual selection decorations directly from buffer state.
/// This provides generic decoration support without depending on specific modules.
fn generate_buffer_decorations(
    state: &crate::session::SessionState,
    buffer_id: BufferId,
    content: &str,
    cursor: (usize, usize),
) -> Vec<Decoration> {
    let mut decorations = Vec::new();

    // Generate visual selection decorations first (lower priority - can be overridden)
    decorations.extend(generate_selection_decorations(state, buffer_id, cursor));

    // Query BufferDecorationSourceRegistry from ServiceRegistry
    let Some(registry) = state
        .app
        .kernel
        .services
        .get::<BufferDecorationSourceRegistry>()
    else {
        return decorations;
    };

    // Iterate ALL registered decoration sources generically
    // This follows mechanism/policy separation: the runner has no knowledge
    // of which specific modules registered which sources (pair, search, etc.)
    for key in registry.keys() {
        if let Some(source) = registry.get(&key) {
            decorations.extend(source.decorations_for_buffer(buffer_id, content, cursor));
        }
    }

    decorations
}

/// Build a decoration lookup map: (line, col) -> Style.
///
/// Used for efficient lookup when building cell grid.
/// Handles both single-line and multi-line spans (for visual selection).
fn build_decoration_map(
    decorations: &[Decoration],
) -> HashMap<(usize, usize), reovim_driver_display::Style> {
    let mut map = HashMap::new();
    for decoration in decorations {
        if let Decoration::InlineStyle { span, style } = decoration {
            let start_line = span.start_line as usize;
            let end_line = span.end_line as usize;

            for line in start_line..=end_line {
                // Determine column range for this line
                let start_col = if line == start_line {
                    span.start_col as usize
                } else {
                    0
                };
                let end_col = if line == end_line {
                    span.end_col as usize
                } else {
                    // For lines before the last, extend to a reasonable max
                    // (actual line length will clip this in rendering)
                    256
                };

                for col in start_col..end_col {
                    map.insert((line, col), style.clone());
                }
            }
        }
    }
    map
}

/// Handler for `state/screen_content` method.
///
/// Returns screen content in requested format (`plain_text`, `raw_ansi`, `cell_grid`).
///
/// # Multi-Window Support
///
/// When a compositor is available with multiple windows, renders each window's
/// content within its bounds and composes them into a single screen.
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "state/screen_content", "params": {"format": "plain_text"}}
/// ```
///
/// # Response
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "result": {"width": 80, "height": 24, "format": "plain_text", "content": "..."}}
/// ```
///
/// # Panics
///
/// This function will not panic as `ScreenContentResult` serialization is infallible.
#[must_use]
pub fn state_screen_content(ctx: RpcContext, params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let params: StateScreenContentParams =
            serde_json::from_value(params).map_err(|e| RpcError::invalid_params(e.to_string()))?;

        let result = ctx
            .session
            .with_state(|state| {
                // Use driver_session as SSOT for terminal size
                let (width, height) = state.session_terminal_size();
                let screen = Rect::new(0, 0, width, height);

                // Check if compositor has multiple windows
                if let Some(compositor) = state.driver_session.compositor() {
                    let composite = compositor.composite(screen);
                    if composite.placements.len() > 1 {
                        // Multi-window mode: render each window within its bounds
                        return render_multi_window(
                            state,
                            params.format,
                            width,
                            height,
                            &composite.placements,
                            composite.focused,
                        );
                    }
                }

                // Single-window mode (no compositor or single window)
                render_single_window(state, params.format, width, height)
            })
            .await;

        Ok(serde_json::to_value(result).expect("ScreenContentResult serialization cannot fail"))
    })
}

/// Render screen with multiple windows from compositor layout.
///
/// Each window's content is rendered within its bounds on the screen.
/// Currently all windows share the active buffer (vim-like split behavior).
///
/// The focused window is indicated with a `▪` at its top-left corner.
fn render_multi_window(
    state: &crate::session::SessionState,
    format: ScreenFormat,
    width: u16,
    height: u16,
    placements: &[reovim_driver_display::layout::WindowPlacement],
    focused: Option<reovim_driver_display::WindowId>,
) -> ScreenContentResult {
    // Get line number mode from options (#445)
    let line_mode = line_number_mode_from_options(&state.app.kernel.options);

    // Get the active buffer content (shared by all windows for now)
    let (buffer_id, content, cursor_line, cursor_col) = state
        .session_active_buffer()
        .and_then(|id: BufferId| {
            state.app.kernel.buffers.get(id).map(|arc| {
                let buf = arc.read();
                (id, buf.content(), buf.position().line, buf.position().column)
            })
        })
        .unwrap_or_default();

    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();
    let gutter_width = calculate_gutter_width(line_mode, total_lines);

    // Generate rainbow bracket decorations (#440)
    let decorations =
        generate_buffer_decorations(state, buffer_id, &content, (cursor_line, cursor_col));
    let decoration_map = build_decoration_map(&decorations);

    // Create a 2D screen buffer
    let mut screen: Vec<Vec<char>> = vec![vec![' '; width as usize]; height as usize];

    // Render each window's content within its bounds
    for placement in placements.iter().filter(|p| p.visible) {
        render_window_content(&mut screen, placement, &lines, line_mode, cursor_line, gutter_width);
    }

    // Draw window separators (borders between windows)
    draw_window_separators(&mut screen, placements, width, height);

    // Draw focus indicator at top-left of focused window
    if let Some(focused_id) = focused
        && let Some(focused_placement) = placements.iter().find(|p| p.window_id == focused_id)
    {
        let bounds = &focused_placement.bounds;
        if (bounds.y as usize) < screen.len()
            && (bounds.x as usize) < screen[bounds.y as usize].len()
        {
            screen[bounds.y as usize][bounds.x as usize] = '▪';
        }
    }

    // Convert screen buffer to string
    let screen_content: String = screen
        .iter()
        .map(|row| row.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");

    format_content(&screen_content, format, width, height, &decoration_map, gutter_width)
}

/// Render a single window's content within its bounds on the screen buffer.
///
/// Includes line numbers when `line_mode` is not `None` (#445).
fn render_window_content(
    screen: &mut [Vec<char>],
    placement: &reovim_driver_display::layout::WindowPlacement,
    lines: &[&str],
    line_mode: LineNumberMode,
    cursor_line: usize,
    gutter_width: usize,
) {
    let bounds = &placement.bounds;

    // Render buffer lines within window bounds
    for (win_row, line_idx) in (0..bounds.height).zip(0..lines.len()) {
        let screen_y = bounds.y as usize + win_row as usize;
        if screen_y >= screen.len() {
            break;
        }

        // Render line number first (#445)
        let line_num_str =
            format_line_number(line_mode, line_idx, cursor_line, gutter_width.saturating_sub(1));
        for (col, ch) in line_num_str.chars().enumerate() {
            let screen_x = bounds.x as usize + col;
            #[allow(clippy::cast_possible_truncation)]
            let col_u16 = col as u16;
            if screen_x < screen[screen_y].len() && col_u16 < bounds.width {
                screen[screen_y][screen_x] = ch;
            }
        }

        // Render content after line number
        let line = lines.get(line_idx).copied().unwrap_or("");
        let content_start = bounds.x as usize + gutter_width;
        let content_width = (bounds.width as usize).saturating_sub(gutter_width);
        for (col, ch) in line.chars().take(content_width).enumerate() {
            let screen_x = content_start + col;
            if screen_x < screen[screen_y].len() {
                screen[screen_y][screen_x] = ch;
            }
        }
    }
}

/// Draw separators between windows using box drawing characters.
///
/// Handles intersections by checking for existing separators:
/// - `│` + `─` = `┼` (cross intersection)
/// - `─` + `│` = `┼` (cross intersection)
fn draw_window_separators(
    screen: &mut [Vec<char>],
    placements: &[reovim_driver_display::layout::WindowPlacement],
    width: u16,
    height: u16,
) {
    // Find vertical and horizontal separator positions
    // Vertical separators: where window right edge meets another window's left edge
    // Horizontal separators: where window bottom edge meets another window's top edge

    for placement in placements.iter().filter(|p| p.visible) {
        let bounds = &placement.bounds;

        // Draw right border if not at screen edge
        let right_x = bounds.x + bounds.width;
        if right_x < width {
            for y in bounds.y..(bounds.y + bounds.height) {
                if (y as usize) < screen.len() && (right_x as usize) < screen[y as usize].len() {
                    // Check if this position already has a horizontal bar
                    let current = screen[y as usize][right_x as usize];
                    screen[y as usize][right_x as usize] =
                        if current == '─' { '┼' } else { '│' };
                }
            }
        }

        // Draw bottom border if not at screen edge
        let bottom_y = bounds.y + bounds.height;
        if bottom_y < height {
            for x in bounds.x..(bounds.x + bounds.width) {
                if (bottom_y as usize) < screen.len()
                    && (x as usize) < screen[bottom_y as usize].len()
                {
                    // Check if this position already has a vertical bar
                    let current = screen[bottom_y as usize][x as usize];
                    screen[bottom_y as usize][x as usize] =
                        if current == '│' { '┼' } else { '─' };
                }
            }
        }
    }
}

/// Render screen with a single window or no windows.
///
/// For backward compatibility, single-window mode returns dimensions
/// based on buffer content, not terminal size.
fn render_single_window(
    state: &crate::session::SessionState,
    format: ScreenFormat,
    _width: u16,
    _height: u16,
) -> ScreenContentResult {
    // Get line number mode from options (#445)
    let line_mode = line_number_mode_from_options(&state.app.kernel.options);

    // Use driver_session SSOT for active_buffer
    let (buffer_id, content, cursor_line, cursor_col) = state
        .session_active_buffer()
        .and_then(|id: BufferId| {
            state.app.kernel.buffers.get(id).map(|arc| {
                let buf = arc.read();
                (id, buf.content(), buf.position().line, buf.position().column)
            })
        })
        .unwrap_or_default();

    // Calculate dimensions from content
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();
    let gutter_width = calculate_gutter_width(line_mode, total_lines);

    // Generate rainbow bracket decorations (#440)
    let decorations =
        generate_buffer_decorations(state, buffer_id, &content, (cursor_line, cursor_col));
    let decoration_map = build_decoration_map(&decorations);

    // Build content with line numbers (#445)
    let rendered_content: String = if line_mode == LineNumberMode::None {
        content.clone()
    } else {
        lines
            .iter()
            .enumerate()
            .map(|(idx, line)| {
                let num =
                    format_line_number(line_mode, idx, cursor_line, gutter_width.saturating_sub(1));
                format!("{num}{line}")
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    #[allow(clippy::cast_possible_truncation)]
    let content_height = total_lines.min(usize::from(u16::MAX)) as u16;
    #[allow(clippy::cast_possible_truncation)]
    let content_width = (lines.iter().map(|l| l.len()).max().unwrap_or(80) + gutter_width)
        .min(usize::from(u16::MAX)) as u16;

    format_content(
        &rendered_content,
        format,
        content_width.max(1),
        content_height.max(1),
        &decoration_map,
        gutter_width,
    )
}

// Multi-window rendering will be implemented in Phase 2 using Session.compositor.
// The compositor provides CompositeResult with all window placements in z-order.

/// Format content according to the requested screen format.
///
/// # Arguments
///
/// * `content` - The text content to format
/// * `format` - The output format (`PlainText`, `RawAnsi`, `CellGrid`)
/// * `width` - Screen width
/// * `height` - Screen height
/// * `decoration_map` - Style decorations by position (line, col) for rainbow brackets (#440)
/// * `gutter_width` - Width of line number gutter (for column offset in decorations)
fn format_content(
    content: &str,
    format: ScreenFormat,
    width: u16,
    height: u16,
    decoration_map: &HashMap<(usize, usize), reovim_driver_display::Style>,
    gutter_width: usize,
) -> ScreenContentResult {
    let formatted_content = match format {
        ScreenFormat::PlainText => content.to_string(),
        ScreenFormat::RawAnsi => {
            // Server-side raw_ansi returns plain text (no styling).
            // For proper ANSI output with colors, use `tui/capture` (#447).
            content.to_string()
        }
        ScreenFormat::CellGrid => {
            // Return JSON cell grid representation with rainbow bracket colors (#440)
            let lines: Vec<&str> = content.lines().collect();
            let cells: Vec<Vec<serde_json::Value>> = lines
                .iter()
                .enumerate()
                .map(|(line_idx, line)| {
                    line.chars()
                        .enumerate()
                        .map(|(col_idx, c)| {
                            // Check for decoration at this position
                            // Account for gutter width when looking up decorations
                            let content_col = col_idx.saturating_sub(gutter_width);
                            let style = decoration_map.get(&(line_idx, content_col));

                            let (fg, bg, attrs) = style.map_or_else(
                                || ("default".to_string(), "default".to_string(), String::new()),
                                |s| {
                                    (
                                        color_to_string(s.fg),
                                        color_to_string(s.bg),
                                        attributes_to_string(s.attributes),
                                    )
                                },
                            );

                            if attrs.is_empty() {
                                serde_json::json!({
                                    "char": c.to_string(),
                                    "fg": fg,
                                    "bg": bg
                                })
                            } else {
                                serde_json::json!({
                                    "char": c.to_string(),
                                    "fg": fg,
                                    "bg": bg,
                                    "attrs": attrs
                                })
                            }
                        })
                        .collect()
                })
                .collect();
            serde_json::to_string(&cells).unwrap_or_else(|_| "[]".to_string())
        }
    };

    ScreenContentResult {
        width: width.max(1),
        height: height.max(1),
        format,
        content: formatted_content,
    }
}

/// Convert a Color to a string representation for JSON output.
fn color_to_string(color: Option<reovim_arch::Color>) -> String {
    match color {
        None | Some(reovim_arch::Color::Reset) => "default".to_string(),
        Some(reovim_arch::Color::Black) => "black".to_string(),
        Some(reovim_arch::Color::DarkGrey) => "#555555".to_string(),
        Some(reovim_arch::Color::Red) => "red".to_string(),
        Some(reovim_arch::Color::DarkRed) => "#aa0000".to_string(),
        Some(reovim_arch::Color::Green) => "green".to_string(),
        Some(reovim_arch::Color::DarkGreen) => "#00aa00".to_string(),
        Some(reovim_arch::Color::Yellow) => "yellow".to_string(),
        Some(reovim_arch::Color::DarkYellow) => "#aaaa00".to_string(),
        Some(reovim_arch::Color::Blue) => "blue".to_string(),
        Some(reovim_arch::Color::DarkBlue) => "#0000aa".to_string(),
        Some(reovim_arch::Color::Magenta) => "magenta".to_string(),
        Some(reovim_arch::Color::DarkMagenta) => "#aa00aa".to_string(),
        Some(reovim_arch::Color::Cyan) => "cyan".to_string(),
        Some(reovim_arch::Color::DarkCyan) => "#00aaaa".to_string(),
        Some(reovim_arch::Color::White) => "white".to_string(),
        Some(reovim_arch::Color::Grey) => "#aaaaaa".to_string(),
        Some(reovim_arch::Color::Rgb { r, g, b }) => format!("#{r:02x}{g:02x}{b:02x}"),
        Some(reovim_arch::Color::AnsiValue(n)) => format!("ansi:{n}"),
    }
}

/// Convert Attributes to a string representation for JSON output.
fn attributes_to_string(attrs: reovim_driver_display::Attributes) -> String {
    let mut parts = Vec::new();
    if attrs.contains(reovim_driver_display::Attributes::BOLD) {
        parts.push("bold");
    }
    if attrs.contains(reovim_driver_display::Attributes::ITALIC) {
        parts.push("italic");
    }
    if attrs.contains(reovim_driver_display::Attributes::UNDERLINE) {
        parts.push("underline");
    }
    if attrs.contains(reovim_driver_display::Attributes::REVERSE) {
        parts.push("reverse");
    }
    if attrs.contains(reovim_driver_display::Attributes::STRIKETHROUGH) {
        parts.push("strikethrough");
    }
    parts.join(",")
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use {
        super::*,
        crate::{
            server::rpc::handlers::test_utils::test_ctx_with_session,
            session::{Session, SessionId},
        },
        reovim_arch::sync::RwLock,
        reovim_driver_vfs::{MockVfs, VfsDriver},
        reovim_kernel::api::v1::{
            Buffer, BufferError, BufferManager, KernelContext, ModeId, ModuleId,
        },
        std::collections::HashMap,
    };

    fn test_vfs() -> Arc<dyn VfsDriver> {
        Arc::new(MockVfs::new())
    }

    /// Test-only buffer manager that actually stores buffers.
    struct TestBufferManager {
        buffers: RwLock<HashMap<BufferId, Arc<RwLock<Buffer>>>>,
    }

    impl TestBufferManager {
        fn new() -> Self {
            Self {
                buffers: RwLock::new(HashMap::new()),
            }
        }
    }

    impl BufferManager for TestBufferManager {
        fn get(&self, id: BufferId) -> Option<Arc<RwLock<Buffer>>> {
            self.buffers.read().get(&id).cloned()
        }

        fn create(&self) -> BufferId {
            let id = BufferId::new();
            let buffer = Arc::new(RwLock::new(Buffer::new()));
            self.buffers.write().insert(id, buffer);
            id
        }

        fn register(&self, buffer: Buffer) -> BufferId {
            let id = BufferId::new();
            let buffer = Arc::new(RwLock::new(buffer));
            self.buffers.write().insert(id, buffer);
            id
        }

        fn unregister(&self, id: BufferId) -> Result<Buffer, BufferError> {
            self.buffers
                .write()
                .remove(&id)
                .map_or(Err(BufferError::NotFound(id)), |arc_buffer| {
                    Arc::try_unwrap(arc_buffer)
                        .map_or_else(|arc| Ok(arc.read().clone()), |rwlock| Ok(rwlock.into_inner()))
                })
        }

        fn list(&self) -> Vec<BufferId> {
            self.buffers.read().keys().copied().collect()
        }

        fn count(&self) -> usize {
            self.buffers.read().len()
        }
    }

    /// Create a `KernelContext` with a real buffer manager for testing.
    fn test_kernel() -> KernelContext {
        use reovim_kernel::api::{
            ServiceRegistry,
            v1::{
                EventBus, MarkBank, MotionEngine, OptionRegistry, RegisterBank, TextObjectEngine,
            },
        };

        KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(RwLock::new(RegisterBank::new())),
            Arc::new(RwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::new()),
            Arc::new(ServiceRegistry::new()),
        )
    }

    fn test_session() -> Arc<Session> {
        Session::new(
            SessionId::new("test"),
            test_kernel(),
            ModeId::new(ModuleId::new("test"), "normal"),
            test_vfs(),
        )
    }

    /// Create a test session with a buffer containing the given content.
    async fn test_session_with_buffer(content: &str) -> Arc<Session> {
        let session = test_session();
        let content_owned = content.to_string();
        session
            .with_state_mut(|state| {
                let buffer = Buffer::from_string(&content_owned);
                let id = state.app.kernel.buffers.register(buffer);
                // Use driver_session as SSOT for active_buffer
                state.set_session_active_buffer(Some(id));
            })
            .await;
        session
    }

    #[tokio::test]
    async fn test_screen_content_default_format() {
        let session = test_session();
        let ctx = test_ctx_with_session(session);

        // Default format (plain_text)
        let result = state_screen_content(ctx, serde_json::json!({})).await;
        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value.get("width").is_some());
        assert!(value.get("height").is_some());
        assert!(value.get("format").is_some());
        assert!(value.get("content").is_some());
    }

    #[tokio::test]
    async fn test_screen_content_cell_grid() {
        let session = test_session();
        let ctx = test_ctx_with_session(session);

        let result = state_screen_content(ctx, serde_json::json!({"format": "cell_grid"})).await;
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value.get("format").unwrap(), "cell_grid");
    }

    #[tokio::test]
    async fn test_screen_content_with_buffer() {
        let session = test_session_with_buffer("Hello\nWorld\nTest").await;
        let ctx = test_ctx_with_session(session);

        let result = state_screen_content(ctx, serde_json::json!({"format": "plain_text"})).await;
        assert!(result.is_ok());
        let value = result.unwrap();

        // Verify dimensions match buffer content
        assert_eq!(value.get("height").unwrap().as_u64().unwrap(), 3);
        assert_eq!(value.get("width").unwrap().as_u64().unwrap(), 5); // "World" is longest

        // Verify content
        let content = value.get("content").unwrap().as_str().unwrap();
        assert!(content.contains("Hello"));
        assert!(content.contains("World"));
    }

    #[tokio::test]
    async fn test_screen_content_cell_grid_structure() {
        let session = test_session_with_buffer("AB\nCD").await;
        let ctx = test_ctx_with_session(session);

        let result = state_screen_content(ctx, serde_json::json!({"format": "cell_grid"})).await;
        assert!(result.is_ok());
        let value = result.unwrap();

        // Cell grid content should be valid JSON
        let content = value.get("content").unwrap().as_str().unwrap();
        let cells: Vec<Vec<serde_json::Value>> =
            serde_json::from_str(content).expect("Cell grid should be valid JSON");

        // Should have 2 rows
        assert_eq!(cells.len(), 2);

        // First row should have 2 cells (A, B)
        assert_eq!(cells[0].len(), 2);
        assert_eq!(cells[0][0].get("char").unwrap().as_str().unwrap(), "A");
        assert_eq!(cells[0][1].get("char").unwrap().as_str().unwrap(), "B");

        // Second row should have 2 cells (C, D)
        assert_eq!(cells[1].len(), 2);
        assert_eq!(cells[1][0].get("char").unwrap().as_str().unwrap(), "C");
        assert_eq!(cells[1][1].get("char").unwrap().as_str().unwrap(), "D");
    }

    // Multi-window separator tests

    #[test]
    fn test_draw_separators_vertical() {
        use reovim_driver_display::layout::{LayerId, WindowPlacement, ZOrder, Zone};

        let mut screen: Vec<Vec<char>> = vec![vec![' '; 10]; 5];
        let placements = vec![
            WindowPlacement::new(
                reovim_driver_display::WindowId::from_raw(1),
                LayerId::new(0),
                Zone::Tiled,
                Rect::new(0, 0, 5, 5),
                ZOrder::new(0),
            ),
            WindowPlacement::new(
                reovim_driver_display::WindowId::from_raw(2),
                LayerId::new(0),
                Zone::Tiled,
                Rect::new(5, 0, 5, 5),
                ZOrder::new(0),
            ),
        ];

        super::draw_window_separators(&mut screen, &placements, 10, 5);

        // Vertical separator at x=5 for all rows
        for (y, row) in screen.iter().enumerate().take(5) {
            assert_eq!(row[5], '│', "Vertical separator missing at row {y}");
        }
    }

    #[test]
    fn test_draw_separators_horizontal() {
        use reovim_driver_display::layout::{LayerId, WindowPlacement, ZOrder, Zone};

        let mut screen: Vec<Vec<char>> = vec![vec![' '; 10]; 6];
        let placements = vec![
            WindowPlacement::new(
                reovim_driver_display::WindowId::from_raw(1),
                LayerId::new(0),
                Zone::Tiled,
                Rect::new(0, 0, 10, 3),
                ZOrder::new(0),
            ),
            WindowPlacement::new(
                reovim_driver_display::WindowId::from_raw(2),
                LayerId::new(0),
                Zone::Tiled,
                Rect::new(0, 3, 10, 3),
                ZOrder::new(0),
            ),
        ];

        super::draw_window_separators(&mut screen, &placements, 10, 6);

        // Horizontal separator at y=3 for all columns
        for (x, &ch) in screen[3].iter().enumerate().take(10) {
            assert_eq!(ch, '─', "Horizontal separator missing at col {x}");
        }
    }

    #[test]
    fn test_draw_separators_intersection() {
        use reovim_driver_display::layout::{LayerId, WindowPlacement, ZOrder, Zone};

        let mut screen: Vec<Vec<char>> = vec![vec![' '; 10]; 6];
        // 2x2 grid of windows
        let placements = vec![
            WindowPlacement::new(
                reovim_driver_display::WindowId::from_raw(1),
                LayerId::new(0),
                Zone::Tiled,
                Rect::new(0, 0, 5, 3), // Top-left
                ZOrder::new(0),
            ),
            WindowPlacement::new(
                reovim_driver_display::WindowId::from_raw(2),
                LayerId::new(0),
                Zone::Tiled,
                Rect::new(5, 0, 5, 3), // Top-right
                ZOrder::new(0),
            ),
            WindowPlacement::new(
                reovim_driver_display::WindowId::from_raw(3),
                LayerId::new(0),
                Zone::Tiled,
                Rect::new(0, 3, 5, 3), // Bottom-left
                ZOrder::new(0),
            ),
            WindowPlacement::new(
                reovim_driver_display::WindowId::from_raw(4),
                LayerId::new(0),
                Zone::Tiled,
                Rect::new(5, 3, 5, 3), // Bottom-right
                ZOrder::new(0),
            ),
        ];

        super::draw_window_separators(&mut screen, &placements, 10, 6);

        // Intersection at (5, 3) should be '┼'
        assert_eq!(screen[3][5], '┼', "Intersection should be cross, got '{}'", screen[3][5]);

        // Vertical separators at x=5
        for y in [0, 1, 2, 4, 5] {
            assert_eq!(screen[y][5], '│', "Vertical separator missing at row {y}");
        }

        // Horizontal separators at y=3
        for x in [0, 1, 2, 3, 4, 6, 7, 8, 9] {
            assert_eq!(screen[3][x], '─', "Horizontal separator missing at col {x}");
        }
    }

    #[test]
    fn test_format_content_plain_text() {
        let content = "Hello\nWorld";
        let decoration_map = HashMap::new();
        let result =
            super::format_content(content, ScreenFormat::PlainText, 10, 5, &decoration_map, 0);

        assert_eq!(result.content, "Hello\nWorld");
        assert_eq!(result.width, 10);
        assert_eq!(result.height, 5);
        assert_eq!(result.format, ScreenFormat::PlainText);
    }

    #[test]
    fn test_format_content_cell_grid() {
        let content = "AB";
        let decoration_map = HashMap::new();
        let result =
            super::format_content(content, ScreenFormat::CellGrid, 10, 5, &decoration_map, 0);

        // Should be valid JSON
        let cells: Vec<Vec<serde_json::Value>> =
            serde_json::from_str(&result.content).expect("Should be valid JSON");
        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0].len(), 2);
        assert_eq!(cells[0][0].get("char").unwrap().as_str().unwrap(), "A");
        assert_eq!(cells[0][1].get("char").unwrap().as_str().unwrap(), "B");
    }

    #[test]
    fn test_format_content_cell_grid_with_decorations() {
        use reovim_driver_display::{Color, Style};

        // Create content with a bracket
        let content = "(A)";

        // Create a decoration map with color at position (0, 0) for the '('
        let mut decoration_map = HashMap::new();
        let style = Style {
            fg: Some(Color::Red),
            bg: None,
            underline_color: None,
            attributes: reovim_driver_display::Attributes::default(),
        };
        decoration_map.insert((0, 0), style);

        let result =
            super::format_content(content, ScreenFormat::CellGrid, 10, 5, &decoration_map, 0);

        // Parse the JSON
        let cells: Vec<Vec<serde_json::Value>> =
            serde_json::from_str(&result.content).expect("Should be valid JSON");

        // First character '(' should have red foreground
        let first_cell = &cells[0][0];
        assert_eq!(first_cell.get("char").unwrap().as_str().unwrap(), "(");
        assert_eq!(first_cell.get("fg").unwrap().as_str().unwrap(), "red");
        assert_eq!(first_cell.get("bg").unwrap().as_str().unwrap(), "default");

        // Second character 'A' should have default colors
        let second_cell = &cells[0][1];
        assert_eq!(second_cell.get("char").unwrap().as_str().unwrap(), "A");
        assert_eq!(second_cell.get("fg").unwrap().as_str().unwrap(), "default");

        // Third character ')' should have default colors
        let third_cell = &cells[0][2];
        assert_eq!(third_cell.get("char").unwrap().as_str().unwrap(), ")");
        assert_eq!(third_cell.get("fg").unwrap().as_str().unwrap(), "default");
    }

    // === Line number rendering tests (#445) ===

    #[test]
    fn test_line_number_mode_from_options_none() {
        use reovim_kernel::api::v1::OptionRegistry;
        let registry = OptionRegistry::new();
        let mode = super::line_number_mode_from_options(&registry);
        assert_eq!(mode, LineNumberMode::None);
    }

    #[test]
    fn test_line_number_mode_from_options_absolute() {
        use reovim_kernel::api::v1::{OptionRegistry, OptionScope, OptionSpec, OptionValue};
        let registry = OptionRegistry::new();
        registry
            .register(
                OptionSpec::new("number", "Line numbers", OptionValue::bool(true))
                    .with_scope(OptionScope::Window),
            )
            .unwrap();
        let mode = super::line_number_mode_from_options(&registry);
        assert_eq!(mode, LineNumberMode::Absolute);
    }

    #[test]
    fn test_line_number_mode_from_options_relative() {
        use reovim_kernel::api::v1::{OptionRegistry, OptionScope, OptionSpec, OptionValue};
        let registry = OptionRegistry::new();
        registry
            .register(
                OptionSpec::new("relativenumber", "Relative numbers", OptionValue::bool(true))
                    .with_scope(OptionScope::Window),
            )
            .unwrap();
        let mode = super::line_number_mode_from_options(&registry);
        assert_eq!(mode, LineNumberMode::Relative);
    }

    #[test]
    fn test_line_number_mode_from_options_hybrid() {
        use reovim_kernel::api::v1::{OptionRegistry, OptionScope, OptionSpec, OptionValue};
        let registry = OptionRegistry::new();
        registry
            .register(
                OptionSpec::new("number", "Line numbers", OptionValue::bool(true))
                    .with_scope(OptionScope::Window),
            )
            .unwrap();
        registry
            .register(
                OptionSpec::new("relativenumber", "Relative numbers", OptionValue::bool(true))
                    .with_scope(OptionScope::Window),
            )
            .unwrap();
        let mode = super::line_number_mode_from_options(&registry);
        assert_eq!(mode, LineNumberMode::Hybrid);
    }

    #[test]
    fn test_calculate_gutter_width_none_mode() {
        assert_eq!(super::calculate_gutter_width(LineNumberMode::None, 100), 0);
    }

    #[test]
    fn test_calculate_gutter_width_small_file() {
        // 9 lines = 1 digit + 1 space = 2
        assert_eq!(super::calculate_gutter_width(LineNumberMode::Absolute, 9), 2);
    }

    #[test]
    fn test_calculate_gutter_width_medium_file() {
        // 99 lines = 2 digits + 1 space = 3
        assert_eq!(super::calculate_gutter_width(LineNumberMode::Absolute, 99), 3);
    }

    #[test]
    fn test_calculate_gutter_width_large_file() {
        // 999 lines = 3 digits + 1 space = 4
        assert_eq!(super::calculate_gutter_width(LineNumberMode::Absolute, 999), 4);
    }

    #[test]
    fn test_format_line_number_absolute() {
        let s = super::format_line_number(LineNumberMode::Absolute, 0, 0, 2);
        assert_eq!(s, " 1 ");
        let s = super::format_line_number(LineNumberMode::Absolute, 9, 0, 2);
        assert_eq!(s, "10 ");
    }

    #[test]
    fn test_format_line_number_relative() {
        // Cursor on line 5, checking line 3 (distance = 2)
        let s = super::format_line_number(LineNumberMode::Relative, 3, 5, 2);
        assert_eq!(s, " 2 ");
        // Cursor line shows 0
        let s = super::format_line_number(LineNumberMode::Relative, 5, 5, 2);
        assert_eq!(s, " 0 ");
    }

    #[test]
    fn test_format_line_number_hybrid() {
        // Cursor line shows absolute number
        let s = super::format_line_number(LineNumberMode::Hybrid, 5, 5, 2);
        assert_eq!(s, " 6 ");
        // Non-cursor lines show relative
        let s = super::format_line_number(LineNumberMode::Hybrid, 3, 5, 2);
        assert_eq!(s, " 2 ");
    }

    #[test]
    fn test_format_line_number_none() {
        let s = super::format_line_number(LineNumberMode::None, 0, 0, 2);
        assert!(s.is_empty());
    }
}
