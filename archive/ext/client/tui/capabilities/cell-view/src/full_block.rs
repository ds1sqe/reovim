//! [`FullBlockRasterizer`] — 1 logical cell → 1 terminal cell.
//!
//! The baseline rasterizer. Output is byte-for-byte identical to the
//! TUI shell's current (pre-17-γ) render path: each logical cell
//! becomes one terminal cell with the same character and style.

use reovim_ext_client_tui_cap_cell::CellCapability;

use crate::{
    raster::{RasterCell, RasterOutput},
    rasterizer::{ViewHint, ViewRasterizer},
};

/// Rasterizer for [`ViewHint::FullBlock`].
///
/// Stateless. One instance per client is enough.
#[derive(Debug, Default, Clone, Copy)]
pub struct FullBlockRasterizer;

impl FullBlockRasterizer {
    /// Construct a new rasterizer.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl ViewRasterizer for FullBlockRasterizer {
    fn rasterize(&self, grid: &CellCapability, out: &mut dyn RasterOutput) {
        let (w_out, h_out) = out.size();
        let w = grid.width().min(w_out);
        let h = grid.height().min(h_out);
        for y in 0..h {
            for x in 0..w {
                if let Some(cell) = grid.get_cell(x, y) {
                    out.set_cell(RasterCell {
                        x,
                        y,
                        ch: cell.ch,
                        style: cell.style,
                    });
                }
            }
        }
    }

    fn hint(&self) -> ViewHint {
        ViewHint::FullBlock
    }
}
