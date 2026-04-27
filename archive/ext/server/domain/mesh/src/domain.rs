//! `impl Domain for Mesh` — the scaffold's load-bearing proof that
//! [`reovim_domain::Domain`] is satisfiable by a non-text domain.

use reovim_domain::Domain;

use crate::types::{MeshContent, MeshEdit, MeshPosition};

/// Mesh content domain marker (Flight 77 scaffold).
///
/// Binds the abstract [`Domain`] associated types to the concrete
/// mesh types in [`crate::types`]. The crate carries no storage,
/// algorithms, or edit-application logic — that lives in the
/// sibling `reovim-provider-mesh` crate, which is itself a stub.
#[derive(Debug, Default, Clone, Copy)]
pub struct Mesh;

impl Domain for Mesh {
    type Position = MeshPosition;
    type Edit = MeshEdit;
    type Content = MeshContent;
}
