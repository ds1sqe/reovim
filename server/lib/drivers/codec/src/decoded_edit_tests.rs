//! Tests for `DecodedEdit`, `TreePath`, and `TreeOp`.

use super::{DecodedEdit, TreeOp, TreePath};

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
// TreeOp tests (Phase 1: only Synthetic variant exists)
// ============================================================================

#[test]
fn tree_op_synthetic_construction() {
    let op = TreeOp::Synthetic {
        name: "noop".to_string(),
    };
    assert!(matches!(op, TreeOp::Synthetic { name } if name == "noop"));
}

#[test]
fn tree_op_synthetic_clone_and_eq() {
    let op = TreeOp::Synthetic {
        name: "replace".to_string(),
    };
    let cloned = op.clone();
    assert_eq!(op, cloned);
}

#[test]
fn tree_op_synthetic_debug_format() {
    let op = TreeOp::Synthetic {
        name: "rename".to_string(),
    };
    let debug = format!("{op:?}");
    assert!(debug.contains("Synthetic"));
    assert!(debug.contains("rename"));
}

// ============================================================================
// DecodedEdit::Tree tests
// ============================================================================

#[test]
fn decoded_edit_tree_construction() {
    let edit = DecodedEdit::Tree {
        path: TreePath::root(),
        op: TreeOp::Synthetic {
            name: "noop".to_string(),
        },
    };
    assert!(matches!(edit, DecodedEdit::Tree { .. }));
}

#[test]
fn decoded_edit_tree_pattern_match() {
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec!["sections".to_string(), "text".to_string()]),
        op: TreeOp::Synthetic {
            name: "patch".to_string(),
        },
    };
    match edit {
        DecodedEdit::Tree { path, op } => {
            assert_eq!(path.components(), &["sections".to_string(), "text".to_string()]);
            assert!(matches!(op, TreeOp::Synthetic { name } if name == "patch"));
        }
        _ => panic!("expected DecodedEdit::Tree"),
    }
}

#[test]
fn decoded_edit_tree_debug_format() {
    let edit = DecodedEdit::Tree {
        path: TreePath::root(),
        op: TreeOp::Synthetic {
            name: "n".to_string(),
        },
    };
    let debug = format!("{edit:?}");
    assert!(debug.contains("Tree"));
    assert!(debug.contains("TreePath"));
    assert!(debug.contains("Synthetic"));
}

#[test]
fn decoded_edit_tree_clone_preserves_fields() {
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec!["a".to_string()]),
        op: TreeOp::Synthetic {
            name: "b".to_string(),
        },
    };
    let cloned = edit.clone();
    assert_eq!(edit, cloned);
}

#[test]
fn decoded_edit_tree_value_equality_not_identity() {
    // Two independently-constructed Tree edits with logically identical
    // contents MUST compare equal (verifying we are not on a dyn-trait
    // identity-equality path).
    let a = DecodedEdit::Tree {
        path: TreePath::new(vec!["x".to_string()]),
        op: TreeOp::Synthetic {
            name: "y".to_string(),
        },
    };
    let b = DecodedEdit::Tree {
        path: TreePath::new(vec!["x".to_string()]),
        op: TreeOp::Synthetic {
            name: "y".to_string(),
        },
    };
    assert_eq!(a, b);
}

#[test]
fn decoded_edit_tree_is_not_insertion_or_deletion() {
    let edit = DecodedEdit::Tree {
        path: TreePath::root(),
        op: TreeOp::Synthetic {
            name: "x".to_string(),
        },
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
