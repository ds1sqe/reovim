//! Rendering pipeline for screen windows
//!
//! This module contains the core rendering logic for transforming buffer content
//! into visual output on the terminal screen. The rendering pipeline consists of:
//! 1. `execute_pipeline()` - Orchestrates render stages (syntax, decorations, etc.)
//! 2. `calculate_scroll_adjustment_for_virtual_lines()` - Adjusts scroll for virtual lines
//! 3. `render_data_to_framebuffer()` - Core rendering loop that processes lines
//! 4. `render_line_number_to_buffer_simple()` - Renders line numbers with proper styling
//! 5. `render_windows()` - Main entry point that renders all windows to the frame buffer

use crate::{
    buffer::Buffer,
    command_line::CommandLine,
    decoration::DecorationStore,
    frame::FrameBuffer,
    highlight::{ColorMode, HighlightStore, Style, Theme},
    indent::IndentAnalyzer,
    modd::{ComponentId, ModeState},
    modifier::{ModifierContext, ModifierRegistry},
    screen::Position,
    sign::Sign,
    visibility::BufferVisibilitySource,
};

use super::{
    Screen, ViewportScrollInfo,
    window::{LineNumber, SignColumnMode, Window},
};

use reovim_sys::{
    cursor::{Hide, MoveTo, Show},
    queue,
};

use std::{collections::BTreeMap, io::Write};

impl Screen {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn execute_pipeline(
        &self,
        window: &Window,
        text_buffer: &Buffer,
        _highlight_store: &HighlightStore,
        theme: &Theme,
        color_mode: ColorMode,
        _visibility_source: &dyn BufferVisibilitySource,
        _indent_analyzer: &IndentAnalyzer,
        _decoration_store: Option<&DecorationStore>,
        render_stages: &std::sync::Arc<std::sync::RwLock<crate::render::RenderStageRegistry>>,
        mode: &crate::modd::ModeState,
    ) -> crate::render::RenderData {
        use crate::{component::RenderContext, render::RenderData};

        let pipeline_start = std::time::Instant::now();

        // Stage 1: Extract buffer content
        let mut data = RenderData::from_buffer(window, text_buffer, mode);
        data.buffer_id = window.buffer_id().unwrap_or(0);
        let from_buffer_time = pipeline_start.elapsed();

        // Create render context
        let ctx = RenderContext::new(self.size.width, self.size.height, theme, color_mode);

        // Execute registered render stages in order
        {
            let stages_guard = render_stages.read().unwrap();
            for stage in stages_guard.stages() {
                let stage_start = std::time::Instant::now();
                data = stage.transform(data, &ctx);
                tracing::trace!("[RTT] stage '{}' took {:?}", stage.name(), stage_start.elapsed());
            }
        }

        tracing::trace!(
            "[RTT] execute_pipeline: from_buffer={:?} stages={:?} total={:?}",
            from_buffer_time,
            pipeline_start.elapsed().saturating_sub(from_buffer_time),
            pipeline_start.elapsed()
        );

        data
    }

