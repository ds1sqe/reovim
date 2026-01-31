//! Tests for block operations subsystem.

use {
    super::*,
    crate::mm::{Buffer, BufferId, Edit, Position},
};

// === Transaction Tests ===

mod transaction_tests {
    use super::*;

    #[test]
    fn test_new_transaction_is_empty() {
        let txn = Transaction::new();
        assert!(txn.is_empty());
        assert_eq!(txn.len(), 0);
    }

    #[test]
    fn test_push_and_edits() {
        let mut txn = Transaction::new();
        txn.push(Edit::insert(Position::new(0, 0), "Hello"));
        txn.push(Edit::insert(Position::new(0, 5), " World"));

        assert!(!txn.is_empty());
        assert_eq!(txn.len(), 2);

        let edits = txn.edits();
        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0].text(), "Hello");
        assert_eq!(edits[1].text(), " World");
    }

    #[test]
    fn test_inverse() {
        let mut txn = Transaction::new();
        txn.push(Edit::insert(Position::new(0, 0), "A"));
        txn.push(Edit::insert(Position::new(0, 1), "B"));
        txn.push(Edit::insert(Position::new(0, 2), "C"));

        let inverse = txn.inverse();

        // Inverse should be in reverse order
        let edits = inverse.edits();
        assert_eq!(edits.len(), 3);

        // Edits should be inverted (insert -> delete)
        assert!(edits[0].is_delete());
        assert!(edits[1].is_delete());
        assert!(edits[2].is_delete());

        // Order should be reversed: C, B, A
        assert_eq!(edits[0].text(), "C");
        assert_eq!(edits[1].text(), "B");
        assert_eq!(edits[2].text(), "A");
    }

    #[test]
    fn test_from_vec() {
        let edits = vec![
            Edit::insert(Position::new(0, 0), "X"),
            Edit::insert(Position::new(0, 1), "Y"),
        ];

        let txn: Transaction = edits.into();
        assert_eq!(txn.len(), 2);
    }

    #[test]
    fn test_from_single_edit() {
        let edit = Edit::insert(Position::new(0, 0), "Z");
        let txn: Transaction = edit.into();
        assert_eq!(txn.len(), 1);
    }

    #[test]
    fn test_into_iter() {
        let mut txn = Transaction::new();
        txn.push(Edit::insert(Position::new(0, 0), "A"));
        txn.push(Edit::insert(Position::new(0, 1), "B"));

        assert_eq!(txn.into_iter().count(), 2);
    }

    #[test]
    fn test_clear() {
        let mut txn = Transaction::new();
        txn.push(Edit::insert(Position::new(0, 0), "A"));
        assert!(!txn.is_empty());

        txn.clear();
        assert!(txn.is_empty());
    }
}

// === UndoTree Tests ===

mod undo_tree_tests {
    use super::*;

    #[test]
    fn test_new_tree_at_root() {
        let tree = UndoTree::new();
        assert!(!tree.can_undo());
        assert!(!tree.can_redo());
        assert_eq!(tree.node_count(), 1); // Root node
    }

    #[test]
    fn test_linear_undo_redo() {
        let mut tree = UndoTree::new();

        // Push first edit
        tree.push(
            vec![Edit::insert(Position::new(0, 0), "Hello")],
            Position::new(0, 0),
            Position::new(0, 5),
        );
        assert!(tree.can_undo());
        assert!(!tree.can_redo());

        // Push second edit
        tree.push(
            vec![Edit::insert(Position::new(0, 5), " World")],
            Position::new(0, 5),
            Position::new(0, 11),
        );

        // Undo second edit
        let result = tree.undo().expect("should be able to undo");
        assert_eq!(result.cursor, Position::new(0, 5));
        assert!(result.edits[0].is_delete());

        // Now can redo
        assert!(tree.can_undo());
        assert!(tree.can_redo());

        // Undo first edit
        let result = tree.undo().expect("should be able to undo");
        assert_eq!(result.cursor, Position::new(0, 0));

        // At root, can't undo more
        assert!(!tree.can_undo());
        assert!(tree.can_redo());

        // Redo first edit
        let result = tree.redo().expect("should be able to redo");
        assert_eq!(result.cursor, Position::new(0, 5));
        assert!(result.edits[0].is_insert());

        // Redo second edit
        let result = tree.redo().expect("should be able to redo");
        assert_eq!(result.cursor, Position::new(0, 11));
    }

