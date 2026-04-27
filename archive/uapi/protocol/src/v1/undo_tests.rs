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
fn test_undo_file_error_debug() {
    let err = UndoFileError::TooShort;
    let debug = format!("{err:?}");
    assert!(debug.contains("TooShort"));

    let err = UndoFileError::InvalidMagic;
    let debug = format!("{err:?}");
    assert!(debug.contains("InvalidMagic"));
}
