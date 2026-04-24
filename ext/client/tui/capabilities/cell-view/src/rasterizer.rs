//! [`ViewHint`] enum + [`ViewRasterizer`] trait.
//!
//! A rasterizer reads a [`CellCapability`] and writes terminal-ready
//! cells to a [`RasterOutput`]. Each rasterizer declares a
//! [`ViewHint`] so callers can pair a per-window hint with the right
//! implementation.

use reovim_ext_client_tui_cap_cell::CellCapability;

use crate::raster::RasterOutput;

/// Per-window rasterization mode.
///
/// - [`ViewHint::FullBlock`] — 1 logical cell → 1 terminal cell. Default.
/// - [`ViewHint::HalfBlock`] — 2 stacked logical cells → 1 terminal cell
///   via `▀` (live since Flight 74; see [`crate::HalfBlockRasterizer`]).
/// - [`ViewHint::Braille`] — 2×4 logical sub-grid → 1 terminal cell via
///   `⠀…⣿` (live since Flight 75; see [`crate::BrailleRasterizer`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewHint {
    /// 1 logical cell → 1 terminal cell. Default.
    #[default]
    FullBlock,
    /// 2 stacked logical cells → 1 terminal cell via `▀`.
    HalfBlock,
    /// 2×4 logical sub-grid → 1 terminal cell via `⠀…⣿`. Monochrome
    /// per glyph.
    Braille,
}

/// Project a [`CellCapability`] onto a [`RasterOutput`].
///
/// Implementations are stateless — `rasterize` should not require
/// `&mut self`. One instance per hint is enough for the whole client.
pub trait ViewRasterizer {
    /// Project `grid` onto `out`.
    fn rasterize(&self, grid: &CellCapability, out: &mut dyn RasterOutput);

    /// The [`ViewHint`] this rasterizer implements. Used by the TUI
    /// shell to pair a per-window hint with an implementation.
    fn hint(&self) -> ViewHint;
}
