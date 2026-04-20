#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! TUI render codec module — registers [`CellGridRenderCodec`] for
//! [`KIND_CELL_GRID`] (0x0001).
//!
//! This is the cell-grid variant of the three-tier render codec pattern:
//!
//! ```text
//! uapi/render-codec/               ← mechanism (Codec trait, kinds)
//! server/lib/subsys/render-codec/  ← registry
//! ext/render-codec/tui/            ← THIS CRATE (cell-grid render target)
//! ```

pub mod codec;
pub mod module;

pub use {
    codec::{CellGridRender, CellGridRenderCodec},
    module::TuiRenderCodecModule,
};

pub use reovim_render_codec::KIND_CELL_GRID;

// Generate FFI entry points for dynamic loading.
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(TuiRenderCodecModule);
