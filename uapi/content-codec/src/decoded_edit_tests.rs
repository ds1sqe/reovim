//! Tests for `DecodedEdit`, `DomainEdit`, `TreePath`, and `TreeOp`.
//!
//! Domain-specific round-trip tests (e.g., text edits) live alongside
//! their concrete edit types in the ext/content-codec/<domain>/ crates.

use {
    super::{DecodedEdit, DomainEdit, TreeOp, TreePath},
    crate::testing::SyntheticTreeOp,
};

// ============================================================================
// TreePath tests
// ============================================================================

#[test]
fn tree_path_new_empty() {
    let path = TreePath::new(Vec::new());
    assert!(path.components().is_empty());
}

#[test]
fn tree_path_new_with_components() {
    let path = TreePath::new(vec!["a".to_string(), "b".to_string()]);
    assert_eq!(path.components(), &["a".to_string(), "b".to_string()]);
}

#[test]
fn tree_path_root_constructor() {
    let path = TreePath::root();
    assert!(path.is_root());
    assert!(path.components().is_empty());
}

#[test]
fn tree_path_is_root_true_for_empty() {
    let path = TreePath::new(Vec::new());
    assert!(path.is_root());
}

#[test]
fn tree_path_is_root_false_for_nonempty() {
    let path = TreePath::new(vec!["x".to_string()]);
    assert!(!path.is_root());
}

#[test]
fn tree_path_clone_preserves_components() {
    let path = TreePath::new(vec!["a".to_string(), "b".to_string()]);
    let cloned = path.clone();
    assert_eq!(path, cloned);
    assert_eq!(cloned.components(), &["a".to_string(), "b".to_string()]);
}

#[test]
fn tree_path_debug_format() {
    let path = TreePath::new(vec!["foo".to_string()]);
    let debug = format!("{path:?}");
    assert!(debug.contains("TreePath"));
    assert!(debug.contains("foo"));
}

#[test]
fn tree_path_partial_eq() {
    let a = TreePath::new(vec!["x".to_string()]);
    let b = TreePath::new(vec!["x".to_string()]);
    let c = TreePath::new(vec!["y".to_string()]);
    assert_eq!(a, b);
    assert_ne!(a, c);
}

// ============================================================================
// TreeOp wrapper tests
// ============================================================================

#[test]
fn tree_op_new_wraps_concrete_type() {
    let op = TreeOp::new(SyntheticTreeOp {
        name: "noop".to_string(),
    });
    assert!(op.downcast_ref::<SyntheticTreeOp>().is_some());
}

#[test]
fn tree_op_downcast_ref_returns_correct_value() {
    let op = TreeOp::new(SyntheticTreeOp {
        name: "patch".to_string(),
    });
    let inner = op.downcast_ref::<SyntheticTreeOp>().unwrap();
    assert_eq!(inner.name, "patch");
}

#[test]
fn tree_op_downcast_ref_wrong_type_returns_none() {
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct OtherOp;
    crate::impl_tree_op!(OtherOp);

    let op = TreeOp::new(SyntheticTreeOp {
        name: "x".to_string(),
    });
    assert!(op.downcast_ref::<OtherOp>().is_none());
}

#[test]
fn tree_op_clone_preserves_value() {
    let op = TreeOp::new(SyntheticTreeOp {
        name: "clone_me".to_string(),
    });
    let cloned = op.clone();
    assert_eq!(op, cloned);
    let inner = cloned.downcast_ref::<SyntheticTreeOp>().unwrap();
    assert_eq!(inner.name, "clone_me");
}

#[test]
fn tree_op_partial_eq_same_value() {
    let a = TreeOp::new(SyntheticTreeOp {
        name: "eq".to_string(),
    });
    let b = TreeOp::new(SyntheticTreeOp {
        name: "eq".to_string(),
    });
    assert_eq!(a, b);
}

#[test]
fn tree_op_partial_eq_different_value() {
    let a = TreeOp::new(SyntheticTreeOp {
        name: "a".to_string(),
    });
    let b = TreeOp::new(SyntheticTreeOp {
        name: "b".to_string(),
    });
    assert_ne!(a, b);
}

