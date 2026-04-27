//! Provider round-trip integration test (#753 Flight 77).
//!
//! Exercises the `MeshStore::apply_edit` path end-to-end against a
//! known vertex list, locking the provider's single edit semantics.
//! The unit tests inside `src/lib_tests.rs` cover individual method
//! contracts; this file asserts the external contract from a
//! downstream crate's point of view.

use {
    reovim_domain_mesh::{MeshEdit, Vertex},
    reovim_provider_mesh::MeshStore,
};

#[test]
fn mesh_store_round_trips_replace_edit() {
    let mut s = MeshStore::new();
    assert!(s.content().is_empty());

    let cube = vec![
        Vertex {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        Vertex {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        },
        Vertex {
            x: 1.0,
            y: 1.0,
            z: 0.0,
        },
        Vertex {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        },
        Vertex {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        },
        Vertex {
            x: 1.0,
            y: 0.0,
            z: 1.0,
        },
        Vertex {
            x: 1.0,
            y: 1.0,
            z: 1.0,
        },
        Vertex {
            x: 0.0,
            y: 1.0,
            z: 1.0,
        },
    ];
    s.apply_edit(MeshEdit::Replace(cube.clone()));
    assert_eq!(s.content(), &cube);
    assert_eq!(s.content().len(), 8);
}
