#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Surface codec subsystem — server-internal contract for registering and
//! looking up surface codecs by `kind` value.
//!
//! The registry pairs the uapi mechanism ([`Codec`] trait in
//! `reovim-surface-codec`) with a lifecycle owned by codec modules: each
//! `ext/surface-codec/<variant>/` module registers its `Arc<dyn Codec>` on
//! load and unregisters it on unload.
//!
//! This is the surface-concern counterpart of the input-concern
//! [`reovim_subsys_input::InputCodecRegistry`] and the content-concern
//! [`reovim_subsys_content_codec::ContentCodecRegistry`]. The three
//! registries stay decoupled — codecs for different concerns never share a
//! registry.

// Re-export the uapi so callers only need a single crate for trait + registry.
pub use reovim_surface_codec::{
    Codec, KIND_CELL_GRID, KIND_PIXEL_BUFFER, SurfaceDescriptor, SurfacePayloadError,
    surface_body, surface_kind,
};

mod registry;

#[cfg(test)]
mod registry_tests;

pub use registry::{DefaultSurfaceCodecRegistry, SurfaceCodecRegistry};
