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
    #[cfg_attr(coverage_nightly, coverage(off))]
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
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn get_mut(&mut self, x: u16, y: u16) -> Option<&mut Cell> {
        if x < self.width && y < self.height {
            let idx = self.index(x, y);
            self.cells.get_mut(idx)
        } else {
            None
        }
    }

    /// Set a cell at (x, y).
    #[cfg_attr(coverage_nightly, coverage(off))]
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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
#[path = "buffer_tests.rs"]
mod tests;
