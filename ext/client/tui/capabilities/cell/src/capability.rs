//! `CellCapability`: a bounded 2D grid of `Cell` values.

use crate::style::CellStyle;

/// A single displayable cell: one character plus its style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// Displayed character. Whitespace cells use `' '`.
    pub ch: char,
    /// Foreground/background color + attribute flags.
    pub style: CellStyle,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            style: CellStyle::default(),
        }
    }
}

impl Cell {
    /// Create a new cell with the given character and style.
    #[must_use]
    pub const fn new(ch: char, style: CellStyle) -> Self {
        Self { ch, style }
    }
}

/// Error returned by [`CellCapability::write_cell`] when the target
/// coordinates are outside the grid.
///
/// Render handlers propagate this upward as
/// `RenderHandlerError::InvalidData { reason }`. A typed error (rather
/// than a silent no-op) prevents handlers from writing into the void
/// without surfacing a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteCellError {
    /// X-coordinate the caller attempted to write to.
    pub x: u16,
    /// Y-coordinate the caller attempted to write to.
    pub y: u16,
    /// Width of the grid at the time of the attempt.
    pub width: u16,
    /// Height of the grid at the time of the attempt.
    pub height: u16,
}

impl std::fmt::Display for WriteCellError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "cell coordinates ({}, {}) out of bounds for grid {}x{}",
            self.x, self.y, self.width, self.height
        )
    }
}

impl std::error::Error for WriteCellError {}

/// A bounded 2D grid of [`Cell`]s.
///
/// Cells are stored row-major (row 0 first). `new(w, h)` allocates
/// `w * h` default cells. Out-of-bounds writes surface a
/// [`WriteCellError`] instead of panicking or silently no-oping.
#[derive(Debug, Clone)]
pub struct CellCapability {
    width: u16,
    height: u16,
    cells: Vec<Cell>,
}

impl CellCapability {
    /// Create a `width` × `height` grid of default cells.
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        let len = usize::from(width) * usize::from(height);
        Self {
            width,
            height,
            cells: vec![Cell::default(); len],
        }
    }

    /// Grid width (columns).
    #[must_use]
    pub const fn width(&self) -> u16 {
        self.width
    }

    /// Grid height (rows).
    #[must_use]
    pub const fn height(&self) -> u16 {
        self.height
    }

    /// Total number of cells (`width * height`).
    #[must_use]
    pub const fn len(&self) -> usize {
        self.cells.len()
    }

    /// Whether the grid is zero-sized.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Map (x, y) to a flat index if in bounds.
    fn index(&self, x: u16, y: u16) -> Option<usize> {
        if x >= self.width || y >= self.height {
            None
        } else {
            Some(usize::from(y) * usize::from(self.width) + usize::from(x))
        }
    }

    /// Write a cell at `(x, y)`.
    ///
    /// # Errors
    ///
    /// Returns [`WriteCellError`] when the coordinates are outside the
    /// grid. Callers must not silently drop OOB writes: render handlers
    /// propagate this error upward as
    /// `RenderHandlerError::InvalidData`.
    pub fn write_cell(&mut self, x: u16, y: u16, cell: Cell) -> Result<(), WriteCellError> {
        let Some(idx) = self.index(x, y) else {
            return Err(WriteCellError {
                x,
                y,
                width: self.width,
                height: self.height,
            });
        };
        self.cells[idx] = cell;
        Ok(())
    }

    /// Read-only access to a cell at `(x, y)`, or `None` if out of
    /// bounds.
    #[must_use]
    pub fn get_cell(&self, x: u16, y: u16) -> Option<&Cell> {
        self.index(x, y).map(|idx| &self.cells[idx])
    }

    /// Overwrite every cell in the grid with the same character and
    /// style.
    pub fn fill(&mut self, ch: char, style: CellStyle) {
        let filler = Cell::new(ch, style);
        for cell in &mut self.cells {
            *cell = filler.clone();
        }
    }

    /// Reset every cell to [`Cell::default`] (space + plain style).
    pub fn clear(&mut self) {
        self.fill(' ', CellStyle::default());
    }

    /// Resize the grid to `(new_width, new_height)` default cells.
    ///
    /// All prior content is dropped. Callers that want to preserve
    /// content must read cells before calling, then re-write them.
    pub fn resize(&mut self, new_width: u16, new_height: u16) {
        let len = usize::from(new_width) * usize::from(new_height);
        self.width = new_width;
        self.height = new_height;
        self.cells.clear();
        self.cells.resize(len, Cell::default());
    }

    /// Iterate over `((x, y), &Cell)` in row-major order.
    pub fn iter(&self) -> impl Iterator<Item = ((u16, u16), &Cell)> {
        let w = self.width;
        self.cells.iter().enumerate().map(move |(i, c)| {
            // i = y * w + x; safe to cast because i < w * h fits in usize
            let x_usize = i % usize::from(w);
            let y_usize = i / usize::from(w);
            #[allow(clippy::cast_possible_truncation)]
            let coord = (x_usize as u16, y_usize as u16);
            (coord, c)
        })
    }
}
