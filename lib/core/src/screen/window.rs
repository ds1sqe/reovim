//! Window rendering module

use crate::buffer::{Buffer, SelectionMode, SelectionOps};
use crate::folding::FoldState;
use crate::highlight::{ColorMode, Highlight, HighlightGroup, HighlightStore, Span, Style, Theme};
use crate::indent::IndentAnalyzer;

use super::layout::WindowType;

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

/// Window is an intermediate object between buffer and screen
pub struct Window {
    /// Unique identifier for this window
    pub id: usize,
    /// Type of window (Editor, Explorer, etc.)
    pub window_type: WindowType,
    /// Where this window's top left is positioned on the screen
    pub anchor: Anchor,
    pub width: u16,
    pub height: u16,

    pub buffer_id: usize,
    /// Where this buffer's top left is positioned
    pub buffer_anchor: Anchor,
    pub line_number: LineNumber,
    /// Whether to show scrollbar
    pub scrollbar_enabled: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum LineNumberMode {
    Absolute,
    Relative,
    Hybrid,
}

#[derive(Debug, Default)]
pub struct LineNumber {
    show: bool,
    number: bool,          // :set number flag
    relative_number: bool, // :set relativenumber flag
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
}

impl Window {
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_precision_loss)]
    #[allow(clippy::single_match_else)]
    #[allow(clippy::collapsible_if)]
    #[allow(clippy::collapsible_else_if)]
    #[allow(clippy::option_if_let_else)]
    #[allow(clippy::if_not_else)]
    #[allow(clippy::too_many_lines)]
    pub fn render(
        &self,
        buf: &Buffer,
        highlight_store: &HighlightStore,
        color_mode: ColorMode,
        theme: &Theme,
        fold_state: Option<&FoldState>,
        indent_analyzer: &IndentAnalyzer,
    ) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();

        // Calculate line number width for alignment
        let total_lines = buf.contents.len();
        let num_width = if self.line_number.show && total_lines > 0 {
            (total_lines as f64).log10().floor() as usize + 1
        } else {
            1
        };

        // Compute scrollbar state
        let scrollbar = self.compute_scrollbar_state(total_lines);

        // Build visual selection highlight dynamically if active
        // Block mode needs special handling (same columns for all lines)
        let visual_highlight = if buf.selection.active {
            match buf.selection_mode() {
                SelectionMode::Block => {
                    let (top_left, bottom_right) = buf.block_bounds();
                    Some(Highlight::new(
                        Span::new(
                            u32::from(top_left.y),
                            u32::from(top_left.x),
                            u32::from(bottom_right.y),
                            u32::from(bottom_right.x) + 1, // +1 because end_col is exclusive
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
                            u32::from(sel_end.x) + 1, // +1 because end_col is exclusive
                        ),
                        theme.selection.visual.clone(),
                        HighlightGroup::Visual,
                    ))
                }
            }
        } else {
            None
        };
        let is_block_mode = buf.selection.active && buf.selection_mode() == SelectionMode::Block;

        // Track buffer line position, accounting for folds
        let mut buffer_row = self.buffer_anchor.y;
        let mut display_rows_rendered = 0u16;

        while display_rows_rendered < self.height && (buffer_row as usize) < buf.contents.len() {
            let row = buffer_row;

            // Check if this line is hidden inside a collapsed fold
            if let Some(fs) = fold_state
                && fs.is_line_hidden(u32::from(row))
            {
                buffer_row += 1;
                continue;
            }

            // Check if this line starts a collapsed fold
            let fold_marker = fold_state.and_then(|fs| fs.get_fold_marker(u32::from(row)));

            let line_out = if let Some((hidden_count, preview)) = fold_marker {
                // Render fold marker line
                let head = if self.line_number.show {
                    let is_current_line = row == buf.cur.y;
                    let line_num_style = if is_current_line {
                        &theme.gutter.current_line_number
                    } else {
                        &theme.gutter.line_number
                    };

                    let num_str = match self.line_number.mode() {
                        LineNumberMode::Absolute => format!("{}", row + 1),
                        LineNumberMode::Relative => {
                            let rel = (i32::from(row) - i32::from(buf.cur.y)).abs();
                            format!("{rel}")
                        }
                        LineNumberMode::Hybrid => {
                            if !is_current_line {
                                let rel = (i32::from(row) - i32::from(buf.cur.y)).abs();
                                format!("{rel}")
                            } else {
                                format!("{}", row + 1)
                            }
                        }
                    };
                    format!(
                        "{}{num_str:>num_width$}{} ",
                        line_num_style.to_ansi_start(color_mode),
                        Style::ansi_reset()
                    )
                } else {
                    String::new()
                };

                // Format fold marker: "+-- N lines: preview ---"
                let fold_text = format!("+-- {hidden_count} lines: {preview} ---");
                let fold_style = &theme.fold.marker;
                let styled_fold = format!(
                    "{}{}{}",
                    fold_style.to_ansi_start(color_mode),
                    fold_text,
                    Style::ansi_reset()
                );
                head + &styled_fold
            } else {
                // Render normal line
                let line_content = buf.contents.get(row as usize);
                match line_content {
                    Some(content) => {
                        let head = if self.line_number.show {
                            let is_current_line = row == buf.cur.y;
                            let line_num_style = if is_current_line {
                                &theme.gutter.current_line_number
                            } else {
                                &theme.gutter.line_number
                            };

                            let num_str = match self.line_number.mode() {
                                LineNumberMode::Absolute => format!("{}", row + 1),
                                LineNumberMode::Relative => {
                                    let rel = (i32::from(row) - i32::from(buf.cur.y)).abs();
                                    format!("{rel}")
                                }
                                LineNumberMode::Hybrid => {
                                    if !is_current_line {
                                        let rel = (i32::from(row) - i32::from(buf.cur.y)).abs();
                                        format!("{rel}")
                                    } else {
                                        format!("{}", row + 1)
                                    }
                                }
                            };
                            format!(
                                "{}{num_str:>num_width$}{} ",
                                line_num_style.to_ansi_start(color_mode),
                                Style::ansi_reset()
                            )
                        } else {
                            String::new()
                        };

                        // Get highlights for this line
                        let line_len = content.inner.chars().count() as u32;
                        let mut line_highlights =
                            highlight_store.get_line_highlights(buf.id, u32::from(row), line_len);

                        // Add visual selection highlight if applicable
                        if let Some(ref visual_hl) = visual_highlight {
                            let cols = if is_block_mode {
                                visual_hl.span.cols_for_line_block(u32::from(row), line_len)
                            } else {
                                visual_hl.span.cols_for_line(u32::from(row), line_len)
                            };
                            if let Some((start, end)) = cols {
                                if start < end {
                                    line_highlights = self.merge_visual_highlight(
                                        line_highlights,
                                        start,
                                        end,
                                        &visual_hl.style,
                                    );
                                }
                            }
                        }

                        // Apply indent guides if enabled
                        let line_with_guides = if indent_analyzer.is_enabled() {
                            // Get cursor's indent level for active guide highlight
                            let cursor_indent = if row == buf.cur.y {
                                Some(indent_analyzer.indent_level(&content.inner))
                            } else {
                                // Use cursor line's indent level for all lines
                                let cursor_line = buf.contents.get(buf.cur.y as usize);
                                cursor_line.map(|l| indent_analyzer.indent_level(&l.inner))
                            };

                            let guides = indent_analyzer.guides_for_line(&content.inner, cursor_indent);
                            if guides.is_empty() {
                                content.inner.clone()
                            } else {
                                // Build style strings for guides
                                // Use active style for active guide, normal style otherwise
                                let mut result = String::new();
                                let chars: Vec<char> = content.inner.chars().collect();
                                let mut col = 0u32;
                                let mut guide_idx = 0;
                                let tab_size = indent_analyzer.tab_size;

                                // Process leading whitespace with guide injection
                                for &ch in &chars {
                                    if ch != ' ' && ch != '\t' {
                                        break;
                                    }

                                    // Check if we should insert a guide at this position
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

                                    // Advance column position
                                    if ch == '\t' {
                                        col += tab_size;
                                    } else {
                                        col += 1;
                                    }
                                }

                                // Append the rest of the line (non-whitespace)
                                let whitespace_chars = content.inner
                                    .chars()
                                    .take_while(|&c| c == ' ' || c == '\t')
                                    .count();
                                if whitespace_chars < chars.len() {
                                    let start_idx = content.inner.char_indices().nth(whitespace_chars).map_or(0, |(i, _)| i);
                                    result.push_str(&content.inner[start_idx..]);
                                }

                                result
                            }
                        } else {
                            content.inner.clone()
                        };

                        let styled_content =
                            self.render_styled_line(&line_with_guides, &line_highlights, color_mode);

                        head + &styled_content
                    }
                    None => String::new(),
                }
            };

            // Append scrollbar character
            let scrollbar_char = Self::render_scrollbar_char(display_rows_rendered, scrollbar, theme, color_mode);
            lines.push(line_out + &scrollbar_char);
            buffer_row += 1;
            display_rows_rendered += 1;
        }

        // Fill remaining display rows with empty lines (with scrollbar)
        while display_rows_rendered < self.height {
            let scrollbar_char = Self::render_scrollbar_char(display_rows_rendered, scrollbar, theme, color_mode);
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

    pub fn set_number(&mut self, enabled: bool) {
        self.line_number.set_number(enabled);
    }

    pub fn set_relative_number(&mut self, enabled: bool) {
        self.line_number.set_relative_number(enabled);
    }

    /// Update `buffer_anchor` to keep cursor visible within the viewport
    pub const fn update_scroll(&mut self, cursor_y: u16) {
        let visible_height = self.height;
        let scroll_offset = self.buffer_anchor.y;

        // Scroll up if cursor is above visible area
        if cursor_y < scroll_offset {
            self.buffer_anchor.y = cursor_y;
        }
        // Scroll down if cursor is below visible area
        else if cursor_y >= scroll_offset + visible_height {
            self.buffer_anchor.y = cursor_y.saturating_sub(visible_height) + 1;
        }
    }

    /// Get the width of the line number gutter (including separator)
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_precision_loss)]
    pub fn line_number_width(&self, total_lines: usize) -> u16 {
        if self.line_number.show {
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
        let scroll_offset = f64::from(self.buffer_anchor.y);

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

        format!(
            "{}{}{}",
            style.to_ansi_start(color_mode),
            ch,
            Style::ansi_reset()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::layout::WindowType;

    fn create_test_window(height: u16) -> Window {
        Window {
            id: 0,
            window_type: WindowType::Editor,
            anchor: Anchor { x: 0, y: 0 },
            width: 80,
            height,
            buffer_id: 0,
            buffer_anchor: Anchor { x: 0, y: 0 },
            line_number: LineNumber::default(),
            scrollbar_enabled: false,
        }
    }

    #[test]
    fn test_update_scroll_cursor_in_view() {
        let mut win = create_test_window(10);
        win.update_scroll(5); // cursor at line 5, viewport 0-9
        assert_eq!(win.buffer_anchor.y, 0); // no scroll needed
    }

    #[test]
    fn test_update_scroll_cursor_below_viewport() {
        let mut win = create_test_window(10);
        win.update_scroll(15); // cursor at line 15, viewport 0-9
        assert_eq!(win.buffer_anchor.y, 6); // scroll to show cursor at bottom
    }

    #[test]
    fn test_update_scroll_cursor_above_viewport() {
        let mut win = create_test_window(10);
        win.buffer_anchor.y = 20; // viewport starts at line 20
        win.update_scroll(5); // cursor at line 5
        assert_eq!(win.buffer_anchor.y, 5); // scroll up to cursor
    }

    #[test]
    fn test_update_scroll_cursor_at_viewport_edge() {
        let mut win = create_test_window(10);
        win.update_scroll(9); // cursor at last visible line
        assert_eq!(win.buffer_anchor.y, 0); // still in view

        win.update_scroll(10); // cursor just below viewport
        assert_eq!(win.buffer_anchor.y, 1); // scroll by 1
    }
}
