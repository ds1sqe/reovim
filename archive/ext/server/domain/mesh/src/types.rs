//! Mesh domain scaffold types (#753 Flight 77).
//!
//! A deliberately narrow surface: a flat vertex list plus one edit
//! operation. The goal is not a real mesh editor but a second
//! [`reovim_domain::Domain`] impl that proves domain-neutrality
//! beyond the text domain. Topology (edges, faces, half-edges), BVH,
//! subdivision, and per-vertex / per-face edits are all deferred.

/// A single 3-D vertex. `f32` is sufficient for a scaffold; a real
/// mesh domain picks its own precision when it lands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
    /// Z coordinate.
    pub z: f32,
}

/// A location within a mesh: an index into the vertex list.
///
/// Positions are not invalidated by any [`MeshEdit`] variant in
/// Flight 77 — the sole operation is full-list replacement, which
/// makes every prior position refer to a new (unrelated) vertex.
/// That's acceptable for the scaffold; a real mesh domain adds
/// structural edits that the position model must survive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshPosition {
    /// Vertex index.
    pub vertex: u32,
}

/// The mesh domain's single edit operation.
///
/// Exists as an enum (rather than a struct) so future variants don't
/// break pattern-matching on existing consumers.
#[derive(Debug, Clone, PartialEq)]
pub enum MeshEdit {
    /// Replace the entire vertex list with a new one.
    Replace(Vec<Vertex>),
}

/// Decoded mesh content at rest: the flat vertex list.
pub type MeshContent = Vec<Vertex>;
