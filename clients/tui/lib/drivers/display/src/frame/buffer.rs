//! Frame buffer for 2D cell grid storage.
//!
//! The frame buffer stores a grid of cells representing the terminal display.
//! It provides efficient access and manipulation of cells by (x, y) coordinates.

use crate::compositor::Style;

use super::cell::{Cell, char_width};

/// 2D cell grid with row-major storage.
///
/// The buffer stores cells in a flat vector for cache efficiency.
/// Coordinates are (x, y) where x is column and y is row, both 0-indexed.
#[derive(Debug, Clone)]
pub struct FrameBuffer {
    cells: Vec<Cell>,
    width: u16,
    height: u16,
}

impl FrameBuffer {
    /// Create a new frame buffer with the given dimensions.
    ///
    /// All cells are initialized to empty (space with default style).
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        let size = usize::from(width) * usize::from(height);
        Self {
            cells: vec![Cell::empty(); size],
            width,
            height,
        }
    }

    /// Resize the buffer to new dimensions.
    ///
    /// If the buffer grows, new cells are initialized to empty.
    /// If the buffer shrinks, excess cells are discarded.
    /// Existing cells that fit in the new dimensions are preserved.
    pub fn resize(&mut self, width: u16, height: u16) {
        if width == self.width && height == self.height {
            return;
        }

        let new_size = usize::from(width) * usize::from(height);
        let mut new_cells = vec![Cell::empty(); new_size];

        // Copy existing cells that fit
        let copy_width = self.width.min(width);
        let copy_height = self.height.min(height);

        for y in 0..copy_height {
            for x in 0..copy_width {
                if let Some(cell) = self.get(x, y) {
                    let new_idx = usize::from(y) * usize::from(width) + usize::from(x);
                    new_cells[new_idx] = cell.clone();
                }
            }
        }

        self.cells = new_cells;
        self.width = width;
        self.height = height;
    }

    /// Get the buffer width in columns.
    #[must_use]
    pub const fn width(&self) -> u16 {
        self.width
    }

    /// Get the buffer height in rows.
    #[must_use]
    pub const fn height(&self) -> u16 {
        self.height
    }

    /// Get a reference to a cell at (x, y), if in bounds.
    #[must_use]
    pub fn get(&self, x: u16, y: u16) -> Option<&Cell> {
        if x < self.width && y < self.height {
            let idx = self.index(x, y);
            self.cells.get(idx)
        } else {
            None
        }
    }

    /// Get a mutable reference to a cell at (x, y), if in bounds.
    #[must_use]
    pub fn get_mut(&mut self, x: u16, y: u16) -> Option<&mut Cell> {
        if x < self.width && y < self.height {
            let idx = self.index(x, y);
            self.cells.get_mut(idx)
        } else {
            None
        }
    }

    /// Set a cell at (x, y), if in bounds.
    pub fn set(&mut self, x: u16, y: u16, cell: Cell) {
        if x < self.width && y < self.height {
            let idx = self.index(x, y);
            self.cells[idx] = cell;
        }
    }

    /// Put a character at (x, y) with the given style.
    ///
    /// For wide characters, also sets the continuation cell at (x+1, y).
    pub fn put_char(&mut self, x: u16, y: u16, char: char, style: &Style) {
        let width = char_width(char);
        let cell = Cell::new(char, style.clone());
        self.set(x, y, cell);

        // Handle wide characters
        if width == 2 && x + 1 < self.width {
            self.set(x + 1, y, Cell::continuation());
        }
    }

    /// Apply style to existing cell without replacing character.
    ///
    /// Used for cursor overlays where we want to show both the character
    /// and a cursor indicator (via background color). The overlay style's
    /// colors take precedence over existing colors.
    pub fn apply_style(&mut self, x: u16, y: u16, style: &Style) {
        if let Some(cell) = self.get_mut(x, y) {
            // Merge overlay style with existing cell style
            // Overlay's colors take precedence (for cursor highlighting)
            if let Some(bg) = style.bg {
                cell.style.bg = Some(bg);
            }
            if let Some(fg) = style.fg {
                cell.style.fg = Some(fg);
            }
            cell.style.attributes = cell.style.attributes.union(style.attributes);
            if let Some(underline_color) = style.underline_color {
                cell.style.underline_color = Some(underline_color);
            }
        }
    }

    /// Write a string starting at (x, y) with the given style.
    ///
    /// Returns the number of columns used (accounting for wide characters).
    /// Stops at the right edge of the buffer (does not wrap).
    pub fn write_str(&mut self, x: u16, y: u16, s: &str, style: &Style) -> u16 {
        let mut col = x;
        for char in s.chars() {
            if col >= self.width {
                break;
            }

            let width = char_width(char);

            // Check if wide char would go past the edge
            if width == 2 && col + 1 >= self.width {
                // Put a space instead of truncating a wide char
                self.set(col, y, Cell::new(' ', style.clone()));
                col += 1;
                break;
            }

            self.put_char(col, y, char, style);
            col += u16::from(width);
        }
        col - x
    }

    /// Fill a rectangular region with a cell.
    pub fn fill_rect(&mut self, x: u16, y: u16, w: u16, h: u16, cell: &Cell) {
        for row in y..y.saturating_add(h).min(self.height) {
            for col in x..x.saturating_add(w).min(self.width) {
                self.set(col, row, cell.clone());
            }
        }
    }

    /// Clear the entire buffer (fill with empty cells).
    pub fn clear(&mut self) {
        self.cells.fill(Cell::empty());
    }

    /// Clear a rectangular region (fill with empty cells).
    pub fn clear_rect(&mut self, x: u16, y: u16, w: u16, h: u16) {
        self.fill_rect(x, y, w, h, &Cell::empty());
    }

    /// Get a slice of cells for a single row.
    #[must_use]
    pub fn row(&self, y: u16) -> Option<&[Cell]> {
        if y < self.height {
            let start = usize::from(y) * usize::from(self.width);
            let end = start + usize::from(self.width);
            Some(&self.cells[start..end])
        } else {
            None
        }
    }

    /// Copy contents from another buffer.
    ///
    /// Only copies the overlapping region if sizes differ.
    pub fn copy_from(&mut self, other: &Self) {
        let copy_width = self.width.min(other.width);
        let copy_height = self.height.min(other.height);

        for y in 0..copy_height {
            for x in 0..copy_width {
                if let Some(cell) = other.get(x, y) {
                    self.set(x, y, cell.clone());
                }
            }
        }
    }

    /// Swap contents with another buffer.
    ///
    /// Both buffers must have the same dimensions.
    ///
    /// # Panics
    ///
    /// Panics if the buffers have different widths or heights.
    pub fn swap_with(&mut self, other: &mut Self) {
        assert_eq!(self.width, other.width, "Buffer widths must match for swap");
        assert_eq!(self.height, other.height, "Buffer heights must match for swap");
        std::mem::swap(&mut self.cells, &mut other.cells);
    }

    /// Calculate the flat index for (x, y) coordinates.
    #[inline]
    fn index(&self, x: u16, y: u16) -> usize {
        usize::from(y) * usize::from(self.width) + usize::from(x)
    }
}

