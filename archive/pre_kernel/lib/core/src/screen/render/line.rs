//! Line rendering for screen windows
//!
//! This module contains the core line rendering logic:
//! - `render_data_to_framebuffer()` - Renders lines to frame buffer
//! - `render_line_number_to_buffer_simple()` - Renders line numbers
//! - `calculate_scroll_adjustment_for_virtual_lines()` - Adjusts scroll for virtual lines

use crate::{
    buffer::Buffer,
    frame::FrameBuffer,
    highlight::{Style, Theme},
    screen::Position,
    sign::Sign,
};

use super::super::{
    Screen,
    window::{LineNumber, SignColumnMode, Window},
};

impl Screen {
    #[allow(clippy::cast_possible_truncation)]
    pub(in crate::screen) fn calculate_scroll_adjustment_for_virtual_lines(
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
    pub(in crate::screen) fn render_data_to_framebuffer(
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
            window.viewport.cursor.y
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
            .config
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
        let sign_column_width = window.config.sign_column_mode.effective_width(has_signs);

        // Render each visible line, starting from scroll offset
        let mut display_row = 0u16;
        let start_line = scroll_offset as usize;
        for (idx, line) in render_data.lines.iter().enumerate().skip(start_line) {
            let line_idx = idx;
            if display_row >= window.bounds.height {
                break;
            }

            // Render virtual lines BEFORE this buffer line
            if let Some(vl) = render_data
                .virtual_lines
                .get(&(line_idx, VirtualLinePosition::Before))
            {
                if display_row >= window.bounds.height {
                    break;
                }
                let screen_y = window.bounds.y + display_row;
                let gutter_total = sign_column_width + num_width as u16 + 1; // +1 for padding
                let mut col = window.bounds.x + gutter_total;
                let max_col = window.bounds.x + window.bounds.width;
                for ch in vl.text.chars() {
                    if col >= max_col {
                        break;
                    }
                    frame_buffer.put_char(col, screen_y, ch, &vl.style);
                    col += 1;
                }
                display_row += 1;
            }

            if display_row >= window.bounds.height {
                break;
            }

            // Check visibility
            let visibility = &render_data.visibility[line_idx];
            match visibility {
                LineVisibility::Hidden => {}
                LineVisibility::FoldMarker { preview, .. } => {
                    let screen_y = window.bounds.y + display_row;
                    let mut gutter_width = 0u16;

                    // Get sign for this line (used by both sign column and number mode)
                    let sign = render_data.signs.get(line_idx).and_then(|s| s.as_ref());

                    // Render sign column FIRST (if enabled) - leftmost gutter element
                    if sign_column_width > 0 {
                        let sign_x = window.bounds.x + gutter_width;
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
                    let line_num_x = window.bounds.x + gutter_width;
                    if line_idx == 0 {
                        tracing::info!("LINE_NUM: x={} gutter_width={}", line_num_x, gutter_width);
                    }
                    gutter_width += self.render_line_number_to_buffer_simple(
                        frame_buffer,
                        window.bounds.x + gutter_width,
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
                    let mut col = window.bounds.x + gutter_width;
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
                    let overlay_limit = (window.bounds.width as usize) * 3;

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

                    let screen_y = window.bounds.y + display_row;
                    let mut gutter_width = 0u16;

                    // Get sign for this line (used by both sign column and number mode)
                    let sign = render_data.signs.get(line_idx).and_then(|s| s.as_ref());

                    // Render sign column FIRST (if enabled) - leftmost gutter element
                    if sign_column_width > 0 {
                        let sign_x = window.bounds.x + gutter_width;
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
                    let line_num_x = window.bounds.x + gutter_width;
                    if line_idx == 0 {
                        tracing::info!("LINE_NUM: x={} gutter_width={}", line_num_x, gutter_width);
                    }
                    gutter_width += self.render_line_number_to_buffer_simple(
                        frame_buffer,
                        window.bounds.x + gutter_width,
                        screen_y,
                        line_idx as u16,
                        cursor_y,
                        num_width,
                        theme,
                        window,
                        sign,
                    );

                    // Render line content with syntax highlights and decorations
                    let mut col = window.bounds.x + gutter_width;
                    #[allow(clippy::cast_possible_truncation)]
                    let buffer_line_y = line_idx as u16;

                    // Get syntax highlights and decorations for this line
                    let line_highlights = render_data.highlights.get(line_idx);
                    let line_decorations = render_data.decorations.get(line_idx);
                    let mut char_idx = 0usize;
                    let chars: Vec<char> = line.chars().collect();

                    while char_idx < chars.len() {
                        if col >= window.bounds.x + window.bounds.width {
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
                                    if col >= window.bounds.x + window.bounds.width {
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
                        let window_right = window.bounds.x + window.bounds.width;
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
                if display_row >= window.bounds.height {
                    continue;
                }
                let screen_y = window.bounds.y + display_row;
                let gutter_total = sign_column_width + num_width as u16 + 1;
                let mut col = window.bounds.x + gutter_total;
                let max_col = window.bounds.x + window.bounds.width;
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
        while display_row < window.bounds.height {
            let screen_y = window.bounds.y + display_row;
            frame_buffer.put_char(window.bounds.x, screen_y, '~', tilde_style);
            display_row += 1;
        }
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::unused_self)]
    #[allow(clippy::cast_possible_truncation)]
    pub(in crate::screen) fn render_line_number_to_buffer_simple(
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

        let Some(line_number) = &window.config.line_number else {
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

        let line_num_style = if window.config.sign_column_mode == SignColumnMode::Number {
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
}
