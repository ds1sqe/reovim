use super::*;

fn create_test_tree() -> SerializableUndoTree {
    SerializableUndoTree {
        nodes: vec![
            SerializableUndoNode {
                edits: vec![],
                cursor_before: SerializablePosition::default(),
                cursor_after: SerializablePosition::default(),
                relative_time_secs: 0.0,
                parent: None,
                children: vec![1],
                seq_num: 0,
                origin: SerializableEditOrigin::System,
            },
            SerializableUndoNode {
                edits: vec![SerializableEdit::Insert {
                    position: SerializablePosition::new(0, 0),
                    text: "hello".to_string(),
                }],
                cursor_before: SerializablePosition::new(0, 0),
                cursor_after: SerializablePosition::new(0, 5),
                relative_time_secs: 1.5,
                parent: Some(0),
                children: vec![],
                seq_num: 1,
                origin: SerializableEditOrigin::System,
            },
        ],
        current: 1,
        seq_counter: 1,
        max_nodes: 10000,
        active_branches: vec![0, 0],
    }
}

#[test]
fn test_undo_file_format_roundtrip() {
    let tree = create_test_tree();
    let format = UndoFileFormat::new("/test/file.rs".to_string(), tree);

    let bytes = format.to_bytes().expect("serialization should succeed");
    let restored = UndoFileFormat::from_bytes(&bytes).expect("deserialization should succeed");

    assert_eq!(restored.version, UNDO_FILE_VERSION);
    assert_eq!(restored.original_path, "/test/file.rs");
    assert_eq!(restored.tree.current, 1);
    assert_eq!(restored.tree.nodes.len(), 2);
}

#[test]
fn test_magic_bytes_validation() {
    // Too short
    assert!(matches!(UndoFileFormat::from_bytes(&[0, 1, 2]), Err(UndoFileError::TooShort)));

    // Wrong magic
    assert!(matches!(
        UndoFileFormat::from_bytes(b"XXXX...."),
        Err(UndoFileError::InvalidMagic)
    ));
}

#[test]
fn test_version_field_preserved() {
    let tree = create_test_tree();
    let format = UndoFileFormat::new("/test.rs".to_string(), tree);

    let bytes = format.to_bytes().unwrap();
    let restored = UndoFileFormat::from_bytes(&bytes).unwrap();

    assert_eq!(restored.version, UNDO_FILE_VERSION);
}

#[test]
fn test_position_serialization() {
    let pos = SerializablePosition::new(10, 20);
    let bytes = rmp_serde::to_vec(&pos).unwrap();
    let restored: SerializablePosition = rmp_serde::from_slice(&bytes).unwrap();
    assert_eq!(pos, restored);
}

#[test]
fn test_edit_serialization() {
    let insert = SerializableEdit::Insert {
        position: SerializablePosition::new(5, 10),
        text: "test".to_string(),
    };
    let bytes = rmp_serde::to_vec(&insert).unwrap();
    let restored: SerializableEdit = rmp_serde::from_slice(&bytes).unwrap();
    assert_eq!(insert, restored);

    let delete = SerializableEdit::Delete {
        position: SerializablePosition::new(1, 0),
        text: "removed".to_string(),
    };
    let bytes = rmp_serde::to_vec(&delete).unwrap();
    let restored: SerializableEdit = rmp_serde::from_slice(&bytes).unwrap();
    assert_eq!(delete, restored);
}

#[test]
fn test_empty_tree_roundtrip() {
    let tree = SerializableUndoTree {
        nodes: vec![SerializableUndoNode {
            edits: vec![],
            cursor_before: SerializablePosition::default(),
            cursor_after: SerializablePosition::default(),
            relative_time_secs: 0.0,
            parent: None,
            children: vec![],
            seq_num: 0,
            origin: SerializableEditOrigin::System,
        }],
        current: 0,
        seq_counter: 0,
        max_nodes: 10000,
        active_branches: vec![0],
    };
    let format = UndoFileFormat::new("/empty.rs".to_string(), tree);

    let bytes = format.to_bytes().unwrap();
    let restored = UndoFileFormat::from_bytes(&bytes).unwrap();

    assert_eq!(restored.tree.nodes.len(), 1);
    assert_eq!(restored.tree.current, 0);
}

