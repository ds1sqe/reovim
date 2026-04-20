#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! TUI surface codec module — registers [`CellGridCodec`] for
//! [`KIND_CELL_GRID`] (0x0001).
//!
//! This is the cell-grid variant of the three-tier surface codec pattern:
//!
//! ```text
//! uapi/surface-codec/               ← mechanism (Codec trait, kinds)
//! server/lib/subsys/surface-codec/  ← registry
//! ext/surface-codec/tui/            ← THIS CRATE (cell-grid surface)
//! ```

pub mod codec;
pub mod module;

pub use {
    codec::{CellGridCodec, CellGridSurface},
    module::TuiSurfaceCodecModule,
};

pub use reovim_surface_codec::KIND_CELL_GRID;

// Generate FFI entry points for dynamic loading.
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(TuiSurfaceCodecModule);
