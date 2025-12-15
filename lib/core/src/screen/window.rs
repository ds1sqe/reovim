use crate::buffer::Buffer;
use crate::screen::Position;

/// ANSI escape codes for selection highlighting
/// Using explicit background color instead of reverse video to avoid
/// conflicts with terminal cursor (which often uses reverse video)
const SELECTION_ON: &str = "\x1b[48;5;240m";  // Gray background (256-color)
const SELECTION_OFF: &str = "\x1b[49m";        // Default background

/// represent top left corner
#[derive(Clone, Copy)]
pub struct Anchor {
    pub x: u16,
    pub y: u16,
}

/// window is a intermediate object between buffer and screen
pub struct Window {
    // where this window's of top left positioned on the screen is
    pub anchor: Anchor,
    pub width: u16,
    pub height: u16,

    pub buffer_id: usize,
    // where this buffer's top left positioned is
    pub buffer_anchor: Anchor,
    pub line_number: LineNumber,
}

pub enum LineNumberMode {
    Absolute,
    Relative,
    Hybrid,
}

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

    fn update_state(&mut self) {
        self.show = self.number || self.relative_number;
    }

    pub fn mode(&self) -> LineNumberMode {
        match (self.number, self.relative_number) {
            (true, true) => LineNumberMode::Hybrid,
            (false, true) => LineNumberMode::Relative,
            _ => LineNumberMode::Absolute,
        }
    }
}

impl Default for LineNumber {
    fn default() -> Self {
        LineNumber {
            show: false,
            number: false,
            relative_number: false,
        }
    }
}

impl Window {
    /// Check if a position is within selection bounds
    fn is_selected(&self, row: u16, col: u16, sel_start: &Position, sel_end: &Position) -> bool {
        if row < sel_start.y || row > sel_end.y {
            return false;
        }
        if row == sel_start.y && row == sel_end.y {
            // Single line selection
            col >= sel_start.x && col <= sel_end.x
        } else if row == sel_start.y {
            // First line of multi-line selection
            col >= sel_start.x
        } else if row == sel_end.y {
            // Last line of multi-line selection
            col <= sel_end.x
        } else {
            // Middle lines are fully selected
            true
        }
    }

    pub fn render(&self, buf: &Buffer) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();

        // Get selection bounds if active
        let selection_active = buf.selection.active;
        let (sel_start, sel_end) = if selection_active {
            buf.selection_bounds()
        } else {
            (Position::default(), Position::default())
        };

        // Calculate line number width for alignment
        let total_lines = buf.contents.len();
        let num_width = if self.line_number.show && total_lines > 0 {
            (total_lines as f64).log10().floor() as usize + 1
        } else {
            1
        };

        for row in self.buffer_anchor.y..(self.height + self.buffer_anchor.y) {
            let line_content = buf.contents.get(row as usize);
            let line_out = match line_content {
                Some(content) => {
                    let head = if self.line_number.show {
                        let num_str = match self.line_number.mode() {
                            LineNumberMode::Absolute => format!("{}", row + 1), // 1-indexed
                            LineNumberMode::Relative => {
                                let rel = (row as i32 - buf.cur.y as i32).abs();
                                format!("{rel}")
                            }
                            LineNumberMode::Hybrid => {
                                if row != buf.cur.y {
                                    let rel = (row as i32 - buf.cur.y as i32).abs();
                                    format!("{rel}")
                                } else {
                                    format!("{}", row + 1) // Show absolute on cursor line
                                }
                            }
                        };
                        // Right-align the number and add space separator
                        format!("{:>width$} ", num_str, width = num_width)
                    } else {
                        "".to_string()
                    };

                    // Render content with selection highlighting
                    let styled_content = if selection_active {
                        self.render_line_with_selection(
                            &content.inner,
                            row,
                            &sel_start,
                            &sel_end,
                        )
                    } else {
                        content.inner.clone()
                    };

                    head + &styled_content
                }
                None => "".to_string(),
            };
            lines.push(line_out);
        }
        lines
    }

    /// Render a line with selection highlighting
    fn render_line_with_selection(
        &self,
        line: &str,
        row: u16,
        sel_start: &Position,
        sel_end: &Position,
    ) -> String {
        let mut result = String::new();

        for (col, ch) in line.chars().enumerate() {
            let selected = self.is_selected(row, col as u16, sel_start, sel_end);

            if selected {
                // Apply selection highlighting with background color
                result.push_str(SELECTION_ON);
                result.push(ch);
                result.push_str(SELECTION_OFF);
            } else {
                result.push(ch);
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

    // TODO: split
}