impl Default for FrameBuffer {
    fn default() -> Self {
        Self::new(80, 24) // Standard terminal size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let buf = FrameBuffer::new(10, 5);
        assert_eq!(buf.width(), 10);
        assert_eq!(buf.height(), 5);
    }

    #[test]
    fn test_get_set() {
        let mut buf = FrameBuffer::new(10, 10);

        let cell = Cell::from_char('x');
        buf.set(5, 5, cell.clone());

        assert_eq!(buf.get(5, 5), Some(&cell));
        assert_eq!(buf.get(0, 0), Some(&Cell::empty()));
    }

    #[test]
    fn test_get_out_of_bounds() {
        let buf = FrameBuffer::new(10, 10);
        assert_eq!(buf.get(10, 0), None);
        assert_eq!(buf.get(0, 10), None);
        assert_eq!(buf.get(100, 100), None);
    }

    #[test]
    fn test_put_char_ascii() {
        let mut buf = FrameBuffer::new(10, 10);
        buf.put_char(0, 0, 'a', &Style::default());

        let cell = buf.get(0, 0).unwrap();
        assert_eq!(cell.char, 'a');
        assert_eq!(cell.width, 1);
        assert!(!cell.is_continuation);
    }

    #[test]
    fn test_put_char_wide() {
        let mut buf = FrameBuffer::new(10, 10);
        buf.put_char(0, 0, '中', &Style::default());

        let cell0 = buf.get(0, 0).unwrap();
        assert_eq!(cell0.char, '中');
        assert_eq!(cell0.width, 2);
        assert!(!cell0.is_continuation);

        let cell1 = buf.get(1, 0).unwrap();
        assert!(cell1.is_continuation);
    }