#[test]
fn test_multi_branch_tree_roundtrip() {
    // Tree structure:
    //       0 (root)
    //      / \
    //     1   2
    //    /
    //   3
    let tree = SerializableUndoTree {
        nodes: vec![
            SerializableUndoNode {
                edits: vec![],
                cursor_before: SerializablePosition::default(),
                cursor_after: SerializablePosition::default(),
                relative_time_secs: 0.0,
                parent: None,
                children: vec![1, 2],
                seq_num: 0,
                origin: SerializableEditOrigin::System,
            },
            SerializableUndoNode {
                edits: vec![SerializableEdit::Insert {
                    position: SerializablePosition::new(0, 0),
                    text: "a".to_string(),
                }],
                cursor_before: SerializablePosition::new(0, 0),
                cursor_after: SerializablePosition::new(0, 1),
                relative_time_secs: 1.0,
                parent: Some(0),
                children: vec![3],
                seq_num: 1,
                origin: SerializableEditOrigin::Client(0),
            },
            SerializableUndoNode {
                edits: vec![SerializableEdit::Insert {
                    position: SerializablePosition::new(0, 0),
                    text: "b".to_string(),
                }],
                cursor_before: SerializablePosition::new(0, 0),
                cursor_after: SerializablePosition::new(0, 1),
                relative_time_secs: 2.0,
                parent: Some(0),
                children: vec![],
                seq_num: 2,
                origin: SerializableEditOrigin::Client(1),
            },
            SerializableUndoNode {
                edits: vec![SerializableEdit::Insert {
                    position: SerializablePosition::new(0, 1),
                    text: "c".to_string(),
                }],
                cursor_before: SerializablePosition::new(0, 1),
                cursor_after: SerializablePosition::new(0, 2),
                relative_time_secs: 3.0,
                parent: Some(1),
                children: vec![],
                seq_num: 3,
                origin: SerializableEditOrigin::Client(0),
            },
        ],
        current: 3,
        seq_counter: 3,
        max_nodes: 10000,
        active_branches: vec![0, 0, 0, 0],
    };
    let format = UndoFileFormat::new("/branched.rs".to_string(), tree);

    let bytes = format.to_bytes().unwrap();
    let restored = UndoFileFormat::from_bytes(&bytes).unwrap();

    assert_eq!(restored.tree.nodes.len(), 4);
    assert_eq!(restored.tree.current, 3);
    assert_eq!(restored.tree.nodes[0].children, vec![1, 2]);
    assert_eq!(restored.tree.nodes[1].children, vec![3]);
}

// ========================================================================
// Kernel conversion tests
// ========================================================================

#[test]
fn test_position_kernel_conversion() {
    let kernel_pos = Position::new(10, 20);
    let serializable: SerializablePosition = kernel_pos.into();
    assert_eq!(serializable.line, 10);
    assert_eq!(serializable.column, 20);

    let back: Position = serializable.into();
    assert_eq!(back, kernel_pos);
}

#[test]
fn test_edit_origin_conversion() {
    // System origin
    let system = EditOrigin::System;
    let serializable: SerializableEditOrigin = system.into();
    assert_eq!(serializable, SerializableEditOrigin::System);
    let back: EditOrigin = serializable.into();
    assert_eq!(back, EditOrigin::System);

    // Client origin
    let client = EditOrigin::Client(42);
    let serializable: SerializableEditOrigin = client.into();
    assert_eq!(serializable, SerializableEditOrigin::Client(42));
    let back: EditOrigin = serializable.into();
    assert_eq!(back, EditOrigin::Client(42));
}

#[test]
fn test_edit_origin_serialization() {
    let system = SerializableEditOrigin::System;
    let bytes = rmp_serde::to_vec(&system).unwrap();
    let restored: SerializableEditOrigin = rmp_serde::from_slice(&bytes).unwrap();
    assert_eq!(system, restored);

    let client = SerializableEditOrigin::Client(123);
    let bytes = rmp_serde::to_vec(&client).unwrap();
    let restored: SerializableEditOrigin = rmp_serde::from_slice(&bytes).unwrap();
    assert_eq!(client, restored);
}

