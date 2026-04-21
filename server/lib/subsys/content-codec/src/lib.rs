#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Content codec subsystem — server-internal contract for registering and
//! looking up content codecs by [`ContentType`].
//!
//! The registry pairs the uapi mechanism ([`ContentCodec`] trait in
//! `reovim-content-codec`) with a lifecycle owned by codec modules:
//! each `ext/content-codec/<variant>/` module registers its
//! `Arc<dyn ContentCodec>` on load and unregisters it on unload.
//!
//! This is the content-concern counterpart of the input-concern
//! `InputCodecRegistry` living in `reovim-subsys-input`. The two
//! registries stay decoupled — codecs for different concerns never
//! share a registry.

// Re-export the uapi so callers only need a single crate for trait + registry.
pub use reovim_content_codec::{ContentCodec, ContentType};

pub mod capabilities;
mod registry;

#[cfg(test)]
mod registry_tests;

pub use registry::{ContentCodecRegistry, DefaultContentCodecRegistry};