#[test]
fn tree_op_partial_eq_different_types() {
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct AlphaOp;
    crate::impl_tree_op!(AlphaOp);

    let a = TreeOp::new(SyntheticTreeOp {
        name: "x".to_string(),
    });
    let b = TreeOp::new(AlphaOp);
    assert_ne!(a, b);
}

#[test]
fn tree_op_debug_format() {
    let op = TreeOp::new(SyntheticTreeOp {
        name: "rename".to_string(),
    });
    let debug = format!("{op:?}");
    assert!(debug.contains("SyntheticTreeOp"));
    assert!(debug.contains("rename"));
}

// ============================================================================
// DomainEdit wrapper tests (type-erased domain edits)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
struct FakeEdit {
    tag: &'static str,
}
crate::impl_domain_edit!(FakeEdit);

#[derive(Debug, Clone, PartialEq, Eq)]
struct OtherEdit;
crate::impl_domain_edit!(OtherEdit);

#[test]
fn domain_edit_new_wraps_concrete_type() {
    let de = DomainEdit::new(FakeEdit { tag: "insert" });
    assert!(de.downcast_ref::<FakeEdit>().is_some());
}

#[test]
fn domain_edit_downcast_ref_returns_correct_value() {
    let de = DomainEdit::new(FakeEdit { tag: "replace" });
    let inner = de.downcast_ref::<FakeEdit>().unwrap();
    assert_eq!(inner.tag, "replace");
}

#[test]
fn domain_edit_downcast_ref_wrong_type_returns_none() {
    let de = DomainEdit::new(FakeEdit { tag: "x" });
    assert!(de.downcast_ref::<OtherEdit>().is_none());
}

#[test]
fn domain_edit_clone_preserves_value() {
    let de = DomainEdit::new(FakeEdit { tag: "clone_me" });
    let cloned = de.clone();
    assert_eq!(de, cloned);
    let inner = cloned.downcast_ref::<FakeEdit>().unwrap();
    assert_eq!(inner.tag, "clone_me");
}

#[test]
fn domain_edit_partial_eq_same_value() {
    let a = DomainEdit::new(FakeEdit { tag: "eq" });
    let b = DomainEdit::new(FakeEdit { tag: "eq" });
    assert_eq!(a, b);
}

#[test]
fn domain_edit_partial_eq_different_value() {
    let a = DomainEdit::new(FakeEdit { tag: "a" });
    let b = DomainEdit::new(FakeEdit { tag: "b" });
    assert_ne!(a, b);
}

#[test]
fn domain_edit_partial_eq_different_types() {
    let a = DomainEdit::new(FakeEdit { tag: "x" });
    let b = DomainEdit::new(OtherEdit);
    assert_ne!(a, b);
}

#[test]
fn domain_edit_debug_format() {
    let de = DomainEdit::new(FakeEdit { tag: "dbg" });
    let debug = format!("{de:?}");
    assert!(debug.contains("FakeEdit"));
    assert!(debug.contains("dbg"));
}

// ============================================================================
// DecodedEdit::Tree tests
// ============================================================================

#[test]
fn decoded_edit_tree_construction() {
    let edit = DecodedEdit::Tree {
        path: TreePath::root(),
        op: TreeOp::new(SyntheticTreeOp {
            name: "noop".to_string(),
        }),
    };
    assert!(matches!(edit, DecodedEdit::Tree { .. }));
}

#[test]
fn decoded_edit_tree_pattern_match() {
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec!["sections".to_string(), "text".to_string()]),
        op: TreeOp::new(SyntheticTreeOp {
            name: "patch".to_string(),
        }),
    };
    match edit {
        DecodedEdit::Tree { path, op } => {
            assert_eq!(
                path.components(),
                &["sections".to_string(), "text".to_string()]
            );
            let inner = op.downcast_ref::<SyntheticTreeOp>().unwrap();
            assert_eq!(inner.name, "patch");
        }
        _ => panic!("expected DecodedEdit::Tree"),
    }
}

#[test]
fn decoded_edit_tree_debug_format() {
    let edit = DecodedEdit::Tree {
        path: TreePath::root(),
        op: TreeOp::new(SyntheticTreeOp {
            name: "n".to_string(),
        }),
    };
    let debug = format!("{edit:?}");
    assert!(debug.contains("Tree"));
    assert!(debug.contains("TreePath"));
    assert!(debug.contains("SyntheticTreeOp"));
}

