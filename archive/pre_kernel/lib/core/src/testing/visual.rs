//! Visual testing utilities
//!
//! Provides helpers for testing visual output of the TUI,
//! including ASCII art snapshots and cell-level assertions.

use crate::visual::VisualSnapshot;

/// Trait for visual assertions on screen snapshots
pub trait VisualAssertions {
    /// Assert cell at position has expected character
    ///
    /// # Panics
    ///
    /// Panics if the cell doesn't match the expected character.
    fn assert_cell_char(&self, x: u16, y: u16, expected: char);

    /// Assert row content equals expected (trimmed)
    ///
    /// # Panics
    ///
    /// Panics if the row content doesn't match.
    fn assert_row_content(&self, y: u16, expected: &str);

    /// Assert row content starts with expected prefix
    ///
    /// # Panics
    ///
    /// Panics if the row doesn't start with the prefix.
    fn assert_row_starts_with(&self, y: u16, prefix: &str);

    /// Assert region contains expected text (concatenated across rows)
    ///
    /// # Panics
    ///
    /// Panics if the region doesn't contain the expected text.
    fn assert_region_contains(&self, x: u16, y: u16, width: u16, height: u16, text: &str);

    /// Assert the screen contains a pattern somewhere
    ///
    /// # Panics
    ///
    /// Panics if the pattern is not found.
    fn assert_contains(&self, pattern: &str);
}

impl VisualAssertions for VisualSnapshot {
    fn assert_cell_char(&self, x: u16, y: u16, expected: char) {
        let actual = self.char_at(x, y);
        assert_eq!(
            actual,
            Some(expected),
            "Cell at ({x}, {y}): expected '{expected}', got {actual:?}"
        );
    }

    fn assert_row_content(&self, y: u16, expected: &str) {
        let actual = self
            .row_to_string(y)
            .map(|s| s.trim_end().to_string())
            .unwrap_or_default();
        assert_eq!(actual.trim_end(), expected.trim_end(), "Row {y} content mismatch");
    }

    fn assert_row_starts_with(&self, y: u16, prefix: &str) {
        let actual = self.row_to_string(y).unwrap_or_default();
        assert!(
            actual.starts_with(prefix),
            "Row {y} doesn't start with '{prefix}'\nActual: '{actual}'"
        );
    }

    fn assert_region_contains(&self, x: u16, y: u16, width: u16, height: u16, text: &str) {
        let region = self.region_to_string(x, y, width, height);
        assert!(
            region.contains(text),
            "Region ({x}, {y}, {width}x{height}) doesn't contain '{text}'\nActual:\n{region}"
        );
    }

    fn assert_contains(&self, pattern: &str) {
        assert!(
            self.plain_text.contains(pattern),
            "Screen doesn't contain pattern '{pattern}'\nScreen content:\n{}",
            self.plain_text
        );
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::rpc::state::CellSnapshot};

    #[allow(clippy::cast_possible_truncation)]
    fn make_snapshot(rows: &[&str]) -> VisualSnapshot {
        let height = rows.len() as u16;
        let width = rows.iter().map(|s| s.len()).max().unwrap_or(0) as u16;

        let cells: Vec<Vec<CellSnapshot>> = rows
            .iter()
            .map(|row| {
                let mut cells: Vec<CellSnapshot> = row.chars().map(CellSnapshot::new).collect();
                // Pad to width
                while cells.len() < width as usize {
                    cells.push(CellSnapshot::new(' '));
                }
                cells
            })
            .collect();

        let plain_text = rows.join("\n");

        VisualSnapshot {
            width,
            height,
            cells,
            cursor: None,
            layers: Vec::new(),
            plain_text,
        }
    }

    #[test]
    fn test_assert_cell_char() {
        let snap = make_snapshot(&["Hello", "World"]);
        snap.assert_cell_char(0, 0, 'H');
        snap.assert_cell_char(4, 0, 'o');
        snap.assert_cell_char(0, 1, 'W');
    }

    #[test]
    fn test_assert_row_starts_with() {
        let snap = make_snapshot(&["  1 fn main() {", "  2     println!()"]);
        snap.assert_row_starts_with(0, "  1 fn");
        snap.assert_row_starts_with(1, "  2");
    }

    #[test]
    fn test_assert_contains() {
        let snap = make_snapshot(&["Hello World", "Foo Bar"]);
        snap.assert_contains("World");
        snap.assert_contains("Foo");
    }
}