    #[test]
    fn test_undo_restores_cursor() {
        let mut tree = UndoTree::new();

        tree.push(
            vec![Edit::insert(Position::new(5, 10), "text")],
            Position::new(5, 10), // cursor before
            Position::new(5, 14), // cursor after
        );

        let result = tree.undo().unwrap();
        assert_eq!(result.cursor, Position::new(5, 10));
    }

    #[test]
    fn test_redo_restores_cursor() {
        let mut tree = UndoTree::new();

        tree.push(
            vec![Edit::insert(Position::new(3, 5), "abc")],
            Position::new(3, 5),
            Position::new(3, 8),
        );

        tree.undo();
        let result = tree.redo().unwrap();
        assert_eq!(result.cursor, Position::new(3, 8));
    }

    #[test]
    fn test_branching_undo() {
        let mut tree = UndoTree::new();

        // Push edit A
        tree.push(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
        );

        // Push edit B
        tree.push(
            vec![Edit::insert(Position::new(0, 1), "B")],
            Position::new(0, 1),
            Position::new(0, 2),
        );

        // Undo B, now at A
        tree.undo();
        assert!(tree.can_redo());

        // Push edit C (creates new branch)
        tree.push(
            vec![Edit::insert(Position::new(0, 1), "C")],
            Position::new(0, 1),
            Position::new(0, 2),
        );

        // Current tree structure:
        // Root -> A -> B
        //           -> C (current)

        // Undo C, back to A
        tree.undo();

        // A should have 2 children (branches)
        let branches = tree.branches();
        assert_eq!(branches.len(), 2);
    }

    #[test]
    fn test_switch_branch() {
        let mut tree = UndoTree::new();

        // Create branching structure
        tree.push(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
        );

        tree.push(
            vec![Edit::insert(Position::new(0, 1), "B")],
            Position::new(0, 1),
            Position::new(0, 2),
        );

        tree.undo();

        tree.push(
            vec![Edit::insert(Position::new(0, 1), "C")],
            Position::new(0, 1),
            Position::new(0, 2),
        );

        tree.undo();

        // At A, branches are [B-index, C-index]
        // Default redo would go to active branch (C, since it was added last)
        let branches = tree.branches();
        assert_eq!(branches.len(), 2);

        // Switch to first branch (B)
        assert!(tree.switch_branch(0));

        // Redo should now go to B branch
        let result = tree.redo().unwrap();
        assert_eq!(result.edits[0].text(), "B");
    }

    #[test]
    fn test_max_nodes_pruning() {
        let mut tree = UndoTree::with_max_nodes(5);

        // Create branches that can be pruned
        // Push A
        tree.push(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
        );

        // Push B, C as branches from A
        tree.push(
            vec![Edit::insert(Position::new(0, 1), "B")],
            Position::new(0, 1),
            Position::new(0, 2),
        );
        tree.undo(); // back to A

        tree.push(
            vec![Edit::insert(Position::new(0, 1), "C")],
            Position::new(0, 1),
            Position::new(0, 2),
        );
        tree.undo(); // back to A

        // Push D, E, F, G from A (creates more branches)
        for c in ['D', 'E', 'F', 'G'] {
            tree.push(
                vec![Edit::insert(Position::new(0, 1), c.to_string())],
                Position::new(0, 1),
                Position::new(0, 2),
            );
            tree.undo(); // back to A
        }

        // Now push H and stay there (this is the protected path)
        tree.push(
            vec![Edit::insert(Position::new(0, 1), "H")],
            Position::new(0, 1),
            Position::new(0, 2),
        );

        // Tree structure: root -> A -> [B, C, D, E, F, G, H]
        // Protected path: root -> A -> H
        // Leaf nodes B, C, D, E, F, G can be pruned

        // Should be pruned to max_nodes (5)
        // Protected: root, A, H (3 nodes)
        // After pruning: at most 5 nodes
        assert!(tree.node_count() <= 5);

        // Current path should still be intact
        assert!(tree.can_undo());

        // Verify we're still at H
        let result = tree.undo().unwrap();
        assert_eq!(result.edits[0].text(), "H");
    }

