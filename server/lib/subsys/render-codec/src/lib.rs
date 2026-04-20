#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Render codec subsystem — server-internal contract for registering and
//! looking up render codecs by `kind` value.
//!
//! The registry pairs the uapi mechanism ([`Codec`] trait in
//! `reovim-render-codec`) with a lifecycle owned by codec modules: each
//! `ext/render-codec/<variant>/` module registers its `Arc<dyn Codec>` on
//! load and unregisters it on unload.
//!
//! This is the render-concern counterpart of the surface-concern
//! [`reovim_subsys_surface_codec::SurfaceCodecRegistry`] and the input-
//! concern [`reovim_subsys_input::InputCodecRegistry`]. The registries
//! stay decoupled — codecs for different concerns never share a registry.

// Re-export the uapi so callers only need a single crate for trait + registry.
pub use reovim_render_codec::{
    Codec, KIND_CELL_GRID, RenderDescriptor, RenderPayloadError, render_body, render_kind,
};

mod registry;

#[cfg(test)]
mod registry_tests;

pub use registry::{DefaultRenderCodecRegistry, RenderCodecRegistry};