#[test]
fn test_edit_origin_preserved_through_roundtrip() {
    let mut tree = UndoTree::new();

    // Push with different origins
    tree.push_with_origin(
        vec![Edit::insert(Position::new(0, 0), "a")],
        Position::new(0, 0),
        Position::new(0, 1),
        EditOrigin::Client(1),
    );
    tree.push_with_origin(
        vec![Edit::insert(Position::new(0, 1), "b")],
        Position::new(0, 1),
        Position::new(0, 2),
        EditOrigin::System,
    );
    tree.push_with_origin(
        vec![Edit::insert(Position::new(0, 2), "c")],
        Position::new(0, 2),
        Position::new(0, 3),
        EditOrigin::Client(2),
    );

    // Serialize
    let serializable = from_undo_tree(&tree);
    assert_eq!(serializable.nodes[1].origin, SerializableEditOrigin::Client(1));
    assert_eq!(serializable.nodes[2].origin, SerializableEditOrigin::System);
    assert_eq!(serializable.nodes[3].origin, SerializableEditOrigin::Client(2));

    // Full roundtrip through file format
    let format = UndoFileFormat::new("/test.rs".to_string(), serializable);
    let bytes = format.to_bytes().unwrap();
    let restored_format = UndoFileFormat::from_bytes(&bytes).unwrap();
    let restored_tree = to_undo_tree(&restored_format.tree);

    // Verify origins preserved
    assert_eq!(restored_tree.node(1).unwrap().origin(), EditOrigin::Client(1));
    assert_eq!(restored_tree.node(2).unwrap().origin(), EditOrigin::System);
    assert_eq!(restored_tree.node(3).unwrap().origin(), EditOrigin::Client(2));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_edit_kernel_conversion() {
    let insert = Edit::insert(Position::new(5, 10), "test");
    let serializable = SerializableEdit::from(&insert);
    assert!(matches!(
        &serializable,
        SerializableEdit::Insert { position, text }
        if position.line == 5 && position.column == 10 && text == "test"
    ));

    let back: Edit = serializable.into();
    assert_eq!(back.position(), Position::new(5, 10));
    assert_eq!(back.text(), "test");
    assert!(back.is_insert());

    let delete = Edit::delete(Position::new(1, 0), "removed");
    let serializable = SerializableEdit::from(&delete);
    let back: Edit = serializable.into();
    assert!(back.is_delete());
    assert_eq!(back.text(), "removed");
}

#[test]
fn test_undo_tree_kernel_roundtrip() {
    let mut tree = UndoTree::new();

    // Make some edits
    tree.push(
        vec![Edit::insert(Position::new(0, 0), "hello")],
        Position::new(0, 0),
        Position::new(0, 5),
    );
    tree.push(
        vec![Edit::insert(Position::new(0, 5), " world")],
        Position::new(0, 5),
        Position::new(0, 11),
    );

    // Convert to serializable
    let serializable = from_undo_tree(&tree);
    assert_eq!(serializable.nodes.len(), 3); // root + 2 edits
    assert_eq!(serializable.current, 2);

    // Convert back
    let restored = to_undo_tree(&serializable);
    assert_eq!(restored.node_count(), 3);
    assert_eq!(restored.current_index(), 2);

    // Verify undo still works
    let mut restored = restored;
    let undo_result = restored.undo();
    assert!(undo_result.is_some());
    assert_eq!(restored.current_index(), 1);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_timestamp_ordering_preserved() {
    // Create a tree with nodes that have increasing relative times
    let serializable = SerializableUndoTree {
        nodes: vec![
            SerializableUndoNode {
                edits: vec![],
                cursor_before: SerializablePosition::default(),
                cursor_after: SerializablePosition::default(),
                relative_time_secs: 0.0,
                parent: None,
                children: vec![1, 2, 3, 4],
                seq_num: 0,
                origin: SerializableEditOrigin::System,
            },
            SerializableUndoNode {
                edits: vec![],
                cursor_before: SerializablePosition::default(),
                cursor_after: SerializablePosition::default(),
                relative_time_secs: 1.0,
                parent: Some(0),
                children: vec![],
                seq_num: 1,
                origin: SerializableEditOrigin::System,
            },
            SerializableUndoNode {
                edits: vec![],
                cursor_before: SerializablePosition::default(),
                cursor_after: SerializablePosition::default(),
                relative_time_secs: 2.5,
                parent: Some(0),
                children: vec![],
                seq_num: 2,
                origin: SerializableEditOrigin::System,
            },
            SerializableUndoNode {
                edits: vec![],
                cursor_before: SerializablePosition::default(),
                cursor_after: SerializablePosition::default(),
                relative_time_secs: 3.0,
                parent: Some(0),
                children: vec![],
                seq_num: 3,
                origin: SerializableEditOrigin::System,
            },
            SerializableUndoNode {
                edits: vec![],
                cursor_before: SerializablePosition::default(),
                cursor_after: SerializablePosition::default(),
                relative_time_secs: 5.0,
                parent: Some(0),
                children: vec![],
                seq_num: 4,
                origin: SerializableEditOrigin::System,
            },
        ],
        current: 4,
        seq_counter: 4,
        max_nodes: 10000,
        active_branches: vec![0, 0, 0, 0, 0],
    };

    // Convert to kernel tree
    let tree = to_undo_tree(&serializable);

    // Verify timestamp ordering is preserved
    // Node with relative_time_secs = 1.0 should have timestamp before node with 2.5, etc.
    for i in 1..tree.node_count() {
        for j in (i + 1)..tree.node_count() {
            let node_i = tree.node(i).unwrap();
            let node_j = tree.node(j).unwrap();

            // Get original relative times
            let time_i = serializable.nodes[i].relative_time_secs;
            let time_j = serializable.nodes[j].relative_time_secs;

            // If node i was earlier than node j in original, it should still be
            if time_i < time_j {
                assert!(
                    node_i.timestamp() <= node_j.timestamp(),
                    "Timestamp ordering violated: node {i} (rel: {time_i}) should be <= node {j} (rel: {time_j})"
                );
            } else if time_i > time_j {
                assert!(
                    node_i.timestamp() >= node_j.timestamp(),
                    "Timestamp ordering violated: node {i} (rel: {time_i}) should be >= node {j} (rel: {time_j})"
                );
            }
        }
    }
}

#[test]
fn test_full_roundtrip_through_file_format() {
    // Create kernel tree
    let mut tree = UndoTree::new();
    tree.push(
        vec![Edit::insert(Position::new(0, 0), "a")],
        Position::new(0, 0),
        Position::new(0, 1),
    );
    tree.push(
        vec![Edit::insert(Position::new(0, 1), "b")],
        Position::new(0, 1),
        Position::new(0, 2),
    );

    // Undo to create a branch point
    tree.undo();

    // Add different edit (creates branch)
    tree.push(
        vec![Edit::insert(Position::new(0, 1), "c")],
        Position::new(0, 1),
        Position::new(0, 2),
    );

    // Full roundtrip: kernel -> serializable -> bytes -> serializable -> kernel
    let serializable = from_undo_tree(&tree);
    let format = UndoFileFormat::new("/test.rs".to_string(), serializable);
    let bytes = format.to_bytes().unwrap();
    let restored_format = UndoFileFormat::from_bytes(&bytes).unwrap();
    let restored_tree = to_undo_tree(&restored_format.tree);

    // Verify structure preserved
    assert_eq!(restored_tree.node_count(), tree.node_count());
    assert_eq!(restored_tree.current_index(), tree.current_index());

    // Verify branches preserved
    let original_root = tree.node(0).unwrap();
    let restored_root = restored_tree.node(0).unwrap();
    assert_eq!(original_root.children().len(), restored_root.children().len());
}

#[test]
fn test_undo_file_error_display() {
    assert_eq!(UndoFileError::TooShort.to_string(), "File too short to be valid undo file");
    assert_eq!(UndoFileError::InvalidMagic.to_string(), "Invalid undo file magic bytes");
}

#[test]
fn test_undo_file_error_display_io() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let err = UndoFileError::Io(io_err);
    let display = err.to_string();
    assert!(display.contains("I/O error"));
    assert!(display.contains("file not found"));
}

#[test]
fn test_undo_file_error_from_io() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
    let err: UndoFileError = io_err.into();
    assert!(matches!(err, UndoFileError::Io(_)));
}

