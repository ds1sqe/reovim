//! Window rendering module

use crate::{
    buffer::{Buffer, SelectionMode, SelectionOps},
    content::WindowContentSource,
    decoration::{Decoration, DecorationStore},
    frame::FrameBuffer,
    highlight::{
        ColorMode, Highlight, HighlightGroup, HighlightStore, Span, Style, Theme,
        store::LineHighlight,
    },
    indent::IndentAnalyzer,
    modd::EditMode,
    visibility::{BufferVisibilitySource, VisibilityQuery},
};

use super::{
    Position,
    border::{BorderConfig, BorderInsets, WindowAdjacency},
};

/// Scrollbar rendering state
#[derive(Debug, Clone, Copy)]
pub struct ScrollbarState {
    /// Whether scrollbar should be displayed
    pub enabled: bool,
    /// Start row of the thumb (0-indexed, relative to viewport)
    pub thumb_start: u16,
    /// End row of the thumb (exclusive)
    pub thumb_end: u16,
}

/// Represents top left corner position
#[derive(Clone, Copy, Debug, Default)]
pub struct Anchor {
    pub x: u16,
    pub y: u16,
}

/// Window is a unified renderable element
///
/// All things that render to screen (editor windows, overlays, plugin UIs)
/// are represented as Windows with different content sources.
#[derive(Clone)]
pub struct Window {
    // Identity
    /// Unique identifier for this window
    pub id: usize,
    /// Source of content for this window
    pub source: WindowContentSource,

    // Layout
    /// Where this window's top left is positioned on the screen
    pub anchor: Anchor,
    /// Window width
    pub width: u16,
    /// Window height
    pub height: u16,
    /// Z-order for layered rendering (higher = on top)
    pub z_order: u16,

    // State
    /// Whether this window is the active/focused window
    pub is_active: bool,
    /// Whether this window is a floating window
    pub is_floating: bool,

    // Editor features (may be None for overlay windows)
    /// Line number display configuration
    pub line_number: Option<LineNumber>,
    /// Whether to show scrollbar
    pub scrollbar_enabled: bool,
    /// Sign column display mode
    pub sign_column_mode: SignColumnMode,
    /// Per-window cursor position
    pub cursor: Position,
    /// Track preferred column for vertical movement (j/k)
    pub desired_col: Option<u16>,
    /// Border configuration for this window
    pub border_config: Option<BorderConfig>,
}

#[derive(Clone, Copy, Debug)]
pub enum LineNumberMode {
    Absolute,
    Relative,
    Hybrid,
}

/// Sign column display mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SignColumnMode {
    /// Show when signs present, hide otherwise (default)
    #[default]
    Auto,
    /// Always show with specified width
    Yes(u16),
    /// Never show
    No,
    /// Display signs in line number column (as background color)
    Number,
}

