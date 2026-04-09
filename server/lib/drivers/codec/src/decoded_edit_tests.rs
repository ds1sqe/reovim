//! Tests for `DecodedEdit`, `TreePath`, and `TreeOp`.

use {
    super::{DecodedEdit, TreeOp, TreePath},
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
    // A second concrete type to test cross-type downcast failure.
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
            assert_eq!(path.components(), &["sections".to_string(), "text".to_string()]);
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
    // Two independently-constructed Tree edits with logically identical
    // contents MUST compare equal (verifying value-based equality through
    // the type-erased wrapper).
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
fn decoded_edit_tree_is_not_insertion_or_deletion() {
    let edit = DecodedEdit::Tree {
        path: TreePath::root(),
        op: TreeOp::new(SyntheticTreeOp {
            name: "x".to_string(),
        }),
    };
    // Structural edits do not participate in the simple text-level
    // is_insertion/is_deletion shortcuts.
    assert!(!edit.is_insertion());
    assert!(!edit.is_deletion());
}

// ============================================================================
// Existing Text and Bytes variant tests (preserved from pre-Phase 1)
// ============================================================================

#[test]
fn insertion_detection() {
    let edit = DecodedEdit::Bytes {
        offset: 4,
        old_len: 0,
        new_bytes: b"abc".to_vec(),
    };
    assert!(edit.is_insertion());
    assert!(!edit.is_deletion());
}

#[test]
fn deletion_detection() {
    let edit = DecodedEdit::Text {
        start: reovim_types_text::Position::new(0, 0),
        end: reovim_types_text::Position::new(0, 3),
        replacement: String::new(),
    };
    assert!(!edit.is_insertion());
    assert!(edit.is_deletion());
}

#[test]
fn text_edit_variant_fields_used() {
    let edit = DecodedEdit::Text {
        start: reovim_types_text::Position::new(0, 2),
        end: reovim_types_text::Position::new(0, 3),
        replacement: "x".to_string(),
    };
    let expected_start = reovim_types_text::Position::new(0, 2);
    let expected_end = reovim_types_text::Position::new(0, 3);

    assert!(matches!(
        edit,
        DecodedEdit::Text {
            start,
            end,
            replacement
        } if replacement == "x"
            && start == expected_start
            && end == expected_end
    ));
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
