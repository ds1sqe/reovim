#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Client-side codec subsystem.
//!
//! Mirrors the server-side input subsys three-tier pattern
//! (`uapi envelope → subsys registry contract → ext variants`)
//! but lives on the client because render decoding and surface
//! encoding both operate on the native display owned by the client.
//!
//! Provided types:
//!
//! - [`FrameTarget`]: typeid-keyed bag of capability slots that render
//!   handlers populate and surface encoders read. Core is shape-blind;
//!   concrete capability types (cell grids, pixel buffers, DOM trees,
//!   mesh scenes, …) live in ext crates.
//! - [`RenderHandler`] + [`RenderHandlerRegistry`] +
//!   [`DefaultRenderHandlerRegistry`]: contract and default impl for
//!   decoding render-payload byte slices into capability slots on a
//!   `FrameTarget`.
//! - [`SurfaceEncoder`] + [`SurfaceEncoderRegistry`] +
//!   [`DefaultSurfaceEncoderRegistry`]: parallel contract for the
//!   reverse direction (capability slot → bytes).
//!
//! The crate has **no dependency on `reovim-kernel`**: kernel-side
//! `Service` markers are a server-internal contract and must not
//! contaminate client core. Registries are stored in
//! `ClientServiceRegistry` by consumers using the existing
//! `Send + Sync + 'static` bound; no marker trait is required.

pub mod render;
pub mod surface;
pub mod surface_descriptor;
pub mod target;

#[cfg(test)]
mod render_tests;
#[cfg(test)]
mod surface_descriptor_tests;
#[cfg(test)]
mod surface_tests;
#[cfg(test)]
mod target_tests;

pub use {
    render::{
        DefaultRenderHandlerRegistry, RenderHandler, RenderHandlerError, RenderHandlerRegistry,
    },
    surface::{
        DefaultSurfaceEncoderRegistry, SurfaceEncoder, SurfaceEncoderError, SurfaceEncoderRegistry,
    },
    surface_descriptor::{
        DefaultSurfaceDescriptorHandlerRegistry, SurfaceApplyContext, SurfaceDescriptorApplyError,
        SurfaceDescriptorHandler, SurfaceDescriptorHandlerError, SurfaceDescriptorHandlerRegistry,
    },
    target::FrameTarget,
};
