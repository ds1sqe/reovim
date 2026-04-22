//! Terminal-backend output side of the rasterization seam.
//!
//! [`RasterOutput`] is the **output** abstraction a [`ViewRasterizer`](
//! crate::ViewRasterizer) writes into. It is deliberately narrow —
//! `set_cell` + `size` — and deliberately distinct from
//! [`reovim_ext_client_tui_cap_cell::CellCapability`] (the *input* grid
//! chrome modules write into).
//!
//! Keeping the two sides separate is what lets `HalfBlock` render two
//! stacked logical cells into one terminal cell with fg = top color,
//! bg = bottom color. That operation is a terminal-backend write, not
//! a chrome-module write; conflating the two through a single trait
//! would make the `HalfBlock` encoding inexpressible.

use reovim_ext_client_tui_cap_cell::CellStyle;

/// A single terminal-ready cell handed to a [`RasterOutput`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RasterCell {
    /// Column in the terminal buffer.
    pub x: u16,
    /// Row in the terminal buffer.
    pub y: u16,
    /// Glyph to emit at this position.
    pub ch: char,
    /// Style to apply (foreground, background, attributes).
    pub style: CellStyle,
}

/// Narrow terminal-backend-level output trait.
///
/// Implementations:
///
/// - Production: a thin adapter around the TUI backend's frame buffer
///   (added in Phase C).
/// - Tests: [`RecordingRasterOutput`] accumulates every
///   [`RasterCell`] for byte-equivalence assertions.
///
/// The trait is intentionally not `ChromeSurface`. See module docs.
pub trait RasterOutput {
    /// Emit a single terminal cell at `(x, y)`.
    ///
    /// Coordinates outside `size()` are silently dropped — the
    /// rasterizer's job is to stay in-bounds, not the backend's.
    fn set_cell(&mut self, cell: RasterCell);

    /// Terminal buffer size in cells `(width, height)`.
    fn size(&self) -> (u16, u16);
}

/// Test fixture: an in-memory [`RasterOutput`] that records every cell
/// written, preserving order. Used by both crate-internal tests and as
/// a vehicle for Phase C byte-equivalence integration tests.
#[derive(Debug, Clone)]
pub struct RecordingRasterOutput {
    width: u16,
    height: u16,
    cells: Vec<RasterCell>,
}

impl RecordingRasterOutput {
    /// Create a recorder sized to `width` × `height`.
    #[must_use]
    pub const fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            cells: Vec::new(),
        }
    }

    /// Every `set_cell` call in the order it was made.
    #[must_use]
    pub fn cells(&self) -> &[RasterCell] {
        &self.cells
    }
}

impl RasterOutput for RecordingRasterOutput {
    fn set_cell(&mut self, cell: RasterCell) {
        if cell.x < self.width && cell.y < self.height {
            self.cells.push(cell);
        }
    }

    fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }
}
