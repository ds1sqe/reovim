//! Mock render surfaces for testing.

use crate::{Rect, RenderSurface, Style, types::Color};

// =============================================================================
// RecordingSurface — cell-grid model
// =============================================================================

/// A cell in the recording surface grid.
#[derive(Debug, Clone)]
pub struct Cell {
    pub ch: char,
    pub style: Style,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            style: Style::new(),
        }
    }
}

/// Cell-grid render surface that records all drawing operations.
///
/// Use this when tests need to inspect rendered content at specific cells.
/// This is the primary surface for most module tests.
pub struct RecordingSurface {
    width: u16,
    height: u16,
    cells: Vec<Vec<Cell>>,
}

impl RecordingSurface {
    /// Create a new surface filled with spaces and default style.
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        let cells = (0..height)
            .map(|_| (0..width).map(|_| Cell::default()).collect())
            .collect();
        Self {
            width,
            height,
            cells,
        }
    }

    /// Get the character at (x, y). Panics if out of bounds.
    ///
    /// # Panics
    ///
    /// Panics if coordinates are outside the surface dimensions.
    #[must_use]
    pub fn char_at(&self, x: u16, y: u16) -> char {
        self.cells[y as usize][x as usize].ch
    }

    /// Get the style at (x, y). Panics if out of bounds.
    ///
    /// # Panics
    ///
    /// Panics if coordinates are outside the surface dimensions.
    #[must_use]
    pub fn style_at(&self, x: u16, y: u16) -> &Style {
        &self.cells[y as usize][x as usize].style
    }

    /// Whether any cell has been written (not default space).
    #[must_use]
    pub fn has_content(&self) -> bool {
        self.cells.iter().any(|row| row.iter().any(|c| c.ch != ' '))
    }

    /// Get the text content of a row, with trailing spaces trimmed.
    #[must_use]
    pub fn text_at_row(&self, y: u16) -> String {
        let row = &self.cells[y as usize];
        let s: String = row.iter().map(|c| c.ch).collect();
        s.trim_end().to_string()
    }

    /// Full snapshot of the surface as a string (rows joined by newlines).
    #[must_use]
    pub fn snapshot(&self) -> String {
        (0..self.height)
            .map(|y| self.text_at_row(y))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Assert that text at position matches expected. Panics with diff on mismatch.
    ///
    /// # Panics
    ///
    /// Panics if the text at position doesn't match expected.
    #[cfg_attr(coverage_nightly, coverage(off))]
    #[allow(clippy::cast_possible_truncation)] // expected.len() bounded by surface width
    pub fn assert_text_at(&self, x: u16, y: u16, expected: &str) {
        let actual: String = (0..expected.len())
            .map(|i| {
                let cx = x + i as u16;
                if cx < self.width {
                    self.cells[y as usize][cx as usize].ch
                } else {
                    ' '
                }
            })
            .collect();
        assert_eq!(
            actual, expected,
            "text mismatch at ({x}, {y}): expected {expected:?}, got {actual:?}"
        );
    }

    /// Assert that the style at position matches expected.
    ///
    /// # Panics
    ///
    /// Panics if the style at position doesn't match expected.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn assert_style_at(&self, x: u16, y: u16, expected: &Style) {
        let actual = self.style_at(x, y);
        assert_eq!(
            actual, expected,
            "style mismatch at ({x}, {y}): expected {expected:?}, got {actual:?}"
        );
    }

    /// Assert that the char at position matches expected.
    ///
    /// # Panics
    ///
    /// Panics if the character at position doesn't match expected.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn assert_char_at(&self, x: u16, y: u16, expected: char) {
        let actual = self.char_at(x, y);
        assert_eq!(
            actual, expected,
            "char mismatch at ({x}, {y}): expected {expected:?}, got {actual:?}"
        );
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl RenderSurface for RecordingSurface {
    fn write_styled(&mut self, x: u16, y: u16, text: &str, style: Style) -> u16 {
        if y >= self.height {
            return 0;
        }
        let mut col = x;
        for ch in text.chars() {
            if col >= self.width {
                break;
            }
            self.cells[y as usize][col as usize] = Cell {
                ch,
                style: style.clone(),
            };
            col += 1;
        }
        col.saturating_sub(x)
    }

    fn apply_style(&mut self, x: u16, y: u16, style: Style) {
        if y < self.height && x < self.width {
            self.cells[y as usize][x as usize].style = style;
        }
    }

    fn overlay_bg(&mut self, x: u16, y: u16, bg: Color) {
        if y < self.height && x < self.width {
            self.cells[y as usize][x as usize].style.bg = Some(bg);
        }
    }

    fn fill(&mut self, rect: Rect, ch: char, style: Style) {
        for y in rect.y..rect.y.saturating_add(rect.height).min(self.height) {
            for x in rect.x..rect.x.saturating_add(rect.width).min(self.width) {
                self.cells[y as usize][x as usize] = Cell {
                    ch,
                    style: style.clone(),
                };
            }
        }
    }

    fn clear(&mut self, rect: Rect) {
        self.fill(rect, ' ', Style::new());
    }

    fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }
}

// =============================================================================
// WriteSurface — write-log model
// =============================================================================

/// A recorded write operation.
#[derive(Debug, Clone)]
pub struct WriteEntry {
    pub x: u16,
    pub y: u16,
    pub text: String,
    pub style: Style,
}

/// Write-log render surface that records operations as a log.
///
/// Lighter weight than `RecordingSurface`. Use when tests only need to check
/// what was written at specific coordinates, not the full cell grid.
pub struct WriteSurface {
    width: u16,
    height: u16,
    writes: Vec<WriteEntry>,
}

impl WriteSurface {
    /// Create a new write-log surface.
    #[must_use]
    pub const fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            writes: Vec::new(),
        }
    }

    /// Get the text of the most recent write at (x, y).
    #[must_use]
    pub fn text_at(&self, x: u16, y: u16) -> Option<&str> {
        self.writes
            .iter()
            .rev()
            .find(|w| w.x == x && w.y == y)
            .map(|w| w.text.as_str())
    }

    /// Get the style of the most recent write at (x, y).
    #[cfg_attr(coverage_nightly, coverage(off))]
    #[must_use]
    pub fn style_at(&self, x: u16, y: u16) -> Option<&Style> {
        self.writes
            .iter()
            .rev()
            .find(|w| w.x == x && w.y == y)
            .map(|w| &w.style)
    }

    /// All recorded writes in order.
    #[must_use]
    pub fn writes(&self) -> &[WriteEntry] {
        &self.writes
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl RenderSurface for WriteSurface {
    #[allow(clippy::cast_possible_truncation)]
    fn write_styled(&mut self, x: u16, y: u16, text: &str, style: Style) -> u16 {
        self.writes.push(WriteEntry {
            x,
            y,
            text: text.to_string(),
            style,
        });
        text.len() as u16
    }

    fn apply_style(&mut self, _x: u16, _y: u16, _style: Style) {}

    fn overlay_bg(&mut self, _x: u16, _y: u16, _bg: Color) {}

    fn fill(&mut self, _rect: Rect, _ch: char, _style: Style) {}

    fn clear(&mut self, _rect: Rect) {}

    fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }
}
