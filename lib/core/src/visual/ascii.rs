//! ASCII art rendering for visual debugging
//!
//! Provides human-readable representations of the frame buffer
//! for debugging and test output.

use std::fmt::Write;

use crate::frame::FrameBuffer;

/// Configuration for ASCII rendering
#[derive(Debug, Clone, Default)]
pub struct AsciiRenderConfig {
    /// Show row numbers on the left
    pub show_row_numbers: bool,
    /// Show column numbers on top
    pub show_column_numbers: bool,
    /// Show border around content
    pub show_border: bool,
    /// Cursor position to highlight
    pub cursor: Option<(u16, u16)>,
    /// Character to display for cursor position
    pub cursor_char: char,
    /// Region annotations to display
    pub regions: Vec<RegionAnnotation>,
}

/// Annotation for a rectangular region
#[derive(Debug, Clone)]
pub struct RegionAnnotation {
    /// Name/label for the region
    pub name: String,
    /// Left edge
    pub x: u16,
    /// Top edge
    pub y: u16,
    /// Width
    pub width: u16,
    /// Height
    pub height: u16,
}

impl AsciiRenderConfig {
    /// Create a new config with defaults
    #[must_use]
    pub fn new() -> Self {
        Self {
            cursor_char: '\u{2588}', // Full block character
            ..Default::default()
        }
    }

    /// Enable all annotations (border, numbers)
    #[must_use]
    pub const fn annotated(mut self) -> Self {
        self.show_row_numbers = true;
        self.show_column_numbers = true;
        self.show_border = true;
        self
    }

    /// Set cursor position
    #[must_use]
    pub const fn with_cursor(mut self, x: u16, y: u16) -> Self {
        self.cursor = Some((x, y));
        self
    }

    /// Add a region annotation
    #[must_use]
    pub fn with_region(mut self, name: &str, x: u16, y: u16, width: u16, height: u16) -> Self {
        self.regions.push(RegionAnnotation {
            name: name.to_string(),
            x,
            y,
            width,
            height,
        });
        self
    }
}

impl FrameBuffer {
    /// Render to plain ASCII (characters only)
    ///
    /// Returns a string with newline-separated rows, trailing whitespace trimmed.
    #[must_use]
    pub fn to_ascii(&self) -> String {
        let mut result = String::new();

        for y in 0..self.height() {
            if y > 0 {
                result.push('\n');
            }
            if let Some(row) = self.row(y) {
                let row_str: String = row.iter().map(|c| c.char).collect();
                result.push_str(row_str.trim_end());
            }
        }

        result
    }

    /// Render to annotated ASCII with borders, line numbers, and cursor
    ///
    /// # Example output
    /// ```text
    ///    0123456789
    ///   +----------+
    ///  0|Hello     | [cursor: (2, 0)]
    ///  1|World     |
    ///   +----------+
    /// ```
    #[must_use]
    pub fn to_annotated_ascii(&self, config: &AsciiRenderConfig) -> String {
        let mut result = String::new();
        let width = self.width();
        let height = self.height();

        // Calculate row number width (for alignment)
        let row_num_width = if config.show_row_numbers {
            format!("{}", height.saturating_sub(1)).len()
        } else {
            0
        };

        // Column numbers
        if config.show_column_numbers {
            // First row: tens digit (only for cols >= 10)
            if width >= 10 {
                result
                    .push_str(&" ".repeat(row_num_width + if config.show_border { 2 } else { 0 }));
                for x in 0..width {
                    if x >= 10 {
                        let _ = write!(result, "{}", (x / 10) % 10);
                    } else {
                        result.push(' ');
                    }
                }
                result.push('\n');
            }
            // Second row: ones digit
            result.push_str(&" ".repeat(row_num_width + if config.show_border { 2 } else { 0 }));
            for x in 0..width {
                let _ = write!(result, "{}", x % 10);
            }
            result.push('\n');
        }

        // Top border
        if config.show_border {
            result.push_str(&" ".repeat(row_num_width + 1));
            result.push('+');
            result.push_str(&"-".repeat(width as usize));
            result.push('+');
            result.push('\n');
        }

        // Content rows
        for y in 0..height {
            // Row number
            if config.show_row_numbers {
                let _ = write!(result, "{y:>row_num_width$}");
            }

            // Left border
            if config.show_border {
                result.push('|');
            }

            // Row content
            if let Some(row) = self.row(y) {
                #[allow(clippy::cast_possible_truncation)]
                for (x, cell) in row.iter().enumerate() {
                    let x = x as u16;
                    // Check if this is the cursor position
                    if config.cursor == Some((x, y)) {
                        result.push(config.cursor_char);
                    } else {
                        result.push(cell.char);
                    }
                }
            }

            // Right border
            if config.show_border {
                result.push('|');
            }

            // Cursor annotation (on the right side)
            if let Some((cx, cy)) = config.cursor
                && cy == y
            {
                let _ = write!(result, " [cursor: ({cx}, {cy})]");
            }

            result.push('\n');
        }

        // Bottom border
        if config.show_border {
            result.push_str(&" ".repeat(row_num_width + 1));
            result.push('+');
            result.push_str(&"-".repeat(width as usize));
            result.push('+');
            result.push('\n');
        }

        // Region annotations
        if !config.regions.is_empty() {
            result.push_str("\nRegions:\n");
            for region in &config.regions {
                let _ = writeln!(
                    result,
                    "  - {}: ({},{}) {}x{}",
                    region.name, region.x, region.y, region.width, region.height
                );
            }
        }

        result
    }

