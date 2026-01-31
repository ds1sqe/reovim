//! Frame buffer for storing screen state

use {super::Cell, crate::highlight::Style};

/// A 2D frame buffer representing the entire screen
#[derive(Clone)]
pub struct FrameBuffer {
    width: u16,
    height: u16,
    /// Row-major cell storage: cells[y * width + x]
    cells: Vec<Cell>,
}

impl FrameBuffer {
    /// Create a new frame buffer with the given dimensions
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        let size = usize::from(width) * usize::from(height);
        Self {
            width,
            height,
            cells: vec![Cell::default(); size],
        }
    }

    /// Resize the buffer to new dimensions
    /// Clears all content
    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        let size = usize::from(width) * usize::from(height);
        self.cells.resize(size, Cell::default());
        self.clear();
    }

    /// Clear all cells to empty
    pub fn clear(&mut self) {
        self.cells.fill(Cell::default());
    }

    /// Get the width of the buffer
    #[must_use]
    pub const fn width(&self) -> u16 {
        self.width
    }

    /// Get the height of the buffer
    #[must_use]
    pub const fn height(&self) -> u16 {
        self.height
    }

    /// Calculate the index for a given position
    #[inline]
    #[allow(clippy::missing_const_for_fn)] // index calculation uses casts
    fn index(&self, x: u16, y: u16) -> usize {
        (y as usize) * (self.width as usize) + (x as usize)
    }

    /// Get a cell at the given position
    #[must_use]
    pub fn get(&self, x: u16, y: u16) -> Option<&Cell> {
        if x < self.width && y < self.height {
            Some(&self.cells[self.index(x, y)])
        } else {
            None
        }
    }

    /// Get a mutable reference to a cell at the given position
    pub fn get_mut(&mut self, x: u16, y: u16) -> Option<&mut Cell> {
        if x < self.width && y < self.height {
            let idx = self.index(x, y);
            Some(&mut self.cells[idx])
        } else {
            None
        }
    }

    /// Set a cell at the given position
    pub fn set(&mut self, x: u16, y: u16, cell: Cell) {
        if x < self.width && y < self.height {
            let idx = self.index(x, y);
            self.cells[idx] = cell;
        }
    }

    /// Write a character at position with style
    pub fn put_char(&mut self, x: u16, y: u16, ch: char, style: &Style) {
        if x < self.width && y < self.height {
            let cell = Cell::new(ch, style.clone());
            let width = cell.width;
            let idx = self.index(x, y);
            self.cells[idx] = cell;

            // Write continuation cell for wide characters
            if width == 2 && x + 1 < self.width {
                let cont_idx = self.index(x + 1, y);
                self.cells[cont_idx] = Cell::continuation(style.clone());
            }
        }
    }

    /// Write a string at position, returning how many columns were written
    pub fn write_str(&mut self, x: u16, y: u16, s: &str, style: &Style) -> u16 {
        let mut col = x;
        for ch in s.chars() {
            if col >= self.width {
                break;
            }
            let cell = Cell::new(ch, style.clone());
            let width = cell.width;
            self.set(col, y, cell);

            // Write continuation cell for wide characters
            if width == 2 && col + 1 < self.width {
                self.set(col + 1, y, Cell::continuation(style.clone()));
            }

            col += u16::from(width);
        }
        col.saturating_sub(x)
    }

    /// Fill a rectangular region with a cell
    pub fn fill_rect(&mut self, x: u16, y: u16, width: u16, height: u16, cell: &Cell) {
        for row in y..y.saturating_add(height).min(self.height) {
            for col in x..x.saturating_add(width).min(self.width) {
                self.set(col, row, cell.clone());
            }
        }
    }

    /// Clear a rectangular region (fill with empty cells)
    pub fn clear_rect(&mut self, x: u16, y: u16, width: u16, height: u16) {
        self.fill_rect(x, y, width, height, &Cell::default());
    }

    /// Get a row slice
    #[must_use]
    pub fn row(&self, y: u16) -> Option<&[Cell]> {
        if y < self.height {
            let start = self.index(0, y);
            let end = start + usize::from(self.width);
            Some(&self.cells[start..end])
        } else {
            None
        }
    }

    /// Iterate over all cells with their positions
    pub fn cells_iter(&self) -> impl Iterator<Item = (u16, u16, &Cell)> {
        (0..self.height).flat_map(move |y| {
            (0..self.width).map(move |x| {
                let idx = usize::from(y) * usize::from(self.width) + usize::from(x);
                (x, y, &self.cells[idx])
            })
        })
    }

    /// Copy contents from another buffer (must be same size)
    pub fn copy_from(&mut self, other: &Self) {
        debug_assert_eq!(self.width, other.width);
        debug_assert_eq!(self.height, other.height);
        self.cells.clone_from(&other.cells);
    }

    /// Swap contents with another buffer
    #[allow(clippy::missing_const_for_fn)] // uses mem::swap
    pub fn swap_with(&mut self, other: &mut Self) {
        std::mem::swap(&mut self.cells, &mut other.cells);
        std::mem::swap(&mut self.width, &mut other.width);
        std::mem::swap(&mut self.height, &mut other.height);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_new() {
        let buf = FrameBuffer::new(80, 24);
        assert_eq!(buf.width(), 80);
        assert_eq!(buf.height(), 24);
    }

    #[test]
    fn test_buffer_get_set() {
        let mut buf = FrameBuffer::new(10, 5);
        buf.set(5, 2, Cell::from_char('X'));

        assert_eq!(buf.get(5, 2).map(|c| c.char), Some('X'));
        assert_eq!(buf.get(0, 0).map(|c| c.char), Some(' '));
        assert!(buf.get(100, 100).is_none());
    }

    #[test]
    fn test_buffer_write_str() {
        let mut buf = FrameBuffer::new(20, 5);
        let written = buf.write_str(0, 0, "Hello", &Style::default());
        assert_eq!(written, 5);
        assert_eq!(buf.get(0, 0).map(|c| c.char), Some('H'));
        assert_eq!(buf.get(4, 0).map(|c| c.char), Some('o'));
    }

    #[test]
    fn test_buffer_resize() {
        let mut buf = FrameBuffer::new(10, 5);
        buf.set(5, 2, Cell::from_char('X'));
        buf.resize(20, 10);

        assert_eq!(buf.width(), 20);
        assert_eq!(buf.height(), 10);
        // Content should be cleared after resize
        assert_eq!(buf.get(5, 2).map(|c| c.char), Some(' '));
    }
}
