use {
    reovim_domain_text::{Edit, EditOrigin, Position, UndoTree},
    reovim_protocol::v1::undo::{
        SerializableEdit, SerializableEditOrigin, SerializablePosition, SerializableUndoNode,
        SerializableUndoTree, UndoFileFormat,
    },
};

use super::*;

#[test]
fn test_position_kernel_conversion() {
    let kernel_pos = Position::new(10, 20);
    let serializable = position_to_serializable(kernel_pos);
    assert_eq!(serializable.line, 10);
    assert_eq!(serializable.column, 20);

    let back = serializable_to_position(serializable);
    assert_eq!(back, kernel_pos);
}

#[test]
fn test_edit_origin_conversion() {
    // System origin
    let system = EditOrigin::System;
    let serializable = origin_to_serializable(system);
    assert_eq!(serializable, SerializableEditOrigin::System);
    let back = serializable_to_origin(serializable);
    assert_eq!(back, EditOrigin::System);

    // Client origin
    let client = EditOrigin::Client(42);
    let serializable = origin_to_serializable(client);
    assert_eq!(serializable, SerializableEditOrigin::Client(42));
    let back = serializable_to_origin(serializable);
    assert_eq!(back, EditOrigin::Client(42));
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
    let serializable = edit_to_serializable(&insert);
    assert!(matches!(
        &serializable,
        SerializableEdit::Insert { position, text }
        if position.line == 5 && position.column == 10 && text == "test"
    ));

    let back = serializable_to_edit(serializable);
    assert_eq!(back.position(), Position::new(5, 10));
    assert_eq!(back.text(), "test");
    assert!(back.is_insert());

    let delete = Edit::delete(Position::new(1, 0), "removed");
    let serializable = edit_to_serializable(&delete);
    let back = serializable_to_edit(serializable);
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
    for i in 1..tree.node_count() {
        for j in (i + 1)..tree.node_count() {
            let node_i = tree.node(i).unwrap();
            let node_j = tree.node(j).unwrap();

            let time_i = serializable.nodes[i].relative_time_secs;
            let time_j = serializable.nodes[j].relative_time_secs;

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
fn test_delete_edit_kernel_conversion() {
    let delete = SerializableEdit::Delete {
        position: SerializablePosition::new(3, 7),
        text: "deleted text".to_string(),
    };
    let edit = serializable_to_edit(delete);
    assert!(edit.is_delete());
    assert_eq!(edit.position(), Position::new(3, 7));
    assert_eq!(edit.text(), "deleted text");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_delete_edit_from_kernel() {
    let kernel_delete = Edit::delete(Position::new(2, 5), "removed");
    let serializable = edit_to_serializable(&kernel_delete);
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
