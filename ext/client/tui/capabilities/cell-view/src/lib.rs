#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! TUI cell-grid view rasterization.
//!
//! After chrome modules have drawn into a [`CellCapability`], the TUI
//! shell must *rasterize* the logical grid into terminal cells for the
//! backend (`crossterm`). Today this is a 1-to-1 pass-through; 17-γ
//! adds two alternative rasterizations:
//!
//! - [`ViewHint::HalfBlock`] (25.2): each terminal cell renders two
//!   stacked logical cells via `▀` with fg=top, bg=bottom — effective
//!   vertical resolution ×2.
//! - [`ViewHint::Braille`] (25.3): each terminal cell renders a 2×4
//!   logical sub-grid via `⠀…⣿` — effective resolution ×8.
//!
//! 25.1 ships only the foundation: the hint enum, the rasterizer
//! trait, a narrow terminal-output trait, and the baseline
//! `FullBlock` rasterizer (byte-identical to today's render).
//!
//! ## Trait layout
//!
//! The seam separates *input* (`ChromeSurface`, where chrome modules
//! write) from *output* ([`RasterOutput`], where the rasterizer lands
//! terminal-ready cells). [`ChromeSurface`] is **not** the output type
//! — conflating input and output makes `HalfBlock`'s
//! `set_cell(x, y, '▀', Style { fg: top, bg: bottom, .. })` semantics
//! impossible to express without trait gymnastics.

pub mod full_block;
pub mod raster;
pub mod rasterizer;

pub use {
    full_block::FullBlockRasterizer,
    raster::{RasterCell, RasterOutput},
    rasterizer::{ViewHint, ViewRasterizer},
};

#[cfg(test)]
mod full_block_tests;
#[cfg(test)]
mod lib_tests;
#[cfg(test)]
mod raster_tests;
#[cfg(test)]
mod rasterizer_tests;
