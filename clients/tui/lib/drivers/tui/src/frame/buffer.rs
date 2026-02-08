//! Frame buffer for 2D cell grid storage.

use crate::style::Style;

use super::cell::{Cell, char_width};

/// 2D cell grid with row-major storage.
#[derive(Debug, Clone)]
pub struct FrameBuffer {
    cells: Vec<Cell>,
    width: u16,
    height: u16,
}

impl FrameBuffer {
    /// Create a new frame buffer with the given dimensions.
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
    pub fn resize(&mut self, width: u16, height: u16) {
        if width == self.width && height == self.height {
            return;
        }

        let new_size = usize::from(width) * usize::from(height);
        let mut new_cells = vec![Cell::empty(); new_size];

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

    /// Get the buffer width.
    #[must_use]
    pub const fn width(&self) -> u16 {
        self.width
    }

    /// Get the buffer height.
    #[must_use]
    pub const fn height(&self) -> u16 {
        self.height
    }

    /// Get a reference to a cell at (x, y).
    #[must_use]
    pub fn get(&self, x: u16, y: u16) -> Option<&Cell> {
        if x < self.width && y < self.height {
            let idx = self.index(x, y);
            self.cells.get(idx)
        } else {
            None
        }
    }

    /// Get a mutable reference to a cell at (x, y).
    #[must_use]
    pub fn get_mut(&mut self, x: u16, y: u16) -> Option<&mut Cell> {
        if x < self.width && y < self.height {
            let idx = self.index(x, y);
            self.cells.get_mut(idx)
        } else {
            None
        }
    }

    /// Set a cell at (x, y).
    pub fn set(&mut self, x: u16, y: u16, cell: Cell) {
        if x < self.width && y < self.height {
            let idx = self.index(x, y);
            self.cells[idx] = cell;
        }
    }

    /// Put a character at (x, y) with the given style.
    pub fn put_char(&mut self, x: u16, y: u16, char: char, style: &Style) {
        let width = char_width(char);
        let cell = Cell::new(char, style.clone());
        self.set(x, y, cell);

        if width == 2 && x + 1 < self.width {
            self.set(x + 1, y, Cell::continuation());
        }
    }

    /// Apply style to existing cell without replacing character.
    ///
    /// Used for cursor overlays where we want to show both the character
    /// and a cursor indicator (via background color). Only colors are merged;
    /// attributes are preserved from the original cell.
    pub fn apply_style(&mut self, x: u16, y: u16, style: &Style) {
        if let Some(cell) = self.get_mut(x, y) {
            if let Some(bg) = style.bg {
                cell.style.bg = Some(bg);
            }
            if let Some(fg) = style.fg {
                cell.style.fg = Some(fg);
            }
            // Note: attrs not merged since Attributes lacks union method
            // and cursor overlay primarily needs background color
        }
    }

    /// Write a string starting at (x, y).
    pub fn write_str(&mut self, x: u16, y: u16, s: &str, style: &Style) -> u16 {
        let mut col = x;
        for char in s.chars() {
            if col >= self.width {
                break;
            }

            let width = char_width(char);

            if width == 2 && col + 1 >= self.width {
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

    /// Clear the entire buffer.
    pub fn clear(&mut self) {
        self.cells.fill(Cell::empty());
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
    /// # Panics
    ///
    /// Panics if the buffers have different dimensions.
    pub fn swap_with(&mut self, other: &mut Self) {
        assert_eq!(self.width, other.width, "Buffer widths must match");
        assert_eq!(self.height, other.height, "Buffer heights must match");
        std::mem::swap(&mut self.cells, &mut other.cells);
    }

    #[inline]
    fn index(&self, x: u16, y: u16) -> usize {
        usize::from(y) * usize::from(self.width) + usize::from(x)
    }
}

impl Default for FrameBuffer {
    fn default() -> Self {
        Self::new(80, 24)
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
    fn test_put_char() {
        let mut buf = FrameBuffer::new(10, 10);
        buf.put_char(0, 0, 'a', &Style::default());
        assert_eq!(buf.get(0, 0).unwrap().char, 'a');
    }

    #[test]
    fn test_write_str() {
        let mut buf = FrameBuffer::new(20, 10);
        buf.write_str(0, 0, "Hello", &Style::default());
        assert_eq!(buf.get(0, 0).unwrap().char, 'H');
        assert_eq!(buf.get(4, 0).unwrap().char, 'o');
    }

    #[test]
    fn test_clear() {
        let mut buf = FrameBuffer::new(10, 10);
        buf.set(5, 5, Cell::from_char('x'));
        buf.clear();
        assert!(buf.get(5, 5).unwrap().is_empty());
    }

    #[test]
    fn test_resize_same_size() {
        let mut buf = FrameBuffer::new(10, 10);
        buf.put_char(5, 5, 'X', &Style::default());
        buf.resize(10, 10);
        // Should preserve content
        assert_eq!(buf.get(5, 5).unwrap().char, 'X');
    }

    #[test]
    fn test_resize_larger() {
        let mut buf = FrameBuffer::new(10, 10);
        buf.put_char(5, 5, 'Y', &Style::default());
        buf.resize(20, 20);
        assert_eq!(buf.width(), 20);
        assert_eq!(buf.height(), 20);
        assert_eq!(buf.get(5, 5).unwrap().char, 'Y');
    }

    #[test]
    fn test_resize_smaller() {
        let mut buf = FrameBuffer::new(20, 20);
        buf.put_char(5, 5, 'Z', &Style::default());
        buf.resize(10, 10);
        assert_eq!(buf.width(), 10);
        assert_eq!(buf.height(), 10);
        assert_eq!(buf.get(5, 5).unwrap().char, 'Z');
    }

    #[test]
    fn test_get_out_of_bounds() {
        let buf = FrameBuffer::new(10, 10);
        assert!(buf.get(100, 100).is_none());
        assert!(buf.get(10, 5).is_none());
        assert!(buf.get(5, 10).is_none());
    }

    #[test]
    fn test_get_mut_out_of_bounds() {
        let mut buf = FrameBuffer::new(10, 10);
        assert!(buf.get_mut(100, 100).is_none());
    }

    #[test]
    fn test_set_out_of_bounds() {
        let mut buf = FrameBuffer::new(10, 10);
        // Should not crash
        buf.set(100, 100, Cell::from_char('A'));
    }

    #[test]
    fn test_put_char_wide() {
        let mut buf = FrameBuffer::new(10, 10);
        buf.put_char(5, 5, '中', &Style::default());
        assert_eq!(buf.get(5, 5).unwrap().char, '中');
        assert_eq!(buf.get(5, 5).unwrap().width, 2);
        // Second column should be continuation
        assert!(buf.get(6, 5).unwrap().is_continuation);
    }

    #[test]
    fn test_put_char_wide_at_edge() {
        let mut buf = FrameBuffer::new(10, 10);
        // Wide char at last column should not crash
        buf.put_char(9, 5, '中', &Style::default());
        assert_eq!(buf.get(9, 5).unwrap().char, '中');
    }

    #[test]
    fn test_apply_style_fg() {
        let mut buf = FrameBuffer::new(10, 10);
        buf.put_char(3, 3, 'C', &Style::default());
        let style = Style::new().with_fg(reovim_arch::Color::Green);
        buf.apply_style(3, 3, &style);

        let cell = buf.get(3, 3).unwrap();
        assert_eq!(cell.char, 'C');
        assert_eq!(cell.style.fg, Some(reovim_arch::Color::Green));
    }

    #[test]
    fn test_apply_style_bg() {
        let mut buf = FrameBuffer::new(10, 10);
        buf.put_char(2, 2, 'D', &Style::default());
        let style = Style::new().with_bg(reovim_arch::Color::Yellow);
        buf.apply_style(2, 2, &style);

        let cell = buf.get(2, 2).unwrap();
        assert_eq!(cell.style.bg, Some(reovim_arch::Color::Yellow));
    }

    #[test]
    fn test_apply_style_out_of_bounds() {
        let mut buf = FrameBuffer::new(10, 10);
        let style = Style::new().with_bg(reovim_arch::Color::Red);
        // Should not crash
        buf.apply_style(100, 100, &style);
    }

    #[test]
    fn test_write_str_returns_width() {
        let mut buf = FrameBuffer::new(20, 10);
        let width = buf.write_str(0, 0, "Hello", &Style::default());
        assert_eq!(width, 5);
    }

    #[test]
    fn test_write_str_overflow() {
        let mut buf = FrameBuffer::new(5, 10);
        buf.write_str(0, 0, "HelloWorld", &Style::default());
        // Should only write "Hello"
        assert_eq!(buf.get(0, 0).unwrap().char, 'H');
        assert_eq!(buf.get(4, 0).unwrap().char, 'o');
    }

    #[test]
    fn test_write_str_wide_chars() {
        let mut buf = FrameBuffer::new(20, 10);
        buf.write_str(0, 0, "中文", &Style::default());
        assert_eq!(buf.get(0, 0).unwrap().char, '中');
        assert!(buf.get(1, 0).unwrap().is_continuation);
        assert_eq!(buf.get(2, 0).unwrap().char, '文');
    }

    #[test]
    fn test_write_str_wide_char_at_boundary() {
        let mut buf = FrameBuffer::new(5, 10);
        // Write wide char that would overflow
        buf.write_str(4, 0, "中", &Style::default());
        // Should replace with space
        assert_eq!(buf.get(4, 0).unwrap().char, ' ');
    }

    #[test]
    fn test_fill_rect() {
        let mut buf = FrameBuffer::new(20, 20);
        let cell = Cell::new('*', Style::default());
        buf.fill_rect(5, 5, 3, 2, &cell);

        assert_eq!(buf.get(5, 5).unwrap().char, '*');
        assert_eq!(buf.get(7, 6).unwrap().char, '*');
        assert_eq!(buf.get(8, 5).unwrap().char, ' '); // Outside rect
    }

    #[test]
    fn test_fill_rect_overflow() {
        let mut buf = FrameBuffer::new(10, 10);
        let cell = Cell::new('#', Style::default());
        // Should not crash, just clip
        buf.fill_rect(8, 8, 10, 10, &cell);
        assert_eq!(buf.get(8, 8).unwrap().char, '#');
    }

    #[test]
    fn test_row_out_of_bounds() {
        let buf = FrameBuffer::new(10, 10);
        assert!(buf.row(100).is_none());
        assert!(buf.row(10).is_none());
    }

    #[test]
    fn test_row_valid() {
        let mut buf = FrameBuffer::new(10, 10);
        buf.put_char(5, 3, 'R', &Style::default());
        let row = buf.row(3).unwrap();
        assert_eq!(row.len(), 10);
        assert_eq!(row[5].char, 'R');
    }

    #[test]
    fn test_copy_from_same_size() {
        let mut buf1 = FrameBuffer::new(10, 10);
        let mut buf2 = FrameBuffer::new(10, 10);
        buf1.put_char(5, 5, 'S', &Style::default());
        buf2.copy_from(&buf1);
        assert_eq!(buf2.get(5, 5).unwrap().char, 'S');
    }

    #[test]
    fn test_copy_from_smaller() {
        let mut buf1 = FrameBuffer::new(5, 5);
        let mut buf2 = FrameBuffer::new(10, 10);
        buf1.put_char(3, 3, 'T', &Style::default());
        buf2.copy_from(&buf1);
        assert_eq!(buf2.get(3, 3).unwrap().char, 'T');
    }

    #[test]
    fn test_copy_from_larger() {
        let mut buf1 = FrameBuffer::new(20, 20);
        let mut buf2 = FrameBuffer::new(10, 10);
        buf1.put_char(5, 5, 'U', &Style::default());
        buf1.put_char(15, 15, 'V', &Style::default());
        buf2.copy_from(&buf1);
        assert_eq!(buf2.get(5, 5).unwrap().char, 'U');
        // (15, 15) should not be copied (out of bounds)
        assert!(buf2.get(15, 15).is_none());
    }

    #[test]
    fn test_swap_with() {
        let mut buf1 = FrameBuffer::new(10, 10);
        let mut buf2 = FrameBuffer::new(10, 10);
        buf1.put_char(1, 1, 'A', &Style::default());
        buf2.put_char(2, 2, 'B', &Style::default());

        buf1.swap_with(&mut buf2);

        assert_eq!(buf1.get(2, 2).unwrap().char, 'B');
        assert_eq!(buf2.get(1, 1).unwrap().char, 'A');
    }

    #[test]
    #[should_panic(expected = "Buffer widths must match")]
    fn test_swap_with_different_width() {
        let mut buf1 = FrameBuffer::new(10, 10);
        let mut buf2 = FrameBuffer::new(20, 10);
        buf1.swap_with(&mut buf2);
    }

    #[test]
    #[should_panic(expected = "Buffer heights must match")]
    fn test_swap_with_different_height() {
        let mut buf1 = FrameBuffer::new(10, 10);
        let mut buf2 = FrameBuffer::new(10, 20);
        buf1.swap_with(&mut buf2);
    }

    #[test]
    fn test_default() {
        let buf = FrameBuffer::default();
        assert_eq!(buf.width(), 80);
        assert_eq!(buf.height(), 24);
    }

    #[test]
    fn test_clone() {
        let mut buf = FrameBuffer::new(10, 10);
        buf.put_char(5, 5, 'W', &Style::default());
        let cloned = buf.clone();
        assert_eq!(cloned.get(5, 5).unwrap().char, 'W');
        assert_eq!(cloned.width(), buf.width());
        assert_eq!(cloned.height(), buf.height());
    }

    #[test]
    fn test_debug() {
        let buf = FrameBuffer::new(10, 10);
        let debug = format!("{buf:?}");
        assert!(debug.contains("FrameBuffer"));
    }
}
