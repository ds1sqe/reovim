#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Pixel surface codec module — registers [`PixelCodec`] for
//! [`KIND_PIXEL_BUFFER`] (0x0002).
//!
//! This is the pixel variant of the three-tier surface codec pattern:
//!
//! ```text
//! uapi/surface-codec/               ← mechanism (Codec trait, kinds)
//! server/lib/subsys/surface-codec/  ← registry
//! ext/surface-codec/pixel/          ← THIS CRATE (pixel raster surface)
//! ```

pub mod codec;
pub mod module;

pub use {
    codec::{PixelCodec, PixelSurface},
    module::PixelSurfaceCodecModule,
};

pub use reovim_surface_codec::KIND_PIXEL_BUFFER;

// Generate FFI entry points for dynamic loading.
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(PixelSurfaceCodecModule);
