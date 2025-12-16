/// A range of text within a buffer (line-based coordinates)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    /// exclusive
    pub end_col: u32,
}

impl Span {
    #[must_use]
    pub const fn new(start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> Self {
        Self {
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    /// Create a single-line span
    #[must_use]
    pub const fn single_line(line: u32, start_col: u32, end_col: u32) -> Self {
        Self::new(line, start_col, line, end_col)
    }

    /// Check if a position (line, col) falls within this span
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn contains(&self, line: u32, col: u32) -> bool {
        if line < self.start_line || line > self.end_line {
            return false;
        }
        if self.start_line == self.end_line {
            // Single line span
            col >= self.start_col && col < self.end_col
        } else if line == self.start_line {
            col >= self.start_col
        } else if line == self.end_line {
            col < self.end_col
        } else {
            // Middle lines are fully contained
            true
        }
    }

    /// Get the column range for a specific line within this span.
    /// Returns `(start_col, end_col)` where `end_col` is exclusive.
    /// Returns None if line is not within this span.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn cols_for_line(&self, line: u32, line_len: u32) -> Option<(u32, u32)> {
        if line < self.start_line || line > self.end_line {
            return None;
        }

        let start = if line == self.start_line {
            self.start_col
        } else {
            0
        };

        let end = if line == self.end_line {
            self.end_col
        } else {
            line_len
        };

        Some((start, end))
    }

    /// Check if this span intersects with another span
    #[must_use]
    pub const fn intersects(&self, other: &Self) -> bool {
        !(self.end_line < other.start_line
            || other.end_line < self.start_line
            || (self.end_line == other.start_line && self.end_col <= other.start_col)
            || (other.end_line == self.start_line && other.end_col <= self.start_col))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_line_contains() {
        let span = Span::single_line(5, 10, 20);
        assert!(!span.contains(4, 15));
        assert!(!span.contains(6, 15));
        assert!(!span.contains(5, 9));
        assert!(span.contains(5, 10));
        assert!(span.contains(5, 15));
        assert!(span.contains(5, 19));
        assert!(!span.contains(5, 20)); // end_col is exclusive
    }

    #[test]
    fn test_multiline_contains() {
        let span = Span::new(2, 5, 4, 10);
        // Before span
        assert!(!span.contains(1, 0));
        assert!(!span.contains(2, 4));
        // Start line
        assert!(span.contains(2, 5));
        assert!(span.contains(2, 100));
        // Middle line
        assert!(span.contains(3, 0));
        assert!(span.contains(3, 100));
        // End line
        assert!(span.contains(4, 0));
        assert!(span.contains(4, 9));
        assert!(!span.contains(4, 10));
        // After span
        assert!(!span.contains(5, 0));
    }

    #[test]
    fn test_cols_for_line() {
        let span = Span::new(2, 5, 4, 10);
        assert_eq!(span.cols_for_line(1, 50), None);
        assert_eq!(span.cols_for_line(2, 50), Some((5, 50)));
        assert_eq!(span.cols_for_line(3, 50), Some((0, 50)));
        assert_eq!(span.cols_for_line(4, 50), Some((0, 10)));
        assert_eq!(span.cols_for_line(5, 50), None);
    }
}
