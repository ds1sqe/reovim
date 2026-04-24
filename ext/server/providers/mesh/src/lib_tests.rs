use reovim_domain_mesh::{MeshEdit, Vertex};

use super::MeshStore;

#[test]
fn new_store_is_empty() {
    let s = MeshStore::new();
    assert!(s.content().is_empty());
}

#[test]
fn default_impl_matches_new() {
    let a = MeshStore::new();
    let b = MeshStore::default();
    assert_eq!(a.content(), b.content());
}

#[test]
fn apply_replace_updates_content() {
    let mut s = MeshStore::new();
    let vs = vec![
        Vertex {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        },
        Vertex {
            x: 4.0,
            y: 5.0,
            z: 6.0,
        },
    ];
    s.apply_edit(MeshEdit::Replace(vs.clone()));
    assert_eq!(s.content(), &vs);
}

#[test]
fn apply_replace_then_apply_replace_uses_latest() {
    let mut s = MeshStore::new();
    s.apply_edit(MeshEdit::Replace(vec![Vertex {
        x: 1.0,
        y: 0.0,
        z: 0.0,
    }]));
    let second = vec![Vertex {
        x: 9.0,
        y: 9.0,
        z: 9.0,
    }];
    s.apply_edit(MeshEdit::Replace(second.clone()));
    assert_eq!(s.content(), &second);
}
