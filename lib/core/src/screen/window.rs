//! Window rendering module

use crate::buffer::{Buffer, SelectionOps};
use crate::highlight::{ColorMode, Highlight, HighlightGroup, HighlightStore, Span, Style, Theme};

use super::layout::WindowType;

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
    pub fn render(
        &self,
        buf: &Buffer,
        highlight_store: &HighlightStore,
        color_mode: ColorMode,
        theme: &Theme,
    ) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();

        // Calculate line number width for alignment
        let total_lines = buf.contents.len();
        let num_width = if self.line_number.show && total_lines > 0 {
            (total_lines as f64).log10().floor() as usize + 1
        } else {
            1
        };

        // Build visual selection highlight dynamically if active
        let visual_highlight = if buf.selection.active {
            let (sel_start, sel_end) = buf.selection_bounds();
            Some(Highlight::new(
                Span::new(
                    u32::from(sel_start.y),
                    u32::from(sel_start.x),
                    u32::from(sel_end.y),
                    u32::from(sel_end.x) + 1, // +1 because end_col is exclusive
                ),
                theme.visual_selection.clone(),
                HighlightGroup::Visual,
            ))
        } else {
            None
        };

        for row in self.buffer_anchor.y..(self.height + self.buffer_anchor.y) {
            let line_content = buf.contents.get(row as usize);
            let line_out = match line_content {
                Some(content) => {
                    let head = if self.line_number.show {
                        // Determine if this is the current line
                        let is_current_line = row == buf.cur.y;
                        let line_num_style = if is_current_line {
                            &theme.current_line_number
                        } else {
                            &theme.line_number
                        };

                        let num_str = match self.line_number.mode() {
                            LineNumberMode::Absolute => format!("{}", row + 1), // 1-indexed
                            LineNumberMode::Relative => {
                                let rel = (i32::from(row) - i32::from(buf.cur.y)).abs();
                                format!("{rel}")
                            }
                            LineNumberMode::Hybrid => {
                                if !is_current_line {
                                    let rel = (i32::from(row) - i32::from(buf.cur.y)).abs();
                                    format!("{rel}")
                                } else {
                                    format!("{}", row + 1) // Show absolute on cursor line
                                }
                            }
                        };
                        // Right-align the number with theme styling and add space separator
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
                        if let Some((start, end)) =
                            visual_hl.span.cols_for_line(u32::from(row), line_len)
                        {
                            if start < end {
                                // Merge visual highlight with stored highlights
                                line_highlights = self.merge_visual_highlight(
                                    line_highlights,
                                    start,
                                    end,
                                    &visual_hl.style,
                                );
                            }
                        }
                    }

                    let styled_content =
                        self.render_styled_line(&content.inner, &line_highlights, color_mode);

                    head + &styled_content
                }
                None => String::new(),
            };
            lines.push(line_out);
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
}