impl SignColumnMode {
    /// Get the effective sign column width based on mode and whether signs exist
    #[must_use]
    pub const fn effective_width(&self, has_signs: bool) -> u16 {
        match self {
            Self::Auto => {
                if has_signs {
                    2
                } else {
                    0
                }
            }
            Self::Yes(width) => *width,
            Self::No | Self::Number => 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LineNumber {
    show: bool,
    number: bool,          // :set number flag
    relative_number: bool, // :set relativenumber flag
}

impl Default for LineNumber {
    /// Default to hybrid line numbers (both number and relativenumber enabled)
    fn default() -> Self {
        Self {
            show: true,
            number: true,
            relative_number: true,
        }
    }
}

impl LineNumber {
    pub fn set_number(&mut self, enabled: bool) {
        self.number = enabled;
        self.update_state();
    }

    pub fn set_relative_number(&mut self, enabled: bool) {
        self.relative_number = enabled;
        self.update_state();
    }

    #[allow(clippy::missing_const_for_fn)]
    fn update_state(&mut self) {
        self.show = self.number || self.relative_number;
    }

    #[must_use]
    pub const fn mode(&self) -> LineNumberMode {
        match (self.number, self.relative_number) {
            (true, true) => LineNumberMode::Hybrid,
            (false, true) => LineNumberMode::Relative,
            _ => LineNumberMode::Absolute,
        }
    }

    #[must_use]
    pub const fn is_shown(&self) -> bool {
        self.show
    }
}

impl Window {
    /// Get buffer ID from content source (if applicable)
    #[must_use]
    pub const fn buffer_id(&self) -> Option<usize> {
        match &self.source {
            WindowContentSource::FileBuffer { buffer_id, .. }
            | WindowContentSource::PluginBuffer { buffer_id, .. } => Some(*buffer_id),
        }
    }

    /// Get buffer anchor from content source (if applicable)
    #[must_use]
    pub const fn buffer_anchor(&self) -> Option<Anchor> {
        match &self.source {
            WindowContentSource::FileBuffer { buffer_anchor, .. }
            | WindowContentSource::PluginBuffer { buffer_anchor, .. } => Some(*buffer_anchor),
        }
    }

    /// Set buffer anchor in content source (if applicable)
    #[allow(clippy::missing_const_for_fn)]
    pub fn set_buffer_anchor(&mut self, new_anchor: Anchor) {
        match &mut self.source {
            WindowContentSource::FileBuffer { buffer_anchor, .. }
            | WindowContentSource::PluginBuffer { buffer_anchor, .. } => {
                *buffer_anchor = new_anchor;
            }
        }
    }

    /// Set buffer ID in content source (if applicable)
    pub const fn set_buffer_id(&mut self, new_buffer_id: usize) {
        match &mut self.source {
            WindowContentSource::FileBuffer { buffer_id, .. }
            | WindowContentSource::PluginBuffer { buffer_id, .. } => {
                *buffer_id = new_buffer_id;
            }
        }
    }

    /// Render line number for a row
    #[must_use]
    pub fn render_line_number(
        &self,
        row: u16,
        cursor_y: u16,
        num_width: usize,
        theme: &Theme,
        color_mode: ColorMode,
    ) -> String {
        let Some(line_number) = &self.line_number else {
            return String::new();
        };
        if !line_number.show {
            return String::new();
        }

        let is_current_line = row == cursor_y;
        let line_num_style = if !self.is_active {
            &theme.gutter.inactive_line_number
        } else if is_current_line {
            &theme.gutter.current_line_number
        } else {
            &theme.gutter.line_number
        };

        // Active windows use configured mode, inactive always use absolute
        let num_str = if self.is_active {
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

        format!(
            "{}{num_str:>num_width$}{} ",
            line_num_style.to_ansi_start(color_mode),
            Style::ansi_reset()
        )
    }

    /// Get the effective cursor Y position for rendering
    /// Active window uses buffer cursor, inactive uses stored window cursor
    #[must_use]
    pub const fn effective_cursor_y(&self, buf: &Buffer) -> u16 {
        if self.is_active {
            buf.cur.y
        } else {
            self.cursor.y
        }
    }

    /// Build visual selection highlight if active
    fn build_visual_highlight(buf: &Buffer, theme: &Theme) -> Option<Highlight> {
        if !buf.selection.active {
            return None;
        }

        match buf.selection_mode() {
            SelectionMode::Block => {
                let (top_left, bottom_right) = buf.block_bounds();
                Some(Highlight::new(
                    Span::new(
                        u32::from(top_left.y),
                        u32::from(top_left.x),
                        u32::from(bottom_right.y),
                        u32::from(bottom_right.x) + 1,
                    ),
                    theme.selection.visual.clone(),
                    HighlightGroup::Visual,
                ))
            }
            SelectionMode::Character | SelectionMode::Line => {
                let (sel_start, sel_end) = buf.selection_bounds();
                Some(Highlight::new(
                    Span::new(
                        u32::from(sel_start.y),
                        u32::from(sel_start.x),
                        u32::from(sel_end.y),
                        u32::from(sel_end.x) + 1,
                    ),
                    theme.selection.visual.clone(),
                    HighlightGroup::Visual,
                ))
            }
        }
    }

    /// Apply indent guides to line content
    #[allow(clippy::cast_possible_truncation)]
    fn apply_indent_guides(
        content: &str,
        buf: &Buffer,
        row: u16,
        cursor_y: u16,
        indent_analyzer: &IndentAnalyzer,
        theme: &Theme,
        color_mode: ColorMode,
    ) -> String {
        if !indent_analyzer.is_enabled() {
            return content.to_string();
        }

        // Get cursor's indent level for active guide highlight
        let cursor_indent = if row == cursor_y {
            Some(indent_analyzer.indent_level(content))
        } else {
            buf.contents
                .get(cursor_y as usize)
                .map(|l| indent_analyzer.indent_level(&l.inner))
        };

        let guides = indent_analyzer.guides_for_line(content, cursor_indent);
        if guides.is_empty() {
            return content.to_string();
        }

        let mut result = String::new();
        let chars: Vec<char> = content.chars().collect();
        let mut col = 0u32;
        let mut guide_idx = 0;
        let tab_size = indent_analyzer.tab_size;

        // Process leading whitespace with guide injection
        for &ch in &chars {
            if ch != ' ' && ch != '\t' {
                break;
            }

            if guide_idx < guides.len() && guides[guide_idx].column == col {
                let style = if guides[guide_idx].active {
                    &theme.indent.active
                } else {
                    &theme.indent.guide
                };
                result.push_str(&style.to_ansi_start(color_mode));
                result.push(indent_analyzer.guide_char);
                result.push_str(Style::ansi_reset());
                guide_idx += 1;
            } else {
                result.push(ch);
            }

            col += if ch == '\t' { tab_size } else { 1 };
        }

        // Append the rest of the line (non-whitespace)
        let whitespace_chars = content
            .chars()
            .take_while(|&c| c == ' ' || c == '\t')
            .count();
        if whitespace_chars < chars.len() {
            let start_idx = content
                .char_indices()
                .nth(whitespace_chars)
                .map_or(0, |(i, _)| i);
            result.push_str(&content[start_idx..]);
        }

        result
    }

    /// Render a fold marker line
    #[allow(clippy::too_many_arguments)]
    fn render_fold_marker_line(
        &self,
        row: u16,
        cursor_y: u16,
        hidden_count: u32,
        preview: &str,
        num_width: usize,
        theme: &Theme,
        color_mode: ColorMode,
    ) -> String {
        let head = self.render_line_number(row, cursor_y, num_width, theme, color_mode);
        let fold_text = format!("+-- {hidden_count} lines: {preview} ---");
        let fold_style = &theme.fold.marker;
        let styled_fold =
            format!("{}{}{}", fold_style.to_ansi_start(color_mode), fold_text, Style::ansi_reset());
        head + &styled_fold
    }

    /// Render a normal content line
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::too_many_arguments)]
    fn render_content_line(
        &self,
        row: u16,
        buf: &Buffer,
        highlight_store: &HighlightStore,
        visual_highlight: Option<&Highlight>,
        is_block_mode: bool,
        num_width: usize,
        indent_analyzer: &IndentAnalyzer,
        theme: &Theme,
        color_mode: ColorMode,
    ) -> String {
        let Some(content) = buf.contents.get(row as usize) else {
            return String::new();
        };

        let cursor_y = self.effective_cursor_y(buf);
        let head = self.render_line_number(row, cursor_y, num_width, theme, color_mode);

        // Get highlights for this line
        let line_len = content.inner.chars().count() as u32;
        let mut line_highlights =
            highlight_store.get_line_highlights(buf.id, u32::from(row), line_len);

        // Add visual selection highlight if applicable
        if let Some(visual_hl) = visual_highlight {
            let cols = if is_block_mode {
                visual_hl.span.cols_for_line_block(u32::from(row), line_len)
            } else {
                visual_hl.span.cols_for_line(u32::from(row), line_len)
            };
            if let Some((start, end)) = cols
                && start < end
            {
                line_highlights =
                    self.merge_visual_highlight(line_highlights, start, end, &visual_hl.style);
            }
        }

        // Apply indent guides
        let line_with_guides = Self::apply_indent_guides(
            &content.inner,
            buf,
            row,
            cursor_y,
            indent_analyzer,
            theme,
            color_mode,
        );

        let styled_content =
            self.render_styled_line(&line_with_guides, &line_highlights, color_mode);
        head + &styled_content
    }

    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_precision_loss)]
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::cognitive_complexity)]
    pub fn render(
        &self,
        buf: &Buffer,
        highlight_store: &HighlightStore,
        color_mode: ColorMode,
        theme: &Theme,
        visibility_source: &dyn BufferVisibilitySource,
        indent_analyzer: &IndentAnalyzer,
    ) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();

        // Calculate line number width for alignment
        let total_lines = buf.contents.len();
        let num_width =
            if self.line_number.as_ref().is_some_and(LineNumber::is_shown) && total_lines > 0 {
                (total_lines as f64).log10().floor() as usize + 1
            } else {
                1
            };

        // Compute scrollbar state
        let scrollbar = self.compute_scrollbar_state(total_lines);

        // Get effective cursor position (buffer cursor for active, window cursor for inactive)
        let cursor_y = self.effective_cursor_y(buf);

        // Build visual selection highlight
        let visual_highlight = Self::build_visual_highlight(buf, theme);
        let is_block_mode = buf.selection.active && buf.selection_mode() == SelectionMode::Block;

        // Track buffer line position, accounting for folds
        let mut buffer_row = self.buffer_anchor().map_or(0, |a| a.y);
        let mut display_rows_rendered = 0u16;

        while display_rows_rendered < self.height && (buffer_row as usize) < buf.contents.len() {
            let row = buffer_row;

            // Check if this line is hidden (e.g., inside a collapsed fold)
            if visibility_source.is_hidden(buf.id, VisibilityQuery::Line(u32::from(row))) {
                buffer_row += 1;
                continue;
            }

            // Check if this line has a visibility marker (e.g., fold marker)
            let visibility_marker =
                visibility_source.get_marker(buf.id, VisibilityQuery::Line(u32::from(row)));

            let line_out = if let Some(marker) = visibility_marker {
                self.render_fold_marker_line(
                    row,
                    cursor_y,
                    marker.hidden_count,
                    &marker.preview,
                    num_width,
                    theme,
                    color_mode,
                )
            } else {
                self.render_content_line(
                    row,
                    buf,
                    highlight_store,
                    visual_highlight.as_ref(),
                    is_block_mode,
                    num_width,
                    indent_analyzer,
                    theme,
                    color_mode,
                )
            };

            // Append scrollbar character
            let scrollbar_char =
                Self::render_scrollbar_char(display_rows_rendered, scrollbar, theme, color_mode);
            lines.push(line_out + &scrollbar_char);
            buffer_row += 1;
            display_rows_rendered += 1;
        }

        // Fill remaining display rows with empty lines (with scrollbar)
        while display_rows_rendered < self.height {
            let scrollbar_char =
                Self::render_scrollbar_char(display_rows_rendered, scrollbar, theme, color_mode);
            lines.push(scrollbar_char);
            display_rows_rendered += 1;
        }

        lines
    }

    /// Merge visual selection highlight with existing highlights
    #[allow(clippy::unused_self)]
    #[allow(clippy::needless_pass_by_value)]
    fn merge_visual_highlight(
        &self,
        highlights: Vec<crate::highlight::store::LineHighlight>,
        start: u32,
        end: u32,
        visual_style: &Style,
    ) -> Vec<crate::highlight::store::LineHighlight> {
        use crate::highlight::store::LineHighlight;

        if highlights.is_empty() {
            // No existing highlights, just add visual selection
            return vec![LineHighlight {
                start_col: start,
                end_col: end,
                style: visual_style.clone(),
            }];
        }

        // Simple approach: merge visual style into overlapping regions
        let mut result: Vec<LineHighlight> = Vec::new();
        let mut current_pos = 0u32;

        for hl in &highlights {
            // Before this highlight
            if current_pos < hl.start_col {
                // Check if visual selection covers this gap
                let gap_start = current_pos.max(start);
                let gap_end = hl.start_col.min(end);
                if gap_start < gap_end {
                    // Visual selection in the gap before existing highlight
                    result.push(LineHighlight {
                        start_col: gap_start,
                        end_col: gap_end,
                        style: visual_style.clone(),
                    });
                }
            }

            // The highlight region itself
            let hl_in_visual = hl.start_col < end && hl.end_col > start;
            if hl_in_visual {
                // Split into: before visual, in visual, after visual
                if hl.start_col < start {
                    result.push(LineHighlight {
                        start_col: hl.start_col,
                        end_col: start,
                        style: hl.style.clone(),
                    });
                }
                let overlap_start = hl.start_col.max(start);
                let overlap_end = hl.end_col.min(end);
                if overlap_start < overlap_end {
                    result.push(LineHighlight {
                        start_col: overlap_start,
                        end_col: overlap_end,
                        style: hl.style.merge(visual_style),
                    });
                }
                if hl.end_col > end {
                    result.push(LineHighlight {
                        start_col: end,
                        end_col: hl.end_col,
                        style: hl.style.clone(),
                    });
                }
            } else {
                result.push(hl.clone());
            }

            current_pos = hl.end_col;
        }

        // After all highlights, check if visual selection extends further
        if current_pos < end && start < end {
            let final_start = current_pos.max(start);
            if final_start < end {
                result.push(LineHighlight {
                    start_col: final_start,
                    end_col: end,
                    style: visual_style.clone(),
                });
            }
        }

        result
    }

    /// Render a line with highlight ranges
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::unused_self)]
    fn render_styled_line(
        &self,
        line: &str,
        highlights: &[crate::highlight::store::LineHighlight],
        color_mode: ColorMode,
    ) -> String {
        if highlights.is_empty() {
            return line.to_string();
        }

        let mut result = String::new();
        let chars: Vec<char> = line.chars().collect();
        let mut current_col: u32 = 0;
        let mut hl_idx = 0;

        while (current_col as usize) < chars.len() {
            // Find if current position is in a highlight
            while hl_idx < highlights.len() && highlights[hl_idx].end_col <= current_col {
                hl_idx += 1;
            }

            if hl_idx < highlights.len() && highlights[hl_idx].start_col <= current_col {
                // We're inside a highlight
                let hl = &highlights[hl_idx];
                result.push_str(&hl.style.to_ansi_start(color_mode));

                while current_col < hl.end_col && (current_col as usize) < chars.len() {
                    result.push(chars[current_col as usize]);
                    current_col += 1;
                }

                result.push_str(Style::ansi_reset());
            } else {
                // Not in a highlight, output until next highlight or end
                let next_start = if hl_idx < highlights.len() {
                    highlights[hl_idx].start_col
                } else {
                    chars.len() as u32
                };

                while current_col < next_start && (current_col as usize) < chars.len() {
                    result.push(chars[current_col as usize]);
                    current_col += 1;
                }
            }
        }

        result
    }

    /// Render line number directly to frame buffer
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::too_many_arguments)]
    fn render_line_number_to_buffer(
        &self,
        buffer: &mut FrameBuffer,
        x: u16,
        y: u16,
        row: u16,
        cursor_y: u16,
        num_width: usize,
        theme: &Theme,
    ) -> u16 {
        let Some(line_number) = &self.line_number else {
            return 0;
        };
        if !line_number.show {
            return 0;
        }

        let is_current_line = row == cursor_y;
        let style = if !self.is_active {
            &theme.gutter.inactive_line_number
        } else if is_current_line {
            &theme.gutter.current_line_number
        } else {
            &theme.gutter.line_number
        };

        // Active windows use configured mode, inactive always use absolute
        let num_str = if self.is_active {
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

        // Right-align number and add space separator
        let formatted = format!("{num_str:>num_width$} ");
        let mut col = x;
        for ch in formatted.chars() {
            if col < buffer.width() {
                buffer.put_char(col, y, ch, style);
                col += 1;
            }
        }

        col.saturating_sub(x)
    }

    /// Render sign column to frame buffer
    ///
    /// Displays sign icon if present, otherwise renders background padding.
    /// Returns the width consumed (equal to `sign_column_width`).
    pub(crate) fn render_sign_to_buffer(
        buffer: &mut FrameBuffer,
        x: u16,
        y: u16,
        sign: Option<&crate::sign::Sign>,
        width: u16,
        theme: &Theme,
    ) -> u16 {
        if sign.is_some() {
            tracing::info!(
                "SIGN_RENDER: Rendering sign at ({}, {}) width={} sign={:?}",
                x,
                y,
                width,
                sign
            );
        }
        let bg_style = &theme.gutter.sign_column_bg;

        if let Some(sign) = sign {
            // Render sign icon (truncate if too long)
            let mut col = x;
            for ch in sign.icon.chars().take(width as usize) {
                buffer.put_char(col, y, ch, &sign.style);
                col += 1;
            }

            // Pad remaining width with background
            while col < x + width {
                buffer.put_char(col, y, ' ', bg_style);
                col += 1;
            }
        } else {
            // No sign - render background padding only
            for col in x..(x + width) {
                buffer.put_char(col, y, ' ', bg_style);
            }
        }

        width
    }

    /// Calculate the width needed for line numbers based on total line count.
    ///
    /// Returns the number of character columns required to display the largest line number.
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_precision_loss)]
    fn compute_line_number_width(&self, total_lines: usize) -> usize {
        if self.line_number.as_ref().is_some_and(LineNumber::is_shown) && total_lines > 0 {
            (total_lines as f64).log10().floor() as usize + 1
        } else {
            1
        }
    }

    /// Render a fold marker line (collapsed fold indicator) to frame buffer.
    ///
    /// Displays line number followed by fold indicator text "+-- folded ---".
    fn render_fold_marker_to_buffer(
        &self,
        buffer: &mut FrameBuffer,
        screen_y: u16,
        row: u16,
        cursor_y: u16,
        num_width: usize,
        theme: &Theme,
    ) {
        let gutter_width = self.render_line_number_to_buffer(
            buffer,
            self.anchor.x,
            screen_y,
            row,
            cursor_y,
            num_width,
            theme,
        );
        let fold_style = &theme.fold.marker;
        let fold_text = "+-- folded ---";
        let mut col = self.anchor.x + gutter_width;
        for ch in fold_text.chars() {
            if col < buffer.width() {
                buffer.put_char(col, screen_y, ch, fold_style);
                col += 1;
            }
        }
    }

    /// Fill remaining viewport rows with empty line markers (~).
    ///
    /// Renders tilde characters in the gutter for lines beyond the buffer content.
    fn render_empty_lines_to_buffer(
        &self,
        buffer: &mut FrameBuffer,
        start_display_row: u16,
        theme: &Theme,
    ) {
        let tilde_style = &theme.gutter.line_number;
        let mut display_row = start_display_row;
        while display_row < self.height {
            let screen_y = self.anchor.y + display_row;
            buffer.put_char(self.anchor.x, screen_y, '~', tilde_style);
            display_row += 1;
        }
    }

    /// Render a complete content line to frame buffer
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::too_many_arguments)]
    pub fn render_content_line_to_buffer(
        &self,
        buffer: &mut FrameBuffer,
        x: u16,
        y: u16,
        row: u16,
        cursor_y: u16,
        buf: &Buffer,
        highlight_store: &HighlightStore,
        visual_highlight: Option<&Highlight>,
        is_block_mode: bool,
        num_width: usize,
        _indent_analyzer: &IndentAnalyzer,
        theme: &Theme,
        decoration_store: Option<&DecorationStore>,
        edit_mode: &EditMode,
    ) -> u16 {
        let Some(content) = buf.contents.get(row as usize) else {
            return 0;
        };

        let mut col = x;

        // Render line number
        col += self.render_line_number_to_buffer(buffer, col, y, row, cursor_y, num_width, theme);

        // Get highlights for this line
        let line_len = content.inner.chars().count() as u32;
        let mut line_highlights =
            highlight_store.get_line_highlights(buf.id, u32::from(row), line_len);

        // Add visual selection highlight if applicable
        if let Some(visual_hl) = visual_highlight {
            let cols = if is_block_mode {
                visual_hl.span.cols_for_line_block(u32::from(row), line_len)
            } else {
                visual_hl.span.cols_for_line(u32::from(row), line_len)
            };
            if let Some((start, end)) = cols
                && start < end
            {
                line_highlights =
                    self.merge_visual_highlight(line_highlights, start, end, &visual_hl.style);
            }
        }

        // In insert mode, show raw content without decorations
        let show_raw = matches!(edit_mode, EditMode::Insert(_));

        // Get line background decoration if present (skip in insert mode)
        let line_bg = if show_raw {
            None
        } else {
            decoration_store.and_then(|ds| ds.get_line_background(buf.id, u32::from(row)))
        };

        // Apply line background if present
        if let Some(Decoration::LineBackground { style, .. }) = line_bg {
            // Fill line with background color (after gutter)
            let content_start = col;
            let content_end = x + self.width;
            for bg_col in content_start..content_end {
                buffer.put_char(bg_col, y, ' ', style);
            }
        }

        // Get conceals for this line (skip in insert mode to show raw markdown)
        let conceals = if show_raw {
            Vec::new()
        } else {
            decoration_store
                .map(|ds| ds.get_conceals(buf.id, u32::from(row)))
                .unwrap_or_default()
        };

        // Get inline styles for this line (skip in insert mode)
        let inline_styles = if show_raw {
            Vec::new()
        } else {
            decoration_store
                .map(|ds| ds.get_inline_styles(buf.id, u32::from(row)))
                .unwrap_or_default()
        };

        // Convert inline styles to spans: (start_col, end_col, style)
        let inline_style_spans: Vec<(u32, u32, Style)> = inline_styles
            .iter()
            .filter_map(|d| match d {
                Decoration::InlineStyle { span, style } => {
                    Some((span.start_col, span.end_col, style.clone()))
                }
                _ => None,
            })
            .collect();

        // Convert conceals to the format needed for apply_conceals
        // Format: (start_col, end_col, replacement_text, style)
        let conceal_spans: Vec<(u32, u32, Option<String>, Option<Style>)> = conceals
            .iter()
            .filter_map(|d| match d {
                Decoration::Conceal {
                    span,
                    replacement,
                    style,
                } => Some((span.start_col, span.end_col, Some(replacement.clone()), style.clone())),
                Decoration::Hide { span } => Some((span.start_col, span.end_col, None, None)),
                Decoration::LineBackground { .. } | Decoration::InlineStyle { .. } => None,
            })
            .collect();

        // Build the effective line content with conceals applied
        let (effective_content, char_mapping, style_overrides) =
            Self::apply_conceals(&content.inner, &conceal_spans);

        // Render styled content with conceals applied
        col += self.render_styled_line_with_conceals(
            buffer,
            col,
            y,
            &effective_content,
            &content.inner,
            &char_mapping,
            &style_overrides,
            &inline_style_spans,
            &line_highlights,
            &theme.base.default,
        );

        col.saturating_sub(x)
    }

    /// Apply conceals to a line and return the effective content with character mapping
    ///
    /// Returns (`effective_content`, `char_mapping`, `style_overrides`) where:
    /// - `char_mapping` maps effective positions back to original positions for highlight lookup
    /// - `style_overrides` contains the conceal's style for replacement chars (None for normal chars)
    #[allow(clippy::cast_possible_truncation)]
    fn apply_conceals(
        original: &str,
        conceals: &[(u32, u32, Option<String>, Option<Style>)],
    ) -> (String, Vec<Option<u32>>, Vec<Option<Style>>) {
        if conceals.is_empty() {
            // No conceals - direct mapping, no style overrides
            let len = original.chars().count();
            let mapping: Vec<Option<u32>> = (0..len as u32).map(Some).collect();
            let style_overrides: Vec<Option<Style>> = vec![None; len];
            return (original.to_string(), mapping, style_overrides);
        }

        let mut result = String::new();
        let mut mapping = Vec::new();
        let mut style_overrides = Vec::new();
        let chars: Vec<char> = original.chars().collect();
        let mut i = 0u32;

        // Sort conceals by start position
        let mut sorted_conceals = conceals.to_vec();
        sorted_conceals.sort_by_key(|(start, _, _, _)| *start);

        let mut conceal_idx = 0;

        while (i as usize) < chars.len() {
            // Check if we're at a conceal start
            if conceal_idx < sorted_conceals.len() && sorted_conceals[conceal_idx].0 == i {
                let (start, end, replacement, style) = &sorted_conceals[conceal_idx];

                // Add replacement text (if any)
                if let Some(repl) = replacement {
                    for ch in repl.chars() {
                        result.push(ch);
                        // Replacement chars map to the original start position
                        mapping.push(Some(*start));
                        // Store the conceal's style override
                        style_overrides.push(style.clone());
                    }
                }

                // Skip the concealed range
                i = *end;
                conceal_idx += 1;
            } else {
                // Normal character - add it with mapping, no style override
                result.push(chars[i as usize]);
                mapping.push(Some(i));
                style_overrides.push(None);
                i += 1;
            }
        }

        (result, mapping, style_overrides)
    }

    /// Render styled line with conceal support
    ///
    /// Uses character mapping to apply highlights from original positions to effective positions.
    /// Style overrides (from conceals) are merged with syntax highlights.
    /// Inline styles (italic, bold) are applied based on original column position.
    #[allow(clippy::too_many_arguments)]
    fn render_styled_line_with_conceals(
        &self,
        buffer: &mut FrameBuffer,
        x: u16,
        y: u16,
        effective_content: &str,
        _original_content: &str,
        char_mapping: &[Option<u32>],
        style_overrides: &[Option<Style>],
        inline_styles: &[(u32, u32, Style)],
        highlights: &[LineHighlight],
        default_style: &Style,
    ) -> u16 {
        let mut col = x;
        let max_col = x + self.width;

        for (eff_idx, ch) in effective_content.chars().enumerate() {
            if col >= max_col {
                break;
            }

            // Get the original position for highlight lookup
            let original_pos = char_mapping.get(eff_idx).copied().flatten();

            // Find syntax highlight style at original position
            let syntax_style = original_pos.map_or(default_style, |orig_col| {
                highlights
                    .iter()
                    .find(|hl| hl.start_col <= orig_col && orig_col < hl.end_col)
                    .map_or(default_style, |hl| &hl.style)
            });

            // Check for style override from conceal - merge with syntax style
            let mut final_style = style_overrides
                .get(eff_idx)
                .and_then(|opt| opt.as_ref())
                .map_or_else(
                    || syntax_style.clone(),
                    |override_style| {
                        // Merge: conceal style takes precedence, but inherit missing properties from syntax
                        syntax_style.merge(override_style)
                    },
                );

            // Apply inline styles (italic, bold) if applicable
            if let Some(orig_col) = original_pos {
                for (start, end, inline_style) in inline_styles {
                    if orig_col >= *start && orig_col < *end {
                        // Merge inline style with current style
                        final_style = final_style.merge(inline_style);
                        break;
                    }
                }
            }

            buffer.put_char(col, y, ch, &final_style);
            col += 1;
        }

        col.saturating_sub(x)
    }

    /// Render the entire window content to frame buffer
    #[allow(clippy::too_many_arguments)]
    pub fn render_to_buffer(
        &self,
        buffer: &mut FrameBuffer,
        buf: &Buffer,
        highlight_store: &HighlightStore,
        theme: &Theme,
        visibility_source: &dyn BufferVisibilitySource,
        indent_analyzer: &IndentAnalyzer,
        decoration_store: Option<&DecorationStore>,
        edit_mode: &EditMode,
    ) {
        // Calculate line number width for alignment
        let num_width = self.compute_line_number_width(buf.contents.len());

        // Build visual selection highlight
        let visual_highlight = Self::build_visual_highlight(buf, theme);
        let is_block_mode = buf.selection.active && buf.selection_mode() == SelectionMode::Block;

        // Get effective cursor position (buffer cursor for active, window cursor for inactive)
        let cursor_y = self.effective_cursor_y(buf);

        // Track buffer line position, accounting for folds
        let mut buffer_row = self.buffer_anchor().map_or(0, |a| a.y);
        let mut display_row = 0u16;

        while display_row < self.height && (buffer_row as usize) < buf.contents.len() {
            let row = buffer_row;

            // Check if this line is hidden (e.g., inside a collapsed fold)
            if visibility_source.is_hidden(buf.id, VisibilityQuery::Line(u32::from(row))) {
                buffer_row += 1;
                continue;
            }

            // Check if this line has a visibility marker (e.g., fold marker)
            let visibility_marker =
                visibility_source.get_marker(buf.id, VisibilityQuery::Line(u32::from(row)));

            let screen_y = self.anchor.y + display_row;

            if visibility_marker.is_some() {
                // Render fold marker
                self.render_fold_marker_to_buffer(
                    buffer, screen_y, row, cursor_y, num_width, theme,
                );
            } else {
                // Render normal content line
                if screen_y == self.anchor.y {
                    tracing::info!(
                        "render line: window.id={}, anchor.x={}, anchor.y={}, screen_y={}",
                        self.id,
                        self.anchor.x,
                        self.anchor.y,
                        screen_y
                    );
                }
                self.render_content_line_to_buffer(
                    buffer,
                    self.anchor.x,
                    screen_y,
                    row,
                    cursor_y,
                    buf,
                    highlight_store,
                    visual_highlight.as_ref(),
                    is_block_mode,
                    num_width,
                    indent_analyzer,
                    theme,
                    decoration_store,
                    edit_mode,
                );
            }

            buffer_row += 1;
            display_row += 1;
        }

        // Fill remaining rows with empty line markers
        self.render_empty_lines_to_buffer(buffer, display_row, theme);
    }

    pub fn set_number(&mut self, enabled: bool) {
        if let Some(line_number) = &mut self.line_number {
            line_number.set_number(enabled);
        }
    }

    pub fn set_relative_number(&mut self, enabled: bool) {
        if let Some(line_number) = &mut self.line_number {
            line_number.set_relative_number(enabled);
        }
    }

    /// Compute the content rectangle (area inside borders)
    ///
    /// Returns (x, y, width, height) of the area available for content.
    /// Takes adjacency into account for `OnCollide` mode.
    #[must_use]
    pub fn content_rect(&self, adjacency: &WindowAdjacency) -> (u16, u16, u16, u16) {
        let insets = self.border_insets(adjacency);
        let x = self.anchor.x + insets.left;
        let y = self.anchor.y + insets.top;
        let width = self.width.saturating_sub(insets.horizontal());
        let height = self.height.saturating_sub(insets.vertical());
        (x, y, width, height)
    }

    /// Get border insets for this window
    #[must_use]
    pub fn border_insets(&self, adjacency: &WindowAdjacency) -> BorderInsets {
        self.border_config
            .as_ref()
            .map_or(BorderInsets::ZERO, |config| {
                config.insets_with_context(adjacency, self.is_floating)
            })
    }

    /// Update `buffer_anchor` to keep cursor visible within the viewport
    ///
    /// Returns `true` if the viewport scrolled (anchor changed), `false` otherwise.
    pub fn update_scroll(&mut self, cursor_y: u16) -> bool {
        let visible_height = self.height;
        let Some(mut buffer_anchor) = self.buffer_anchor() else {
            return false;
        };
        let scroll_offset = buffer_anchor.y;

        // Scroll up if cursor is above visible area
        if cursor_y < scroll_offset {
            buffer_anchor.y = cursor_y;
            self.set_buffer_anchor(buffer_anchor);
            true
        }
        // Scroll down if cursor is below visible area
        else if cursor_y >= scroll_offset + visible_height {
            buffer_anchor.y = cursor_y.saturating_sub(visible_height) + 1;
            self.set_buffer_anchor(buffer_anchor);
            true
        } else {
            false
        }
    }

    /// Get the viewport bounds (`top_line`, `bottom_line`) for this window
    ///
    /// Returns (`top_line`, `bottom_line`) where both are 0-indexed buffer line numbers.
    /// `bottom_line` is the last visible line (inclusive).
    #[must_use]
    pub fn viewport_bounds(&self) -> (u32, u32) {
        let top_line = self.buffer_anchor().map_or(0, |a| u32::from(a.y));
        let bottom_line = top_line + u32::from(self.height).saturating_sub(1);
        (top_line, bottom_line)
    }

    /// Get the width of the line number gutter (including separator)
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_precision_loss)]
    pub fn line_number_width(&self, total_lines: usize) -> u16 {
        if self.line_number.as_ref().is_some_and(LineNumber::is_shown) {
            // Width of largest line number + 1 for space separator
            let digits = if total_lines == 0 {
                1
            } else {
                (total_lines as f64).log10().floor() as u16 + 1
            };
            digits + 1 // +1 for space separator
        } else {
            0
        }
    }

    /// Enable or disable scrollbar
    pub const fn set_scrollbar(&mut self, enabled: bool) {
        self.scrollbar_enabled = enabled;
    }

    /// Check if a screen position is within this window's bounds
    #[must_use]
    pub const fn contains_screen_position(&self, x: u16, y: u16) -> bool {
        x >= self.anchor.x
            && x < self.anchor.x + self.width
            && y >= self.anchor.y
            && y < self.anchor.y + self.height
    }

    /// Translate screen coordinates to buffer position
    ///
    /// Returns `Some(Position)` if the click is in the content area,
    /// `None` if the click is outside the window or in the gutter/scrollbar.
    ///
    /// # Arguments
    /// * `screen_x` - Screen column coordinate
    /// * `screen_y` - Screen row coordinate
    /// * `buffer_line_count` - Total number of lines in the buffer
    /// * `has_signs` - Whether the buffer has any signs to display
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn screen_to_buffer_position(
        &self,
        screen_x: u16,
        screen_y: u16,
        buffer_line_count: usize,
        has_signs: bool,
    ) -> Option<Position> {
        // Check if position is within window bounds
        if !self.contains_screen_position(screen_x, screen_y) {
            return None;
        }

        // Calculate window-relative position
        let window_x = screen_x - self.anchor.x;
        let window_y = screen_y - self.anchor.y;

        // Calculate gutter width (line numbers + sign column)
        let line_num_width = self.line_number_width(buffer_line_count);
        let sign_width = self.sign_column_mode.effective_width(has_signs);
        let gutter_width = line_num_width + sign_width;

        // Check if click is in gutter area
        if window_x < gutter_width {
            // Click in gutter - treat as first column of that line
            let buffer_y = self.buffer_anchor().map_or(0, |a| a.y) + window_y;
            return Some(Position { x: 0, y: buffer_y });
        }

        // Calculate content position (accounting for scrollbar on right)
        let scrollbar_width = u16::from(self.scrollbar_enabled);
        let content_width = self.width.saturating_sub(gutter_width + scrollbar_width);

        // Check if click is in scrollbar area
        if window_x >= gutter_width + content_width {
            return None; // Click on scrollbar - don't move cursor
        }

        // Calculate buffer position
        let content_x = window_x - gutter_width;
        let buffer_y = self.buffer_anchor().map_or(0, |a| a.y) + window_y;

        // Clamp to valid buffer bounds
        let max_y = buffer_line_count.saturating_sub(1) as u16;
        let clamped_y = buffer_y.min(max_y);

        Some(Position {
            x: content_x,
            y: clamped_y,
        })
    }

    /// Compute scrollbar state based on buffer content and viewport
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_precision_loss)]
    #[allow(clippy::cast_sign_loss)]
    pub fn compute_scrollbar_state(&self, total_lines: usize) -> ScrollbarState {
        if !self.scrollbar_enabled || total_lines == 0 || self.height == 0 {
            return ScrollbarState {
                enabled: false,
                thumb_start: 0,
                thumb_end: 0,
            };
        }

        let viewport_height = f64::from(self.height);
        let total = total_lines as f64;
        let scroll_offset = f64::from(self.buffer_anchor().map_or(0, |a| a.y));

        // Thumb size proportional to visible portion (minimum 1 row)
        let thumb_size = ((viewport_height / total) * viewport_height).max(1.0);

        // Thumb position based on scroll position
        // When scroll_offset = 0, thumb_start = 0
        // When scroll_offset = total_lines - viewport_height, thumb_start = viewport_height - thumb_size
        let scroll_range = (total - viewport_height).max(0.0);
        let thumb_pos = if scroll_range > 0.0 {
            (scroll_offset / scroll_range) * (viewport_height - thumb_size)
        } else {
            0.0
        };

        ScrollbarState {
            enabled: true,
            thumb_start: thumb_pos.floor() as u16,
            thumb_end: (thumb_pos + thumb_size).ceil() as u16,
        }
    }

    /// Render a scrollbar character for a given row
    #[must_use]
    fn render_scrollbar_char(
        row: u16,
        scrollbar: ScrollbarState,
        theme: &Theme,
        color_mode: ColorMode,
    ) -> String {
        if !scrollbar.enabled {
            return String::new();
        }

        let is_thumb = row >= scrollbar.thumb_start && row < scrollbar.thumb_end;
        let style = if is_thumb {
            &theme.scrollbar.thumb
        } else {
            &theme.scrollbar.track
        };

        // Use block characters for the scrollbar
        let ch = if is_thumb { '█' } else { '▕' };

        format!("{}{}{}", style.to_ansi_start(color_mode), ch, Style::ansi_reset())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[must_use]
    pub fn create_test_window(height: u16) -> Window {
        Window {
            id: 0,
            source: WindowContentSource::FileBuffer {
                buffer_id: 0,
                buffer_anchor: Anchor { x: 0, y: 0 },
            },
            anchor: Anchor { x: 0, y: 0 },
            width: 80,
            height,
            z_order: 100, // Editor window z-order range
            is_active: true,
            is_floating: false,
            line_number: Some(LineNumber::default()),
            scrollbar_enabled: false,
            sign_column_mode: SignColumnMode::No, // Disabled by default in tests to avoid position shifts
            cursor: Position { x: 0, y: 0 },
            desired_col: None,
            border_config: None,
        }
    }

    #[test]
    fn test_update_scroll_cursor_in_view() {
        let mut win = create_test_window(10);
        win.update_scroll(5); // cursor at line 5, viewport 0-9
        assert_eq!(win.buffer_anchor().map_or(0, |a| a.y), 0); // no scroll needed
    }

    #[test]
    fn test_update_scroll_cursor_below_viewport() {
        let mut win = create_test_window(10);
        win.update_scroll(15); // cursor at line 15, viewport 0-9
        assert_eq!(win.buffer_anchor().map_or(0, |a| a.y), 6); // scroll to show cursor at bottom
    }

    #[test]
    fn test_update_scroll_cursor_above_viewport() {
        let mut win = create_test_window(10);
        win.set_buffer_anchor(Anchor { x: 0, y: 20 }); // viewport starts at line 20
        win.update_scroll(5); // cursor at line 5
        assert_eq!(win.buffer_anchor().map_or(0, |a| a.y), 5); // scroll up to cursor
    }

    #[test]
    fn test_update_scroll_cursor_at_viewport_edge() {
        let mut win = create_test_window(10);
        win.update_scroll(9); // cursor at last visible line
        assert_eq!(win.buffer_anchor().map_or(0, |a| a.y), 0); // still in view

        win.update_scroll(10); // cursor just below viewport
        assert_eq!(win.buffer_anchor().map_or(0, |a| a.y), 1); // scroll by 1
    }
}
