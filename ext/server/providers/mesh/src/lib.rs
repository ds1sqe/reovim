#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Mesh content provider stub for reovim (#753 Flight 77).
//!
//! Companion to `reovim-domain-mesh`. Offers a trivial
//! [`MeshStore`] that holds the current mesh content and applies
//! [`MeshEdit`]s.  Exists purely to demonstrate that the
//! provider-under-domain pattern works for a non-text domain —
//! deliberately far narrower than `reovim-provider-text`.

use reovim_domain_mesh::{MeshContent, MeshEdit};

/// In-memory mesh store.
///
/// Holds the current [`MeshContent`] (a flat `Vec<Vertex>`) and
/// applies [`MeshEdit`]s. No persistence, no change history, no
/// concurrent access — that's all future work.
#[derive(Debug, Default, Clone)]
pub struct MeshStore {
    content: MeshContent,
}

impl MeshStore {
    /// Create a new empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Borrow the current content.
    #[must_use]
    pub const fn content(&self) -> &MeshContent {
        &self.content
    }

    /// Apply a [`MeshEdit`] to the store. The scaffold's sole
    /// operation is [`MeshEdit::Replace`], which swaps the entire
    /// vertex list.
    pub fn apply_edit(&mut self, edit: MeshEdit) {
        match edit {
            MeshEdit::Replace(vertices) => self.content = vertices,
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