#[test]
fn test_undo_file_error_is_error_trait() {
    fn assert_error<E: std::error::Error>() {}
    assert_error::<UndoFileError>();
}

#[test]
fn test_undo_file_error_deserialize_display() {
    // Create an invalid msgpack payload with valid magic
    let mut bytes = UNDO_FILE_MAGIC.to_vec();
    bytes.extend(b"invalid msgpack data");
    let err = UndoFileFormat::from_bytes(&bytes).unwrap_err();
    assert!(matches!(err, UndoFileError::Deserialize(_)));
    let display = err.to_string();
    assert!(display.contains("Failed to deserialize"));
}

#[test]
fn test_serializable_position_default() {
    let pos = SerializablePosition::default();
    assert_eq!(pos.line, 0);
    assert_eq!(pos.column, 0);
}

#[test]
fn test_serializable_edit_origin_default() {
    let origin = SerializableEditOrigin::default();
    assert_eq!(origin, SerializableEditOrigin::System);
}

#[test]
fn test_undo_file_magic_bytes() {
    assert_eq!(&UNDO_FILE_MAGIC, b"RUND");
}

#[test]
fn test_undo_file_version() {
    assert_eq!(UNDO_FILE_VERSION, 1);
}

#[test]
fn test_undo_file_format_metadata() {
    let tree = create_test_tree();
    let format = UndoFileFormat::new("/test/meta.rs".to_string(), tree);
    assert_eq!(format.version, UNDO_FILE_VERSION);
    assert_eq!(format.original_path, "/test/meta.rs");
    assert!(format.created_at > 0);
    assert!(!format.reovim_version.is_empty());
}

