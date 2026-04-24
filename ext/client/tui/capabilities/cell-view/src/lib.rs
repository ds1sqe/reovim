#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! TUI cell-grid view rasterization.
//!
//! After chrome modules have drawn into a [`CellCapability`], the TUI
//! shell *rasterizes* the logical grid into terminal cells for the
//! backend (`crossterm`). Three modes:
//!
//! - [`ViewHint::FullBlock`]: 1 logical cell → 1 terminal cell,
//!   byte-identical to the baseline render.
//! - [`ViewHint::HalfBlock`] (live since Flight 74): each terminal
//!   cell renders two stacked logical cells via `▀` / `▄` / space with
//!   fg=top-half, bg=bottom-half — effective vertical resolution ×2.
//! - [`ViewHint::Braille`]: each terminal cell renders a 2×4 logical
//!   sub-grid via `⠀…⣿` — effective resolution ×8. Rasterizer lands
//!   in Flight 75.
//!
//! ## Trait layout
//!
//! The seam separates *input* (`ChromeSurface`, where chrome modules
//! write) from *output* ([`RasterOutput`], where the rasterizer lands
//! terminal-ready cells). [`ChromeSurface`] is **not** the output type
//! — conflating input and output makes `HalfBlock`'s
//! `set_cell(x, y, '▀', Style { fg: top, bg: bottom, .. })` semantics
//! impossible to express without trait gymnastics.

pub mod backend_output;
pub mod full_block;
pub mod half_block;
pub mod raster;
pub mod rasterizer;

pub use {
    backend_output::{BackendRasterOutput, convert_cell_style_to_display_style},
    full_block::FullBlockRasterizer,
    half_block::HalfBlockRasterizer,
    raster::{RasterCell, RasterOutput},
    rasterizer::{ViewHint, ViewRasterizer},
};

#[cfg(test)]
mod backend_output_tests;
#[cfg(test)]
mod full_block_tests;
#[cfg(test)]
mod half_block_tests;
#[cfg(test)]
mod lib_tests;
#[cfg(test)]
mod raster_tests;
#[cfg(test)]
mod rasterizer_tests;