    #[test]
    fn test_empty_edits_not_pushed() {
        let mut tree = UndoTree::new();

        tree.push(vec![], Position::new(0, 0), Position::new(0, 0));

        // Should still only have root
        assert_eq!(tree.node_count(), 1);
        assert!(!tree.can_undo());
    }

    #[test]
    fn test_clear() {
        let mut tree = UndoTree::new();

        tree.push(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
        );

        tree.push(
            vec![Edit::insert(Position::new(0, 1), "B")],
            Position::new(0, 1),
            Position::new(0, 2),
        );

        tree.clear();

        assert!(!tree.can_undo());
        assert!(!tree.can_redo());
        assert_eq!(tree.node_count(), 1);
    }

    // === Traversal Accessor Tests ===

    // --- node() accessor tests ---

    #[test]
    fn test_node_accessor_valid_index() {
        let mut tree = UndoTree::new();
        tree.push(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
        );

        assert!(tree.node(0).expect("root").is_root());
        assert!(!tree.node(1).expect("node 1").is_root());
    }

    #[test]
    fn test_node_accessor_boundary_cases() {
        let tree = UndoTree::new();

        // Root always accessible
        assert!(tree.node(0).is_some());

        // Just past end
        assert!(tree.node(tree.node_count()).is_none());

        // Way past end
        assert!(tree.node(usize::MAX).is_none());
    }

    #[test]
    fn test_node_accessor_after_modifications() {
        let mut tree = UndoTree::new();

        tree.push(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
        );
        tree.push(
            vec![Edit::insert(Position::new(0, 1), "B")],
            Position::new(0, 1),
            Position::new(0, 2),
        );

        // All nodes accessible
        assert!(tree.node(0).is_some());
        assert!(tree.node(1).is_some());
        assert!(tree.node(2).is_some());
        assert!(tree.node(3).is_none());

        // Verify relationships
        let node_a = tree.node(1).unwrap();
        assert_eq!(node_a.parent(), Some(0));
        assert_eq!(node_a.children(), &[2]);
    }

    // --- node_indices() tests ---

    #[test]
    fn test_node_indices_empty_tree() {
        let tree = UndoTree::new();
        let indices: Vec<_> = tree.node_indices().collect();
        assert_eq!(indices, vec![0]); // Only root
    }

    #[test]
    fn test_node_indices_with_branches() {
        let mut tree = UndoTree::new();
        tree.push(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
        );
        tree.push(
            vec![Edit::insert(Position::new(0, 1), "B")],
            Position::new(0, 1),
            Position::new(0, 2),
        );

        let indices: Vec<_> = tree.node_indices().collect();
        assert_eq!(indices, vec![0, 1, 2]);
    }

    #[test]
    fn test_node_indices_all_valid() {
        let mut tree = UndoTree::new();
        tree.push(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
        );
        tree.push(
            vec![Edit::insert(Position::new(0, 1), "B")],
            Position::new(0, 1),
            Position::new(0, 2),
        );
        tree.undo();
        tree.push(
            vec![Edit::insert(Position::new(0, 1), "C")],
            Position::new(0, 1),
            Position::new(0, 2),
        );

        // All indices from node_indices() should be valid
        for idx in tree.node_indices() {
            assert!(tree.node(idx).is_some(), "Index {idx} should be valid");
        }

        // Count matches node_count()
        assert_eq!(tree.node_indices().count(), tree.node_count());
    }

