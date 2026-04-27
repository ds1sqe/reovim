use reovim_domain::Domain;

use super::{
    domain::Mesh,
    types::{MeshContent, MeshEdit, MeshPosition},
};

#[test]
fn mesh_implements_domain() {
    // Compile-time: any rename of the trait or the impl breaks here.
    fn assert_domain<T: Domain>() {}
    assert_domain::<Mesh>();
}

#[test]
fn mesh_domain_position_is_mesh_position() {
    // Function-binding identity witness: the compiler accepts this
    // only when `<Mesh as Domain>::Position` IS `MeshPosition`.
    fn identity(p: MeshPosition) -> <Mesh as Domain>::Position {
        p
    }
    let got = identity(MeshPosition { vertex: 7 });
    assert_eq!(got.vertex, 7);
}

#[test]
fn mesh_domain_edit_is_mesh_edit() {
    fn identity(e: MeshEdit) -> <Mesh as Domain>::Edit {
        e
    }
    let got = identity(MeshEdit::Replace(Vec::new()));
    assert!(matches!(got, MeshEdit::Replace(_)));
}

#[test]
fn mesh_domain_content_is_mesh_content() {
    fn identity(c: MeshContent) -> <Mesh as Domain>::Content {
        c
    }
    let got = identity(Vec::new());
    assert!(got.is_empty());
}
