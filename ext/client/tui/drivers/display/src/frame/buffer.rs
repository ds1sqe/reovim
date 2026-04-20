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
                // x < copy_width <= self.width and y < copy_height <= self.height,
                // so direct index is always valid.
                let old_idx = self.index(x, y);
                let new_idx = usize::from(y) * usize::from(width) + usize::from(x);
                new_cells[new_idx] = self.cells[old_idx].clone();
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
                // x < copy_width <= other.width and y < copy_height <= other.height,
                // so direct index is always valid.
                let src_idx = usize::from(y) * usize::from(other.width) + usize::from(x);
                let dst_idx = self.index(x, y);
                self.cells[dst_idx] = other.cells[src_idx].clone();
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
#[path = "buffer_tests.rs"]
mod tests;