    // --- active_branch_at() tests ---

    #[test]
    fn test_active_branch_at_with_branches() {
        let mut tree = UndoTree::new();

        // Create branching: root -> A -> [B, C]
        tree.push(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
        );
        tree.push(
            vec![Edit::insert(Position::new(0, 1), "B")],
            Position::new(0, 1),
            Position::new(0, 2),
        );
        tree.undo();
        tree.push(
            vec![Edit::insert(Position::new(0, 1), "C")],
            Position::new(0, 1),
            Position::new(0, 2),
        );
        tree.undo();

        // Active branch at A should be 1 (C, the most recently created branch)
        let active = tree.active_branch_at(1);
        assert!(active.is_some());
        assert_eq!(active.unwrap(), 1);
    }

    #[test]
    fn test_active_branch_at_edge_cases() {
        let tree = UndoTree::new();

        // Invalid index returns None
        assert!(tree.active_branch_at(999).is_none());

        // Root has no active branch initially (no children)
        // active_branches[0] exists but node has no children
        let root = tree.node(0).unwrap();
        assert!(root.children().is_empty());
    }

    #[test]
    fn test_active_branch_after_switch() {
        let mut tree = UndoTree::new();

        // Create: root -> A -> [B, C]
        tree.push(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
        );
        tree.push(
            vec![Edit::insert(Position::new(0, 1), "B")],
            Position::new(0, 1),
            Position::new(0, 2),
        );
        tree.undo();
        tree.push(
            vec![Edit::insert(Position::new(0, 1), "C")],
            Position::new(0, 1),
            Position::new(0, 2),
        );
        tree.undo();

        // Switch to first branch (B)
        tree.switch_branch(0);
        assert_eq!(tree.active_branch_at(1), Some(0));

        // Switch to second branch (C)
        tree.switch_branch(1);
        assert_eq!(tree.active_branch_at(1), Some(1));
    }

    // --- parent() and children() tests ---

    #[test]
    fn test_undo_node_parent_accessor() {
        let mut tree = UndoTree::new();
        tree.push(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
        );

        // Root has no parent
        assert!(tree.node(0).unwrap().parent().is_none());

        // Node 1's parent is root
        assert_eq!(tree.node(1).unwrap().parent(), Some(0));
    }

    #[test]
    fn test_undo_node_children_accessor() {
        let mut tree = UndoTree::new();

        // Create: root -> A -> [B, C]
        tree.push(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
        );
        tree.push(
            vec![Edit::insert(Position::new(0, 1), "B")],
            Position::new(0, 1),
            Position::new(0, 2),
        );
        tree.undo();
        tree.push(
            vec![Edit::insert(Position::new(0, 1), "C")],
            Position::new(0, 1),
            Position::new(0, 2),
        );

        // Root has 1 child (A)
        assert_eq!(tree.node(0).unwrap().children(), &[1]);

        // A has 2 children (B, C)
        assert_eq!(tree.node(1).unwrap().children(), &[2, 3]);

        // Leaf nodes have empty children
        assert!(tree.node(2).unwrap().children().is_empty());
        assert!(tree.node(3).unwrap().children().is_empty());
    }

    // --- Invariant tests ---

    #[test]
    fn test_tree_structure_invariants() {
        let mut tree = UndoTree::new();

        // Build complex tree
        tree.push(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
        );
        tree.push(
            vec![Edit::insert(Position::new(0, 1), "B")],
            Position::new(0, 1),
            Position::new(0, 2),
        );
        tree.undo();
        tree.push(
            vec![Edit::insert(Position::new(0, 1), "C")],
            Position::new(0, 1),
            Position::new(0, 2),
        );

        // Invariant: parent-child relationships are bidirectional
        for index in tree.node_indices() {
            let node = tree.node(index).unwrap();

            // If has parent, parent's children includes this node
            if let Some(parent_idx) = node.parent() {
                let parent = tree.node(parent_idx).unwrap();
                assert!(
                    parent.children().contains(&index),
                    "Node {index} has parent {parent_idx}, but parent doesn't list it"
                );
            }

            // All children have this node as parent
            for &child_idx in node.children() {
                let child = tree.node(child_idx).unwrap();
                assert_eq!(
                    child.parent(),
                    Some(index),
                    "Node {index} lists {child_idx} as child, but child's parent doesn't match"
                );
            }
        }
    }