#[test]
fn decoded_edit_tree_clone_preserves_fields() {
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec!["a".to_string()]),
        op: TreeOp::new(SyntheticTreeOp {
            name: "b".to_string(),
        }),
    };
    let cloned = edit.clone();
    assert_eq!(edit, cloned);
}

#[test]
fn decoded_edit_tree_value_equality_not_identity() {
    let a = DecodedEdit::Tree {
        path: TreePath::new(vec!["x".to_string()]),
        op: TreeOp::new(SyntheticTreeOp {
            name: "y".to_string(),
        }),
    };
    let b = DecodedEdit::Tree {
        path: TreePath::new(vec!["x".to_string()]),
        op: TreeOp::new(SyntheticTreeOp {
            name: "y".to_string(),
        }),
    };
    assert_eq!(a, b);
}

#[test]
fn decoded_edit_tree_byte_shortcuts_return_false() {
    let edit = DecodedEdit::Tree {
        path: TreePath::root(),
        op: TreeOp::new(SyntheticTreeOp {
            name: "x".to_string(),
        }),
    };
    assert!(!edit.is_byte_insertion());
    assert!(!edit.is_byte_deletion());
}

// ============================================================================
// DecodedEdit::Bytes tests (byte-shaped edit semantics)
// ============================================================================

#[test]
fn bytes_insertion_detection() {
    let edit = DecodedEdit::Bytes {
        offset: 4,
        old_len: 0,
        new_bytes: b"abc".to_vec(),
    };
    assert!(edit.is_byte_insertion());
    assert!(!edit.is_byte_deletion());
}

#[test]
fn bytes_replacement_is_not_insertion() {
    let edit = DecodedEdit::Bytes {
        offset: 0,
        old_len: 2,
        new_bytes: b"xy".to_vec(),
    };
    assert!(!edit.is_byte_insertion());
    assert!(!edit.is_byte_deletion());
}

#[test]
fn bytes_deletion() {
    let edit = DecodedEdit::Bytes {
        offset: 0,
        old_len: 3,
        new_bytes: Vec::new(),
    };
    assert!(!edit.is_byte_insertion());
    assert!(edit.is_byte_deletion());
}

#[test]
fn bytes_empty_noop_is_not_deletion() {
    let edit = DecodedEdit::Bytes {
        offset: 0,
        old_len: 0,
        new_bytes: Vec::new(),
    };
    assert!(edit.is_byte_insertion());
    assert!(!edit.is_byte_deletion());
}

#[test]
fn bytes_edit_variant_fields_used() {
    let edit = DecodedEdit::Bytes {
        offset: 8,
        old_len: 2,
        new_bytes: vec![0xAA, 0xBB],
    };
    assert!(matches!(
        edit,
        DecodedEdit::Bytes {
            offset: 8,
            old_len: 2,
            new_bytes
        } if new_bytes == vec![0xAA, 0xBB]
    ));
}

// ============================================================================
// DecodedEdit::Domain tests (type-erased domain payload)
// ============================================================================

#[test]
fn domain_variant_construction() {
    let edit = DecodedEdit::Domain(DomainEdit::new(FakeEdit { tag: "insert" }));
    match edit {
        DecodedEdit::Domain(inner) => {
            assert_eq!(inner.downcast_ref::<FakeEdit>().unwrap().tag, "insert");
        }
        _ => panic!("expected DecodedEdit::Domain"),
    }
}

#[test]
fn domain_variant_byte_shortcuts_return_false() {
    let edit = DecodedEdit::Domain(DomainEdit::new(FakeEdit { tag: "x" }));
    assert!(!edit.is_byte_insertion());
    assert!(!edit.is_byte_deletion());
}

#[test]
fn domain_variant_value_equality() {
    let a = DecodedEdit::Domain(DomainEdit::new(FakeEdit { tag: "eq" }));
    let b = DecodedEdit::Domain(DomainEdit::new(FakeEdit { tag: "eq" }));
    assert_eq!(a, b);
}

#[test]
fn domain_variant_value_inequality_different_tags() {
    let a = DecodedEdit::Domain(DomainEdit::new(FakeEdit { tag: "a" }));
    let b = DecodedEdit::Domain(DomainEdit::new(FakeEdit { tag: "b" }));
    assert_ne!(a, b);
}
