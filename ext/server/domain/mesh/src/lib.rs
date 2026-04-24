#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Mesh content domain scaffold for reovim (#753 Flight 77).
//!
//! Exists to prove the zero-edit-for-new-domain invariant: a second
//! concrete [`reovim_domain::Domain`] impl sits under `ext/server/`
//! alongside the pre-existing text domain, without requiring any
//! change to the kernel, server subsys, client subsys, or render
//! pipeline. The existing depgraph probes
//! (`kernel_no_domain`, `server_no_domain`, `subsys_no_domain`)
//! enforce that invariant automatically through their prefix-match
//! rules; this crate's `domain_mesh_exists` probe adds a shape-lock
//! on the new crate.
//!
//! Intentionally narrow: a `Vec<Vertex>` for content plus a single
//! `MeshEdit::Replace` op. Topology (edges, faces, half-edges, BVH),
//! per-vertex / per-face edits, file formats, and any rasterizer or
//! gRPC integration are all deferred.

pub mod domain;
pub mod types;

pub use {
    domain::Mesh,
    types::{MeshContent, MeshEdit, MeshPosition, Vertex},
};

#[cfg(test)]
#[path = "domain_tests.rs"]
mod domain_tests;
#[cfg(test)]
#[path = "types_tests.rs"]
mod types_tests;