    #[test]
    fn test_accessors_after_pruning() {
        let mut tree = UndoTree::with_max_nodes(5);

        // Create more nodes than max, forcing pruning
        tree.push(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
        );

        // Create branches from A
        for c in ['B', 'C', 'D', 'E', 'F'] {
            tree.push(
                vec![Edit::insert(Position::new(0, 1), c.to_string())],
                Position::new(0, 1),
                Position::new(0, 2),
            );
            tree.undo();
        }

        // Stay on last branch
        tree.push(
            vec![Edit::insert(Position::new(0, 1), "G")],
            Position::new(0, 1),
            Position::new(0, 2),
        );

        // Should have pruned
        assert!(tree.node_count() <= 5);

        // All returned indices should be valid
        for index in tree.node_indices() {
            assert!(tree.node(index).is_some(), "node_indices() returned invalid index {index}");
        }

        // Current node always accessible
        let current_idx = tree.current_index();
        assert!(tree.node(current_idx).is_some());

        // Verify invariants still hold after pruning
        for index in tree.node_indices() {
            let node = tree.node(index).unwrap();
            if let Some(parent_idx) = node.parent() {
                assert!(tree.node(parent_idx).is_some());
            }
        }
    }
}

// === History Tests ===

mod history_tests {
    use super::*;

    #[test]
    fn test_new_history_is_empty() {
        let history = History::new();
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
    }

    #[test]
    fn test_record_and_entries() {
        let mut history = History::new();

        history.record(vec![Edit::insert(Position::new(0, 0), "A")]);
        history.record(vec![Edit::insert(Position::new(0, 1), "B")]);

        assert!(!history.is_empty());
        assert_eq!(history.len(), 2);

        let entries = history.entries();
        assert_eq!(entries[0].edits()[0].text(), "A");
        assert_eq!(entries[1].edits()[0].text(), "B");
    }

    #[test]
    fn test_max_entries() {
        let mut history = History::with_max_entries(3);

        for i in 0..5 {
            history.record(vec![Edit::insert(Position::new(0, 0), i.to_string())]);
        }

        // Should be capped at 3
        assert_eq!(history.len(), 3);

        // Oldest entries should be removed
        let entries = history.entries();
        assert_eq!(entries[0].edits()[0].text(), "2");
        assert_eq!(entries[1].edits()[0].text(), "3");
        assert_eq!(entries[2].edits()[0].text(), "4");
    }

    #[test]
    fn test_seq_num_increments() {
        let mut history = History::new();

        history.record(vec![Edit::insert(Position::new(0, 0), "A")]);
        history.record(vec![Edit::insert(Position::new(0, 0), "B")]);

        let entries = history.entries();
        assert_eq!(entries[0].seq_num(), 1);
        assert_eq!(entries[1].seq_num(), 2);
    }

    #[test]
    fn test_empty_edits_not_recorded() {
        let mut history = History::new();
        history.record(vec![]);
        assert!(history.is_empty());
    }

    #[test]
    fn test_clear() {
        let mut history = History::new();
        history.record(vec![Edit::insert(Position::new(0, 0), "A")]);
        assert!(!history.is_empty());

        history.clear();
        assert!(history.is_empty());

        // seq_counter should not reset
        history.record(vec![Edit::insert(Position::new(0, 0), "B")]);
        assert!(history.entries()[0].seq_num() > 1);
    }