    #[allow(clippy::cast_possible_truncation)]
    pub(super) fn calculate_scroll_adjustment_for_virtual_lines(
        render_data: &crate::render::RenderData,
        current_scroll: u16,
        window_height: u16,
        cursor_line: u16,
    ) -> Option<u16> {
        use crate::render::VirtualLinePosition;

        // Count virtual lines in the viewport that appear at or before cursor
        let mut display_rows_used = 0u16;
        let scroll_start = current_scroll as usize;
        let cursor_idx = cursor_line as usize;

        // Count ALL display rows from scroll to cursor (don't return early)
        for line_idx in scroll_start..=cursor_idx {
            // Count Before virtual lines
            if render_data
                .virtual_lines
                .contains_key(&(line_idx, VirtualLinePosition::Before))
            {
                display_rows_used += 1;
            }

            // Count the buffer line itself
            display_rows_used += 1;

            // Count After virtual lines (except for cursor line - it's after cursor)
            if line_idx < cursor_idx
                && render_data
                    .virtual_lines
                    .contains_key(&(line_idx, VirtualLinePosition::After))
            {
                display_rows_used += 1;
            }
        }

        // AFTER counting everything, check if adjustment needed
        if display_rows_used > window_height {
            let overshoot = display_rows_used - window_height;
            Some(current_scroll + overshoot)
        } else {
            None
        }
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::too_many_lines)]
    pub(super) fn render_data_to_framebuffer(
        &self,
        render_data: &crate::render::RenderData,
        window: &Window,
        frame_buffer: &mut FrameBuffer,
        theme: &Theme,
        buffer: &Buffer,
    ) {
        use crate::render::{DecorationKind, LineVisibility, VirtualLinePosition};

        // Get effective cursor position (active window uses buffer cursor, inactive uses window cursor)
        let cursor_y = if window.is_active {
            buffer.cur.y
        } else {
            window.cursor.y
        };

        // Calculate selection bounds if active
        let selection_bounds = if buffer.selection.active {
            use crate::buffer::SelectionMode;
            match buffer.selection.mode {
                SelectionMode::Block => {
                    let anchor = buffer.selection.anchor;
                    let cursor = buffer.cur;
                    let top_left = Position {
                        x: anchor.x.min(cursor.x),
                        y: anchor.y.min(cursor.y),
                    };
                    let bottom_right = Position {
                        x: anchor.x.max(cursor.x),
                        y: anchor.y.max(cursor.y),
                    };
                    Some((top_left, bottom_right, SelectionMode::Block))
                }
                SelectionMode::Character | SelectionMode::Line => {
                    let anchor = buffer.selection.anchor;
                    let cursor = buffer.cur;
                    let (start, end) =
                        if anchor.y < cursor.y || (anchor.y == cursor.y && anchor.x <= cursor.x) {
                            (anchor, cursor)
                        } else {
                            (cursor, anchor)
                        };
                    Some((start, end, buffer.selection.mode))
                }
            }
        } else {
            None
        };

        // Get scroll offset from buffer anchor
        let scroll_offset = window.buffer_anchor().map_or(0, |anchor| anchor.y);

        // Calculate line number width
        let total_lines = render_data.lines.len();
        #[allow(clippy::cast_precision_loss)]
        #[allow(clippy::cast_possible_truncation)]
        #[allow(clippy::cast_sign_loss)]
        let num_width = if window
            .line_number
            .as_ref()
            .is_some_and(LineNumber::is_shown)
            && total_lines > 0
        {
            (total_lines as f64).log10().floor() as usize + 1
        } else {
            1
        };

        // Precompute whether any signs exist (for auto mode)
        let has_signs = render_data.signs.iter().any(Option::is_some);
        let sign_column_width = window.sign_column_mode.effective_width(has_signs);

        // Render each visible line, starting from scroll offset
        let mut display_row = 0u16;
        let start_line = scroll_offset as usize;
        for (idx, line) in render_data.lines.iter().enumerate().skip(start_line) {
            let line_idx = idx;
            if display_row >= window.height {
                break;
            }

            // Render virtual lines BEFORE this buffer line
            if let Some(vl) = render_data
                .virtual_lines
                .get(&(line_idx, VirtualLinePosition::Before))
            {
                if display_row >= window.height {
                    break;
                }
                let screen_y = window.anchor.y + display_row;
                let gutter_total = sign_column_width + num_width as u16 + 1; // +1 for padding
                let mut col = window.anchor.x + gutter_total;
                let max_col = window.anchor.x + window.width;
                for ch in vl.text.chars() {
                    if col >= max_col {
                        break;
                    }
                    frame_buffer.put_char(col, screen_y, ch, &vl.style);
                    col += 1;
                }
                display_row += 1;
            }

            if display_row >= window.height {
                break;
            }

            // Check visibility
            let visibility = &render_data.visibility[line_idx];
            match visibility {
                LineVisibility::Hidden => {}
                LineVisibility::FoldMarker { preview, .. } => {
                    let screen_y = window.anchor.y + display_row;
                    let mut gutter_width = 0u16;

                    // Get sign for this line (used by both sign column and number mode)
                    let sign = render_data.signs.get(line_idx).and_then(|s| s.as_ref());

                    // Render sign column FIRST (if enabled) - leftmost gutter element
                    if sign_column_width > 0 {
                        let sign_x = window.anchor.x + gutter_width;
                        if line_idx == 0 {
                            tracing::info!(
                                "SIGN_COLUMN: x={} y={} width={} sign_present={}",
                                sign_x,
                                screen_y,
                                sign_column_width,
                                sign.is_some()
                            );
                        }
                        gutter_width += Window::render_sign_to_buffer(
                            frame_buffer,
                            sign_x,
                            screen_y,
                            sign,
                            sign_column_width,
                            theme,
                        );
                    }

                    // Render line numbers AFTER sign column
                    let line_num_x = window.anchor.x + gutter_width;
                    if line_idx == 0 {
                        tracing::info!("LINE_NUM: x={} gutter_width={}", line_num_x, gutter_width);
                    }
                    gutter_width += self.render_line_number_to_buffer_simple(
                        frame_buffer,
                        window.anchor.x + gutter_width,
                        screen_y,
                        line_idx as u16,
                        cursor_y,
                        num_width,
                        theme,
                        window,
                        sign,
                    );

                    // Render fold marker
                    let fold_style = &theme.fold.marker;
                    let mut col = window.anchor.x + gutter_width;
                    for ch in preview.chars() {
                        if col < frame_buffer.width() {
                            frame_buffer.put_char(col, screen_y, ch, fold_style);
                            col += 1;
                        }
                    }
                    display_row += 1;
                }
                LineVisibility::Visible => {
                    // Overlay search limit: 3x screen cells (covers worst-case bracket density)
                    let overlay_limit = (window.width as usize) * 3;

                    // Helper function: find highlight covering a position
                    // Checks last N highlights first (render stage additions), then falls back to
                    // sequential search for syntax highlights from cache
                    #[allow(clippy::items_after_statements)]
                    fn find_highlight_at(
                        highlights: &[crate::render::LineHighlight],
                        pos: usize,
                        overlay_limit: usize,
                    ) -> Option<&crate::render::LineHighlight> {
                        // First check recent highlights (render stage overlays) from end
                        let overlay_start = highlights.len().saturating_sub(overlay_limit);
                        if let Some(h) = highlights[overlay_start..]
                            .iter()
                            .rev()
                            .find(|h| h.start_col <= pos && pos < h.end_col)
                        {
                            return Some(h);
                        }
                        // Fall back to syntax highlights (sorted by position, sequential scan OK)
                        highlights[..overlay_start]
                            .iter()
                            .find(|h| h.start_col <= pos && pos < h.end_col)
                    }

                    let screen_y = window.anchor.y + display_row;
                    let mut gutter_width = 0u16;

                    // Get sign for this line (used by both sign column and number mode)
                    let sign = render_data.signs.get(line_idx).and_then(|s| s.as_ref());

                    // Render sign column FIRST (if enabled) - leftmost gutter element
                    if sign_column_width > 0 {
                        let sign_x = window.anchor.x + gutter_width;
                        if line_idx == 0 {
                            tracing::info!(
                                "SIGN_COLUMN: x={} y={} width={} sign_present={}",
                                sign_x,
                                screen_y,
                                sign_column_width,
                                sign.is_some()
                            );
                        }
                        gutter_width += Window::render_sign_to_buffer(
                            frame_buffer,
                            sign_x,
                            screen_y,
                            sign,
                            sign_column_width,
                            theme,
                        );
                    }

                    // Render line numbers AFTER sign column
                    let line_num_x = window.anchor.x + gutter_width;
                    if line_idx == 0 {
                        tracing::info!("LINE_NUM: x={} gutter_width={}", line_num_x, gutter_width);
                    }
                    gutter_width += self.render_line_number_to_buffer_simple(
                        frame_buffer,
                        window.anchor.x + gutter_width,
                        screen_y,
                        line_idx as u16,
                        cursor_y,
                        num_width,
                        theme,
                        window,
                        sign,
                    );

                    // Render line content with syntax highlights and decorations
                    let mut col = window.anchor.x + gutter_width;
                    #[allow(clippy::cast_possible_truncation)]
                    let buffer_line_y = line_idx as u16;

                    // Get syntax highlights and decorations for this line
                    let line_highlights = render_data.highlights.get(line_idx);
                    let line_decorations = render_data.decorations.get(line_idx);
                    let mut char_idx = 0usize;
                    let chars: Vec<char> = line.chars().collect();

                    while char_idx < chars.len() {
                        if col >= window.anchor.x + window.width {
                            break;
                        }

                        // Find ALL decorations that cover current position
                        #[allow(clippy::option_if_let_else)]
                        let (conceal_deco, background_deco) =
                            if let Some(decorations) = line_decorations {
                                let mut conceal: Option<&crate::render::Decoration> = None;
                                let mut background: Option<&crate::render::Decoration> = None;
                                for deco in decorations {
                                    if deco.start_col <= char_idx && char_idx < deco.end_col {
                                        match &deco.kind {
                                            DecorationKind::Conceal { .. } if conceal.is_none() => {
                                                conceal = Some(deco);
                                            }
                                            DecorationKind::Background { .. }
                                                if background.is_none() =>
                                            {
                                                background = Some(deco);
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                (conceal, background)
                            } else {
                                (None, None)
                            };

                        // Handle conceal decorations (with optional background style)
                        if let Some(deco) = conceal_deco
                            && let DecorationKind::Conceal { replacement, .. } = &deco.kind
                        {
                            // Only output replacement at start of decoration span
                            if char_idx == deco.start_col
                                && let Some(repl) = replacement
                            {
                                // Use background style if present, otherwise default
                                let style = background_deco
                                    .and_then(|d| match &d.kind {
                                        DecorationKind::Background { style } => Some(style),
                                        _ => None,
                                    })
                                    .unwrap_or(&theme.base.default);
                                // Output replacement text
                                for repl_ch in repl.chars() {
                                    if col >= window.anchor.x + window.width {
                                        break;
                                    }
                                    frame_buffer.put_char(col, screen_y, repl_ch, style);
                                    col += 1;
                                }
                            }
                            // Hide decorations (replacement=None) output nothing
                            // Skip the concealed character
                            char_idx += 1;
                            continue;
                        }

                        let ch = chars[char_idx];

                        // Check if this character is within the selection
                        #[allow(clippy::cast_possible_truncation)]
                        let char_x = char_idx as u16;
                        let is_selected = if let Some((start, end, mode)) = selection_bounds {
                            use crate::buffer::SelectionMode;
                            match mode {
                                SelectionMode::Block => {
                                    // Block selection: check if within rectangle
                                    buffer_line_y >= start.y
                                        && buffer_line_y <= end.y
                                        && char_x >= start.x
                                        && char_x <= end.x
                                }
                                SelectionMode::Character | SelectionMode::Line => {
                                    // Character/line selection: check if within range
                                    if buffer_line_y < start.y || buffer_line_y > end.y {
                                        false
                                    } else if buffer_line_y == start.y && buffer_line_y == end.y {
                                        // Single line selection
                                        char_x >= start.x && char_x <= end.x
                                    } else if buffer_line_y == start.y {
                                        // First line of selection
                                        char_x >= start.x
                                    } else if buffer_line_y == end.y {
                                        // Last line of selection
                                        char_x <= end.x
                                    } else {
                                        // Middle lines - all selected
                                        true
                                    }
                                }
                            }
                        } else {
                            false
                        };

                        // Apply appropriate style (selection > decoration background + syntax > syntax > default)
                        // We may need to merge decoration background with syntax highlight
                        #[allow(unused_assignments)]
                        let mut merged_style: Option<
                            crate::highlight::Style,
                        > = None;

                        let style = if is_selected {
                            &theme.selection.visual
                        } else if let Some(deco) = background_deco {
                            // Check for background decoration
                            #[allow(clippy::option_if_let_else)]
                            if let DecorationKind::Background { style: deco_style } = &deco.kind {
                                // Merge decoration background with syntax highlight foreground
                                // This allows code blocks to have both background tint AND syntax colors
                                if let Some(hl) = line_highlights
                                    .and_then(|h| find_highlight_at(h, char_idx, overlay_limit))
                                {
                                    // Syntax highlight provides fg, decoration provides bg
                                    merged_style = Some(hl.style.merge(deco_style));
                                    merged_style.as_ref().unwrap()
                                } else {
                                    deco_style
                                }
                            } else if let Some(highlights) = line_highlights {
                                // Find highlight at current position (later ones override)
                                find_highlight_at(highlights, char_idx, overlay_limit)
                                    .map_or(&theme.base.default, |h| &h.style)
                            } else {
                                &theme.base.default
                            }
                        } else if let Some(highlights) = line_highlights {
                            // Find highlight at current position (later ones override)
                            find_highlight_at(highlights, char_idx, overlay_limit)
                                .map_or(&theme.base.default, |h| &h.style)
                        } else {
                            &theme.base.default
                        };

                        frame_buffer.put_char(col, screen_y, ch, style);
                        col += 1;
                        char_idx += 1;
                    }

                    // Render virtual text after line content (if present)
                    if let Some(vt) = render_data
                        .virtual_texts
                        .get(line_idx)
                        .and_then(|v| v.as_ref())
                    {
                        let window_right = window.anchor.x + window.width;
                        let remaining = window_right.saturating_sub(col) as usize;

                        // Need at least 4 chars: "  X..." (separator + 1 char + ellipsis)
                        if remaining > 4 {
                            // Render separator (2 spaces)
                            frame_buffer.put_char(col, screen_y, ' ', &vt.style);
                            frame_buffer.put_char(col + 1, screen_y, ' ', &vt.style);
                            col += 2;

                            // Calculate max chars for virtual text
                            let max_chars = remaining.saturating_sub(2);
                            let vt_chars: Vec<char> = vt.text.chars().collect();

                            // Truncate with ellipsis if needed
                            let (text_to_render, needs_ellipsis) = if vt_chars.len() > max_chars {
                                (&vt_chars[..max_chars.saturating_sub(3)], true)
                            } else {
                                (&vt_chars[..], false)
                            };

                            // Render virtual text characters
                            for &ch in text_to_render {
                                if col < window_right {
                                    frame_buffer.put_char(col, screen_y, ch, &vt.style);
                                    col += 1;
                                }
                            }

                            // Render ellipsis if truncated
                            if needs_ellipsis {
                                for ch in "...".chars() {
                                    if col < window_right {
                                        frame_buffer.put_char(col, screen_y, ch, &vt.style);
                                        col += 1;
                                    }
                                }
                            }
                        }
                    }

                    display_row += 1;
                }
            }

            // Render virtual lines AFTER this buffer line
            if let Some(vl) = render_data
                .virtual_lines
                .get(&(line_idx, VirtualLinePosition::After))
            {
                if display_row >= window.height {
                    continue;
                }
                let screen_y = window.anchor.y + display_row;
                let gutter_total = sign_column_width + num_width as u16 + 1;
                let mut col = window.anchor.x + gutter_total;
                let max_col = window.anchor.x + window.width;
                for ch in vl.text.chars() {
                    if col >= max_col {
                        break;
                    }
                    frame_buffer.put_char(col, screen_y, ch, &vl.style);
                    col += 1;
                }
                display_row += 1;
            }
        }

        // Fill remaining rows with tilde markers
        let tilde_style = &theme.gutter.line_number;
        while display_row < window.height {
            let screen_y = window.anchor.y + display_row;
            frame_buffer.put_char(window.anchor.x, screen_y, '~', tilde_style);
            display_row += 1;
        }
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::unused_self)]
    #[allow(clippy::cast_possible_truncation)]
    pub(super) fn render_line_number_to_buffer_simple(
        &self,
        buffer: &mut FrameBuffer,
        x: u16,
        y: u16,
        row: u16,
        cursor_y: u16,
        num_width: usize,
        theme: &Theme,
        window: &Window,
        sign: Option<&Sign>,
    ) -> u16 {
        use crate::screen::window::LineNumberMode;

        let Some(line_number) = &window.line_number else {
            return 0;
        };
        if !line_number.is_shown() {
            return 0;
        }

        // Calculate plain text line number (without ANSI codes for frame buffer)
        let is_current_line = row == cursor_y;

        let num_str = if window.is_active {
            match line_number.mode() {
                LineNumberMode::Absolute => format!("{}", row + 1),
                LineNumberMode::Relative => {
                    let rel = (i32::from(row) - i32::from(cursor_y)).abs();
                    format!("{rel}")
                }
                LineNumberMode::Hybrid => {
                    if is_current_line {
                        format!("{}", row + 1)
                    } else {
                        let rel = (i32::from(row) - i32::from(cursor_y)).abs();
                        format!("{rel}")
                    }
                }
            }
        } else {
            format!("{}", row + 1)
        };

        // Format with padding and trailing space (plain text only)
        let line_num_str = format!("{num_str:>num_width$} ");

        // Render line number with style
        // For Number mode: use sign's foreground as background color
        let base_style = if !window.is_active {
            theme.gutter.inactive_line_number.clone()
        } else if is_current_line {
            theme.gutter.current_line_number.clone()
        } else {
            theme.gutter.line_number.clone()
        };

        let line_num_style = if window.sign_column_mode == SignColumnMode::Number {
            if let Some(sign) = sign {
                // Use sign's foreground color as background for line number
                Style {
                    bg: sign.style.fg,
                    ..base_style
                }
            } else {
                base_style
            }
        } else {
            base_style
        };

        let mut col = x;
        for ch in line_num_str.chars() {
            buffer.put_char(col, y, ch, &line_num_style);
            col += 1;
        }

        line_num_str.len() as u16
    }

    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_lines)]
    pub(super) fn render_windows(
        &mut self,
        buffers: &BTreeMap<usize, Buffer>,
        highlight_store: &HighlightStore,
        mode: &ModeState,
        cmd_line: &CommandLine,
        pending_keys: &str,
        last_command: &str,
        color_mode: ColorMode,
        theme: &Theme,
        visibility_source: &dyn BufferVisibilitySource,
        indent_analyzer: &IndentAnalyzer,
        modifier_registry: Option<&ModifierRegistry>,
        decoration_store: Option<&DecorationStore>,
        render_stages: &std::sync::Arc<std::sync::RwLock<crate::render::RenderStageRegistry>>,
        plugin_state: &std::sync::Arc<crate::plugin::PluginStateRegistry>,
        display_registry: &crate::display::DisplayRegistry,
    ) -> std::result::Result<(), std::io::Error> {
        let rw_start = std::time::Instant::now();
        // Take frame renderer out (borrow checker workaround)
        let mut renderer = self
            .frame_renderer
            .take()
            .expect("frame renderer should always be initialized");

        // Set color mode for style conversion
        renderer.set_color_mode(color_mode);

        // Clear buffer for fresh frame
        renderer.clear();
        let buffer = renderer.buffer_mut();

        // Track cursor position
        let mut cursor_pos: Option<(u16, u16)> = None;
        let mut current_buffer: Option<&Buffer> = None;

        // Render tab line if multiple tabs exist
        self.render_tab_line_to_buffer(buffer, color_mode, theme);

        // Phase 2: Render editor windows
        // Phase 3 will add overlay collection and unified z-order rendering

        // Query panel offsets from plugin state and update layout manager if changed
        let left_offset = plugin_state.left_panel_width();
        let right_offset = plugin_state.right_panel_width();
        let offsets_changed =
            left_offset != self.layout.left_offset() || right_offset != self.layout.right_offset();

        if offsets_changed {
            self.layout.set_left_offset(left_offset);
            self.layout.set_right_offset(right_offset);
            self.update_window_layouts();
        }

        // Clear pending scroll events from previous frame
        self.pending_viewport_scrolls.clear();

        // Update scroll positions BEFORE cloning windows
        for win in &mut self.windows {
            if let Some(buffer_id) = win.buffer_id()
                && let Some(buf) = buffers.get(&buffer_id)
            {
                let effective_cursor_y = if win.is_active {
                    buf.cur.y
                } else {
                    win.cursor.y
                };
                let scrolled = win.update_scroll(effective_cursor_y);
                if scrolled {
                    let (top_line, bottom_line) = win.viewport_bounds();
                    self.pending_viewport_scrolls.push(ViewportScrollInfo {
                        window_id: win.id,
                        buffer_id,
                        top_line,
                        bottom_line,
                    });
                }
            }
        }

        // Collect all windows (editor + plugin windows)
        let mut windows_to_render = self.windows.clone();

        // Extract active window info for EditorContext
        let (
            active_anchor_x,
            active_anchor_y,
            active_gutter_width,
            active_scroll_y,
            active_height,
            cursor_col,
            cursor_row,
            active_buffer_content,
        ) = self
            .windows
            .iter()
            .find(|w| w.is_active)
            .and_then(|win| {
                let buffer_id = win.buffer_id()?;
                let buf = buffers.get(&buffer_id)?;
                let line_num_width = win.line_number_width(buf.contents.len());
                // Use max width for layout (assume signs present in auto mode)
                let sign_width = win.sign_column_mode.effective_width(true);
                let gutter_width = sign_width + line_num_width;
                let scroll_y = win.buffer_anchor().map_or(0, |a| a.y);
                let height = win.height;
                // Get buffer content for context providers (e.g., sticky headers)
                let snapshot = crate::buffer::BufferSnapshot::from_buffer(buf);
                let content = snapshot.content();
                Some((
                    win.anchor.x,
                    win.anchor.y,
                    gutter_width,
                    scroll_y,
                    height,
                    buf.cur.x,
                    buf.cur.y,
                    Some(content),
                ))
            })
            .unwrap_or((0, 0, 0, 0, 0, 0, 0, None));

        // Build editor context for window providers
        let editor_ctx = crate::plugin::EditorContext::new(
            self.size.width,
            self.size.height,
            mode.edit_mode.clone(),
            mode.sub_mode.clone(),
            mode.interactor_id,
            self.active_buffer_id().unwrap_or(0),
            buffers.len(),
            color_mode,
        )
        .with_left_offset(left_offset)
        .with_pending_keys(pending_keys)
        .with_active_window(
            active_anchor_x,
            active_anchor_y,
            active_gutter_width,
            active_scroll_y,
            active_height,
        )
        .with_cursor(cursor_col, cursor_row)
        .with_buffer_content(active_buffer_content);

        // Sort all windows by z-order
        windows_to_render.sort_by_key(|w| w.z_order);

        // Render all windows
        for win in &mut windows_to_render {
            // Handle PluginBuffer windows differently
            if let crate::content::WindowContentSource::PluginBuffer { provider, .. } = &win.source
            {
                // For plugin buffers, generate virtual content via provider
                use crate::content::BufferContext;
                let buffer_ctx = BufferContext {
                    buffer_id: 0,
                    width: win.width,
                    height: win.height,
                    state: plugin_state,
                };
                let lines = provider.get_lines(&buffer_ctx);

                // Render plugin buffer lines directly to frame buffer
                for (line_idx, line) in lines.iter().enumerate() {
                    let y = win.anchor.y + line_idx as u16;
                    if y >= self.size.height {
                        break;
                    }
                    let mut x = win.anchor.x;
                    for ch in line.chars() {
                        if x >= win.anchor.x + win.width {
                            break;
                        }
                        buffer.put_char(x, y, ch, &theme.base.default);
                        x += 1;
                    }
                }
                continue; // Skip normal window rendering
            }

            // Normal FileBuffer windows
            if let Some(buffer_id) = win.buffer_id()
                && let Some(buf) = buffers.get(&buffer_id)
            {
                current_buffer = Some(buf);

                // Find the corresponding window in self.windows (for mutable access)
                // Plugin windows won't have a match, so we skip modifier/scroll updates for them
                let editor_win_idx = self.windows.iter().position(|w| w.id == win.id);

                // Evaluate modifiers for this window (only for editor windows)
                if let Some(editor_idx) = editor_win_idx
                    && let Some(registry) = modifier_registry
                {
                    let filetype = buf
                        .file_path
                        .as_ref()
                        .map(|p| crate::filetype::filetype_id(p));
                    let mod_ctx = ModifierContext::new(
                        ComponentId::EDITOR,
                        &mode.edit_mode,
                        &mode.sub_mode,
                        win.id,
                        buffer_id,
                    )
                    .with_filetype(filetype)
                    .with_active(win.is_active)
                    .with_modified(buf.modified)
                    .with_floating(win.is_floating);

                    let style_state = registry.evaluate(&mod_ctx);

                    // Apply window decorations from modifiers (need mutable access)
                    let win_mut = &mut self.windows[editor_idx];
                    if let Some(show_ln) = style_state.style.decorations.line_numbers {
                        win_mut.set_number(show_ln);
                    }
                    if let Some(relative) = style_state.style.decorations.relative_numbers {
                        win_mut.set_relative_number(relative);
                    }
                    if let Some(scrollbar) = style_state.style.decorations.scrollbar {
                        win_mut.scrollbar_enabled = scrollbar;
                    }
                    // Note: scroll position already updated before window cloning
                }

                // Execute render pipeline with the window from windows_to_render
                let pipeline_start = std::time::Instant::now();
                let render_data = self.execute_pipeline(
                    win,
                    buf,
                    highlight_store,
                    theme,
                    color_mode,
                    visibility_source,
                    indent_analyzer,
                    decoration_store,
                    render_stages,
                    mode,
                );
                let pipeline_time = pipeline_start.elapsed();

                // Adjust scroll if virtual lines would push cursor off-screen
                let current_scroll = win.buffer_anchor().map_or(0, |a| a.y);
                if let Some(new_scroll) = Self::calculate_scroll_adjustment_for_virtual_lines(
                    &render_data,
                    current_scroll,
                    win.height,
                    buf.cur.y,
                ) {
                    // Update both the cloned window and the original
                    if let Some(mut anchor) = win.buffer_anchor() {
                        anchor.y = new_scroll;
                        win.set_buffer_anchor(anchor);
                    }
                    if let Some(editor_idx) = editor_win_idx
                        && let Some(mut anchor) = self.windows[editor_idx].buffer_anchor()
                    {
                        anchor.y = new_scroll;
                        self.windows[editor_idx].set_buffer_anchor(anchor);
                    }
                    tracing::trace!(
                        "[VLINE] adjusted scroll: {} -> {} for virtual lines",
                        current_scroll,
                        new_scroll
                    );
                }

                // Render pipeline data to frame buffer
                let fb_start = std::time::Instant::now();
                self.render_data_to_framebuffer(&render_data, win, buffer, theme, buf);
                tracing::trace!(
                    "[RTT] window render: pipeline={:?} framebuffer={:?}",
                    pipeline_time,
                    fb_start.elapsed()
                );

                // Calculate cursor position only for the ACTIVE window (and if editor is focused)
                if win.is_active {
                    use crate::render::VirtualLinePosition;

                    let line_num_width = win.line_number_width(buf.contents.len());
                    // Use max width for cursor position (assume signs present in auto mode)
                    let sign_width = win.sign_column_mode.effective_width(true);

                    // Apply column mapping for decorated lines (e.g., table expansion)
                    let visual_cursor_col = render_data
                        .cursor_col_mapping()
                        .and_then(|mapping| mapping.get(buf.cur.x as usize).copied())
                        .unwrap_or(buf.cur.x);
                    let cursor_x = win.anchor.x + sign_width + line_num_width + visual_cursor_col;

                    // Calculate virtual line offset for cursor position
                    // Virtual lines rendered BEFORE buffer lines shift the cursor down
                    let scroll_offset = win.buffer_anchor().map_or(0, |a| a.y);
                    let cursor_buffer_line = buf.cur.y;

                    // Count virtual lines that appear visually before the cursor line
                    // - Before: renders before line N, count if N <= cursor
                    // - After: renders after line N, count if N < cursor (before cursor line)
                    let virtual_line_offset: u16 = render_data
                        .virtual_lines
                        .keys()
                        .filter(|(line_idx, pos)| {
                            let line = *line_idx as u16;
                            line >= scroll_offset
                                && match pos {
                                    VirtualLinePosition::Before => line <= cursor_buffer_line,
                                    VirtualLinePosition::After => line < cursor_buffer_line,
                                }
                        })
                        .count() as u16;

                    let cursor_y = win.anchor.y
                        + cursor_buffer_line.saturating_sub(scroll_offset)
                        + virtual_line_offset;
                    cursor_pos = Some((cursor_x, cursor_y));
                }
            }
        }

        // Render plugin windows (new unified system)
        // These are sorted by z_order and rendered on top of editor windows
        self.render_plugin_windows(buffer, plugin_state, &editor_ctx, theme);

        // Render window separators
        self.render_window_separators_to_buffer(buffer, theme);

        // Render status line or command line
        if mode.is_command() {
            self.render_command_line_to_buffer(buffer, cmd_line, theme);
        } else {
            self.render_status_line_to_buffer(
                buffer,
                mode,
                current_buffer,
                pending_keys,
                last_command,
                theme,
                color_mode,
                plugin_state,
                display_registry,
            );
        }

        // Settings menu overlay is now rendered by the settings-menu plugin

        // Put renderer back and flush
        let pre_flush = rw_start.elapsed();
        self.frame_renderer = Some(renderer);
        let renderer = self.frame_renderer.as_mut().unwrap();
        queue!(self.out_stream, Hide)?;
        renderer.flush(&mut self.out_stream)?;
        let post_renderer_flush = rw_start.elapsed();

        // Position cursor
        if let Some((x, y)) = cursor_pos {
            queue!(self.out_stream, MoveTo(x, y))?;
        }

        queue!(self.out_stream, Show)?;
        let result = self.out_stream.flush();
        tracing::trace!(
            "[RTT] render_windows: pre_flush={:?} renderer.flush={:?} stream.flush={:?} total={:?}",
            pre_flush,
            post_renderer_flush.saturating_sub(pre_flush),
            rw_start.elapsed().saturating_sub(post_renderer_flush),
            rw_start.elapsed()
        );
        result
    }
}