#[test]
fn test_delete_edit_kernel_conversion() {
    // Test the Delete branch of SerializableEdit -> Edit conversion
    let delete = SerializableEdit::Delete {
        position: SerializablePosition::new(3, 7),
        text: "deleted text".to_string(),
    };
    let edit: Edit = delete.into();
    assert!(edit.is_delete());
    assert_eq!(edit.position(), Position::new(3, 7));
    assert_eq!(edit.text(), "deleted text");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_delete_edit_from_kernel() {
    // Test the Delete branch of &Edit -> SerializableEdit conversion
    let kernel_delete = Edit::delete(Position::new(2, 5), "removed");
    let serializable = SerializableEdit::from(&kernel_delete);
    match &serializable {
        SerializableEdit::Delete { position, text } => {
            assert_eq!(position.line, 2);
            assert_eq!(position.column, 5);
            assert_eq!(text, "removed");
        }
        SerializableEdit::Insert { .. } => panic!("Expected Delete variant"),
    }
}

#[test]
fn test_tree_with_delete_edits_roundtrip() {
    // Full roundtrip with delete edits to cover all conversion branches
    let mut tree = UndoTree::new();
    tree.push(
        vec![Edit::delete(Position::new(0, 0), "removed")],
        Position::new(0, 7),
        Position::new(0, 0),
    );

    let serializable = from_undo_tree(&tree);
    assert_eq!(serializable.nodes.len(), 2);

    // Verify the delete edit was serialized
    assert!(matches!(&serializable.nodes[1].edits[0], SerializableEdit::Delete { .. }));

    // Convert back and verify
    let restored = to_undo_tree(&serializable);
    assert_eq!(restored.node_count(), 2);

    let node = restored.node(1).unwrap();
    let edit = &node.edits()[0];
    assert!(edit.is_delete());
    assert_eq!(edit.text(), "removed");
}

#[test]
fn test_undo_file_error_debug() {
    let err = UndoFileError::TooShort;
    let debug = format!("{err:?}");
    assert!(debug.contains("TooShort"));

    let err = UndoFileError::InvalidMagic;
    let debug = format!("{err:?}");
    assert!(debug.contains("InvalidMagic"));
}