    #[test]
    fn test_since() {
        let mut history = History::new();

        for i in 0..5 {
            history.record(vec![Edit::insert(Position::new(0, 0), i.to_string())]);
        }

        // Get entries since seq_num 3
        let since = history.since(3);
        assert_eq!(since.len(), 2);
        assert_eq!(since[0].seq_num(), 4);
        assert_eq!(since[1].seq_num(), 5);
    }

    #[test]
    fn test_last() {
        let mut history = History::new();
        assert!(history.last().is_none());

        history.record(vec![Edit::insert(Position::new(0, 0), "A")]);
        history.record(vec![Edit::insert(Position::new(0, 0), "B")]);

        assert_eq!(history.last().unwrap().edits()[0].text(), "B");
    }
}

// === Snapshot Tests ===

mod snapshot_tests {
    use super::*;

    #[test]
    fn test_capture_and_restore() {
        let mut buffer = Buffer::from_string("Hello\nWorld");
        buffer.set_position(Position::new(1, 3));

        let snapshot = Snapshot::capture(&buffer);

        // Modify buffer
        buffer.set_content("Something else");
        buffer.set_position(Position::new(0, 0));

        // Restore
        snapshot.restore(&mut buffer);

        assert_eq!(buffer.content(), "Hello\nWorld");
        assert_eq!(buffer.position(), Position::new(1, 3));
    }

    #[test]
    fn test_accessors() {
        let mut buffer = Buffer::from_string("Test content");
        buffer.set_position(Position::new(0, 5));

        let snapshot = Snapshot::capture(&buffer);

        assert_eq!(snapshot.lines(), &["Test content"]);
        assert_eq!(snapshot.cursor(), Position::new(0, 5));
        assert_eq!(snapshot.buffer_id(), buffer.id());
        assert!(snapshot.timestamp() <= std::time::SystemTime::now());
    }

    #[test]
    fn test_cursor_preserved() {
        let mut buffer = Buffer::from_string("Line 1\nLine 2\nLine 3");
        buffer.set_position(Position::new(2, 4));

        let snapshot = Snapshot::capture(&buffer);
        assert_eq!(snapshot.cursor(), Position::new(2, 4));

        // Modify and restore cursor only
        buffer.set_position(Position::new(0, 0));
        snapshot.restore_cursor(&mut buffer);

        assert_eq!(buffer.position(), Position::new(2, 4));
    }

    #[test]
    fn test_from_parts() {
        let lines = vec!["Line 1".to_string(), "Line 2".to_string()];
        let cursor = Position::new(1, 5);
        let buffer_id = BufferId::new();
        let timestamp = std::time::SystemTime::now();

        let snapshot = Snapshot::from_parts(lines.clone(), cursor, buffer_id, timestamp);

        assert_eq!(snapshot.lines(), &lines[..]);
        assert_eq!(snapshot.cursor(), cursor);
        assert_eq!(snapshot.buffer_id(), buffer_id);
    }

    #[test]
    fn test_matches_buffer() {
        let buffer = Buffer::from_string("Test");
        let snapshot = Snapshot::capture(&buffer);

        assert!(snapshot.matches_buffer(&buffer));

        let other_buffer = Buffer::from_string("Other");
        assert!(!snapshot.matches_buffer(&other_buffer));
    }

    #[test]
    fn test_char_count() {
        let buffer = Buffer::from_string("Hello\nWorld");
        let snapshot = Snapshot::capture(&buffer);

        // "Hello" (5) + "\n" (1) + "World" (5) = 11
        assert_eq!(snapshot.char_count(), 11);
    }

    #[test]
    fn test_line_count() {
        let buffer = Buffer::from_string("A\nB\nC");
        let snapshot = Snapshot::capture(&buffer);
        assert_eq!(snapshot.line_count(), 3);
    }

    #[test]
    fn test_empty_snapshot() {
        let buffer = Buffer::new();
        let snapshot = Snapshot::capture(&buffer);

        assert!(snapshot.is_empty());
        assert_eq!(snapshot.line_count(), 0);
        assert_eq!(snapshot.char_count(), 0);
    }
}