    #[test]
    fn test_write_str() {
        let mut buf = FrameBuffer::new(20, 10);
        let written = buf.write_str(0, 0, "Hello", &Style::default());

        assert_eq!(written, 5);
        assert_eq!(buf.get(0, 0).unwrap().char, 'H');
        assert_eq!(buf.get(4, 0).unwrap().char, 'o');
    }

    #[test]
    fn test_write_str_with_wide() {
        let mut buf = FrameBuffer::new(20, 10);
        let written = buf.write_str(0, 0, "Hi中文", &Style::default());

        // H(1) + i(1) + 中(2) + 文(2) = 6 columns
        assert_eq!(written, 6);
        assert_eq!(buf.get(0, 0).unwrap().char, 'H');
        assert_eq!(buf.get(1, 0).unwrap().char, 'i');
        assert_eq!(buf.get(2, 0).unwrap().char, '中');
        assert!(buf.get(3, 0).unwrap().is_continuation);
        assert_eq!(buf.get(4, 0).unwrap().char, '文');
        assert!(buf.get(5, 0).unwrap().is_continuation);
    }

    #[test]
    fn test_fill_rect() {
        let mut buf = FrameBuffer::new(10, 10);
        let cell = Cell::from_char('#');
        buf.fill_rect(2, 2, 3, 3, &cell);

        assert_eq!(buf.get(2, 2), Some(&cell));
        assert_eq!(buf.get(4, 4), Some(&cell));
        assert_eq!(buf.get(1, 1), Some(&Cell::empty()));
        assert_eq!(buf.get(5, 5), Some(&Cell::empty()));
    }

    #[test]
    fn test_clear() {
        let mut buf = FrameBuffer::new(10, 10);
        buf.set(5, 5, Cell::from_char('x'));
        buf.clear();

        assert!(buf.get(5, 5).unwrap().is_empty());
    }

    #[test]
    fn test_clear_rect() {
        let mut buf = FrameBuffer::new(10, 10);
        buf.fill_rect(0, 0, 10, 10, &Cell::from_char('#'));
        buf.clear_rect(2, 2, 3, 3);

        assert_eq!(buf.get(0, 0).unwrap().char, '#');
        assert!(buf.get(3, 3).unwrap().is_empty());
    }

    #[test]
    fn test_row() {
        let mut buf = FrameBuffer::new(5, 3);
        buf.write_str(0, 1, "Hello", &Style::default());

        let row = buf.row(1).unwrap();
        assert_eq!(row.len(), 5);
        assert_eq!(row[0].char, 'H');
        assert_eq!(row[4].char, 'o');

        assert!(buf.row(10).is_none());
    }

    #[test]
    fn test_resize_grow() {
        let mut buf = FrameBuffer::new(5, 5);
        buf.set(2, 2, Cell::from_char('x'));
        buf.resize(10, 10);

        assert_eq!(buf.width(), 10);
        assert_eq!(buf.height(), 10);
        assert_eq!(buf.get(2, 2).unwrap().char, 'x');
        assert!(buf.get(9, 9).unwrap().is_empty());
    }

    #[test]
    fn test_resize_shrink() {
        let mut buf = FrameBuffer::new(10, 10);
        buf.set(2, 2, Cell::from_char('x'));
        buf.set(8, 8, Cell::from_char('y'));
        buf.resize(5, 5);

        assert_eq!(buf.width(), 5);
        assert_eq!(buf.height(), 5);
        assert_eq!(buf.get(2, 2).unwrap().char, 'x');
        assert!(buf.get(8, 8).is_none()); // Out of bounds now
    }

    #[test]
    fn test_copy_from() {
        let mut buf1 = FrameBuffer::new(10, 10);
        let mut buf2 = FrameBuffer::new(10, 10);

        buf1.set(5, 5, Cell::from_char('x'));
        buf2.copy_from(&buf1);

        assert_eq!(buf2.get(5, 5).unwrap().char, 'x');
    }

    #[test]
    fn test_swap_with() {
        let mut buf1 = FrameBuffer::new(10, 10);
        let mut buf2 = FrameBuffer::new(10, 10);

        buf1.set(0, 0, Cell::from_char('a'));
        buf2.set(0, 0, Cell::from_char('b'));

        buf1.swap_with(&mut buf2);

        assert_eq!(buf1.get(0, 0).unwrap().char, 'b');
        assert_eq!(buf2.get(0, 0).unwrap().char, 'a');
    }

    #[test]
    fn test_default() {
        let buf = FrameBuffer::default();
        assert_eq!(buf.width(), 80);
        assert_eq!(buf.height(), 24);
    }
}
