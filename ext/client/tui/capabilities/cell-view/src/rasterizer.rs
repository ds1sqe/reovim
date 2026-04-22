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
/// - [`ViewHint::FullBlock`] — 1 logical cell → 1 terminal cell. This
///   is the default and matches today's TUI output byte-for-byte.
/// - [`ViewHint::HalfBlock`] — *declared but unreachable in 25.1.*
///   A rasterizer lands in Plan 26 (17-γ.2).
/// - [`ViewHint::Braille`] — *declared but unreachable in 25.1.*
///   A rasterizer lands in Plan 27 (17-γ.3).
///
/// The two future variants are defined here rather than added later so
/// downstream match expressions written in 25.1 (e.g. the hint→
/// rasterizer selector in the TUI shell) can be made exhaustive now;
/// unreachable arms document the design invariant explicitly at their
/// call sites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewHint {
    /// 1 logical cell → 1 terminal cell. Default.
    #[default]
    FullBlock,
    /// 2 stacked logical cells → 1 terminal cell via `▀`.
    /// Rasterizer lands in 17-γ.2.
    HalfBlock,
    /// 2×4 logical sub-grid → 1 terminal cell via `⠀…⣿`.
    /// Rasterizer lands in 17-γ.3.
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