    /// Get a single row as a string (trailing whitespace trimmed)
    #[must_use]
    pub fn row_to_string(&self, y: u16) -> Option<String> {
        self.row(y).map(|row| {
            let s: String = row.iter().map(|c| c.char).collect();
            s.trim_end().to_string()
        })
    }

    /// Get text from a rectangular region
    #[must_use]
    pub fn region_to_string(&self, x: u16, y: u16, width: u16, height: u16) -> String {
        let mut result = String::new();

        for row_idx in y..y.saturating_add(height).min(self.height()) {
            if row_idx > y {
                result.push('\n');
            }
            if let Some(row) = self.row(row_idx) {
                for col_idx in x..x.saturating_add(width).min(self.width()) {
                    if let Some(cell) = row.get(col_idx as usize) {
                        result.push(cell.char);
                    }
                }
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::frame::Cell};

    #[allow(clippy::cast_possible_truncation)]
    fn make_buffer(content: &[&str]) -> FrameBuffer {
        let height = content.len() as u16;
        let width = content.iter().map(|s| s.len()).max().unwrap_or(0) as u16;
        let mut buf = FrameBuffer::new(width, height);

        for (y, line) in content.iter().enumerate() {
            for (x, ch) in line.chars().enumerate() {
                buf.set(x as u16, y as u16, Cell::from_char(ch));
            }
        }

        buf
    }

    #[test]
    fn test_to_ascii() {
        let buf = make_buffer(&["Hello", "World"]);
        let ascii = buf.to_ascii();
        assert_eq!(ascii, "Hello\nWorld");
    }

    #[test]
    fn test_to_ascii_trims_trailing_whitespace() {
        let buf = make_buffer(&["Hi   ", "  "]);
        let ascii = buf.to_ascii();
        assert_eq!(ascii, "Hi\n");
    }

    #[test]
    fn test_row_to_string() {
        let buf = make_buffer(&["First", "Second"]);
        assert_eq!(buf.row_to_string(0), Some("First".to_string()));
        assert_eq!(buf.row_to_string(1), Some("Second".to_string()));
        assert_eq!(buf.row_to_string(2), None);
    }

    #[test]
    fn test_region_to_string() {
        let buf = make_buffer(&["ABCDE", "FGHIJ", "KLMNO"]);
        // Extract a 2x2 region starting at (1, 1)
        let region = buf.region_to_string(1, 1, 2, 2);
        assert_eq!(region, "GH\nLM");
    }

    #[test]
    fn test_annotated_ascii_with_cursor() {
        let buf = make_buffer(&["Hi"]);
        let config = AsciiRenderConfig::new().with_cursor(1, 0);
        let annotated = buf.to_annotated_ascii(&config);
        assert!(annotated.contains("[cursor: (1, 0)]"));
    }
}
