use super::types::{MeshContent, MeshEdit, MeshPosition, Vertex};

/// Compile-time `T: Copy` proof.
const fn assert_copy<T: Copy>() {}
/// Compile-time `T: Send + Sync + 'static` proof.
const fn assert_send_sync_static<T: Send + Sync + 'static>() {}
/// Compile-time `Domain::Position` bound proof (includes `PartialEq`).
const fn assert_position_bounds<T: Send + Sync + Clone + core::fmt::Debug + PartialEq + 'static>() {}
/// Compile-time `Domain::Edit` / `Domain::Content` bound proof.
const fn assert_edit_content_bounds<T: Send + Sync + Clone + core::fmt::Debug + 'static>() {}

#[test]
fn vertex_construction_and_equality() {
    let a = Vertex {
        x: 1.0,
        y: 2.0,
        z: 3.0,
    };
    let b = Vertex {
        x: 1.0,
        y: 2.0,
        z: 3.0,
    };
    assert_eq!(a, b);
    // `PartialEq` on `f32` is fine when comparing values constructed
    // from identical literals — no arithmetic has run, no ULP drift.
    assert_eq!(a, Vertex { x: 1.0, y: 2.0, z: 3.0 });
}

#[test]
fn vertex_is_copy() {
    assert_copy::<Vertex>();
    let v = Vertex {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    let _ = v;
    let _ = v; // would fail to compile without Copy
}

#[test]
fn mesh_position_vertex_index_round_trip() {
    let p = MeshPosition { vertex: 42 };
    assert_eq!(p.vertex, 42);
    assert_copy::<MeshPosition>();
}

#[test]
fn mesh_edit_replace_carries_vertex_list() {
    let vs = vec![
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
    ];
    let edit = MeshEdit::Replace(vs.clone());
    match edit {
        MeshEdit::Replace(got) => assert_eq!(got, vs),
    }
}

#[test]
fn mesh_edit_is_clone_and_debug() {
    let edit = MeshEdit::Replace(vec![Vertex {
        x: 1.0,
        y: 2.0,
        z: 3.0,
    }]);
    let cloned = edit.clone();
    assert_eq!(edit, cloned);
    let debug = format!("{edit:?}");
    assert!(debug.contains("Replace"));
}

#[test]
fn mesh_content_is_vec_of_vertex() {
    let c: MeshContent = Vec::<Vertex>::new();
    assert!(c.is_empty());
}

#[test]
fn types_satisfy_send_sync_static() {
    assert_send_sync_static::<Vertex>();
    assert_send_sync_static::<MeshPosition>();
    assert_send_sync_static::<MeshEdit>();
    assert_send_sync_static::<MeshContent>();
}

#[test]
fn types_satisfy_domain_trait_bounds() {
    assert_position_bounds::<MeshPosition>();
    assert_edit_content_bounds::<MeshEdit>();
    assert_edit_content_bounds::<MeshContent>();
}
