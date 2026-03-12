//! Tests for block operations subsystem.

use {
    super::*,
    crate::mm::{Buffer, BufferId, Edit, Position},
};

mod undo;

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
    use {super::*, crate::block::EditOrigin};

    #[test]
    fn test_new_tree_at_root() {
        let tree = UndoTree::new();
        assert!(!tree.can_undo());
        assert!(!tree.can_redo());
        assert_eq!(tree.node_count(), 1); // Root node
    }

    // === EditOrigin Tests (#471) ===

    #[test]
    fn test_edit_origin_default() {
        assert_eq!(EditOrigin::default(), EditOrigin::System);
    }

    #[test]
    fn test_push_default_origin_is_system() {
        let mut tree = UndoTree::new();

        tree.push(
            vec![Edit::insert(Position::new(0, 0), "Hello")],
            Position::new(0, 0),
            Position::new(0, 5),
        );

        // Default push uses System origin
        let node = tree.node(1).expect("node should exist");
        assert_eq!(node.origin(), EditOrigin::System);
    }

    #[test]
    fn test_push_with_origin_tags_node() {
        let mut tree = UndoTree::new();

        // Push with client origin
        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 0), "Hello")],
            Position::new(0, 0),
            Position::new(0, 5),
            EditOrigin::Client(42),
        );

        let node = tree.node(1).expect("node should exist");
        assert_eq!(node.origin(), EditOrigin::Client(42));
    }

    #[test]
    fn test_multiple_clients_different_origins() {
        let mut tree = UndoTree::new();

        // Client 0 makes edit
        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
            EditOrigin::Client(0),
        );

        // Client 1 makes edit
        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 1), "B")],
            Position::new(0, 1),
            Position::new(0, 2),
            EditOrigin::Client(1),
        );

        // Client 0 makes another edit
        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 2), "C")],
            Position::new(0, 2),
            Position::new(0, 3),
            EditOrigin::Client(0),
        );

        // Verify origins
        assert_eq!(tree.node(1).unwrap().origin(), EditOrigin::Client(0));
        assert_eq!(tree.node(2).unwrap().origin(), EditOrigin::Client(1));
        assert_eq!(tree.node(3).unwrap().origin(), EditOrigin::Client(0));
    }

    #[test]
    fn test_root_node_has_system_origin() {
        let tree = UndoTree::new();
        let root = tree.node(0).expect("root should exist");
        assert_eq!(root.origin(), EditOrigin::System);
    }

    #[test]
    fn test_origin_preserved_through_undo_redo() {
        let mut tree = UndoTree::new();

        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 0), "X")],
            Position::new(0, 0),
            Position::new(0, 1),
            EditOrigin::Client(99),
        );

        // Undo and redo don't change origin
        tree.undo();
        tree.redo();

        let node = tree.node(1).unwrap();
        assert_eq!(node.origin(), EditOrigin::Client(99));
    }

    #[test]
    fn test_edit_origin_equality() {
        assert_eq!(EditOrigin::System, EditOrigin::System);
        assert_eq!(EditOrigin::Client(1), EditOrigin::Client(1));
        assert_ne!(EditOrigin::Client(1), EditOrigin::Client(2));
        assert_ne!(EditOrigin::System, EditOrigin::Client(0));
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

// === edits_since Tests (#495) ===

mod edits_since_tests {
    use {super::*, crate::block::EditOrigin};

    #[test]
    fn test_edits_since_same_as_current() {
        let mut tree = UndoTree::new();

        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 0), "AAA")],
            Position::new(0, 0),
            Position::new(0, 3),
            EditOrigin::Client(0),
        );

        // from_idx == current → empty vec
        let edits = tree.edits_since(tree.current_index()).unwrap();
        assert!(edits.is_empty());
    }

    #[test]
    fn test_edits_since_one_step() {
        let mut tree = UndoTree::new();

        // Node 1: Client A inserts "AAA"
        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 0), "AAA")],
            Position::new(0, 0),
            Position::new(0, 3),
            EditOrigin::Client(0),
        );

        // Node 2: Client B inserts "BBB"
        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 3), "BBB")],
            Position::new(0, 3),
            Position::new(0, 6),
            EditOrigin::Client(1),
        );

        // Edits since node 1 (current is node 2)
        let edits = tree.edits_since(1).unwrap();
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].text(), "BBB");
        assert!(edits[0].is_insert());
    }

    #[test]
    fn test_edits_since_multiple_steps() {
        let mut tree = UndoTree::new();

        // Node 1: Client A
        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 0), "A1")],
            Position::new(0, 0),
            Position::new(0, 2),
            EditOrigin::Client(0),
        );

        // Node 2: Client B
        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 2), "B1")],
            Position::new(0, 2),
            Position::new(0, 4),
            EditOrigin::Client(1),
        );

        // Node 3: Client C
        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 4), "C1")],
            Position::new(0, 4),
            Position::new(0, 6),
            EditOrigin::Client(2),
        );

        // Edits since node 1 → should include nodes 2 and 3 in order
        let edits = tree.edits_since(1).unwrap();
        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0].text(), "B1"); // forward order: B first
        assert_eq!(edits[1].text(), "C1"); // then C
    }

    #[test]
    fn test_edits_since_not_ancestor() {
        let mut tree = UndoTree::new();

        // Create branching: root → A → B
        //                          ↘ C (current)
        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
            EditOrigin::Client(0),
        );

        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 1), "B")],
            Position::new(0, 1),
            Position::new(0, 2),
            EditOrigin::Client(1),
        );

        // B is node 2, undo back to A
        tree.undo();

        // Push C (creates branch)
        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 1), "C")],
            Position::new(0, 1),
            Position::new(0, 2),
            EditOrigin::Client(2),
        );

        // Current is C (node 3). B (node 2) is NOT an ancestor of C.
        assert!(tree.edits_since(2).is_none());
    }

    #[test]
    fn test_edits_since_with_branches() {
        let mut tree = UndoTree::new();

        // root → A → B → C
        //           ↘ D (branch)
        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
            EditOrigin::Client(0),
        );

        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 1), "B")],
            Position::new(0, 1),
            Position::new(0, 2),
            EditOrigin::Client(1),
        );

        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 2), "C")],
            Position::new(0, 2),
            Position::new(0, 3),
            EditOrigin::Client(2),
        );

        // Now undo to A and create branch D
        tree.undo(); // at B
        tree.undo(); // at A

        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 1), "D")],
            Position::new(0, 1),
            Position::new(0, 2),
            EditOrigin::Client(3),
        );

        // Current is D. edits_since(A) should be just D's edits.
        // A is node 1.
        let edits = tree.edits_since(1).unwrap();
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].text(), "D");

        // edits_since(root) should be A + D in order
        let edits = tree.edits_since(0).unwrap();
        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0].text(), "A");
        assert_eq!(edits[1].text(), "D");
    }

    #[test]
    fn test_edits_since_out_of_bounds() {
        let tree = UndoTree::new();
        assert!(tree.edits_since(999).is_none());
    }

    #[test]
    fn test_edits_since_batched_node() {
        let mut tree = UndoTree::new();

        // Node 1: Client A
        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 0), "X")],
            Position::new(0, 0),
            Position::new(0, 1),
            EditOrigin::Client(0),
        );

        // Node 2: Client B with BATCHED edits (2 edits in one node)
        tree.push_with_origin(
            vec![
                Edit::insert(Position::new(0, 1), "AA"),
                Edit::insert(Position::new(0, 3), "BB"),
            ],
            Position::new(0, 1),
            Position::new(0, 5),
            EditOrigin::Client(1),
        );

        // Edits since node 1 should include BOTH of node 2's edits
        let edits = tree.edits_since(1).unwrap();
        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0].text(), "AA");
        assert_eq!(edits[1].text(), "BB");
    }

    #[test]
    fn test_edits_since_skips_empty() {
        let mut tree = UndoTree::new();

        // Node 1
        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
            EditOrigin::Client(0),
        );

        // Node 2 with an empty edit mixed in
        tree.push_with_origin(
            vec![
                Edit::insert(Position::new(0, 1), ""),
                Edit::insert(Position::new(0, 1), "B"),
            ],
            Position::new(0, 1),
            Position::new(0, 2),
            EditOrigin::Client(1),
        );

        let edits = tree.edits_since(1).unwrap();
        assert_eq!(edits.len(), 1); // empty edit skipped
        assert_eq!(edits[0].text(), "B");
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

    // Note: Tests updated for #471 - cursor is now per-window, not per-buffer.
    // Snapshot::capture takes cursor position explicitly.

    #[test]
    fn test_capture_and_restore() {
        let mut buffer = Buffer::from_string("Hello\nWorld");
        let cursor = Position::new(1, 3);

        let snapshot = Snapshot::capture(&buffer, cursor);

        // Modify buffer
        buffer.set_content("Something else");

        // Restore returns cursor position
        let restored_cursor = snapshot.restore(&mut buffer);

        assert_eq!(buffer.content(), "Hello\nWorld");
        assert_eq!(restored_cursor, Position::new(1, 3));
    }

    #[test]
    fn test_accessors() {
        let buffer = Buffer::from_string("Test content");
        let cursor = Position::new(0, 5);

        let snapshot = Snapshot::capture(&buffer, cursor);

        assert_eq!(snapshot.lines(), &["Test content"]);
        assert_eq!(snapshot.cursor(), Position::new(0, 5));
        assert_eq!(snapshot.buffer_id(), buffer.id());
        assert!(snapshot.timestamp() <= std::time::SystemTime::now());
    }

    #[test]
    fn test_cursor_preserved() {
        let buffer = Buffer::from_string("Line 1\nLine 2\nLine 3");
        let cursor = Position::new(2, 4);

        let snapshot = Snapshot::capture(&buffer, cursor);
        assert_eq!(snapshot.cursor(), Position::new(2, 4));
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
        let snapshot = Snapshot::capture(&buffer, Position::origin());

        assert!(snapshot.matches_buffer(&buffer));

        let other_buffer = Buffer::from_string("Other");
        assert!(!snapshot.matches_buffer(&other_buffer));
    }

    #[test]
    fn test_char_count() {
        let buffer = Buffer::from_string("Hello\nWorld");
        let snapshot = Snapshot::capture(&buffer, Position::origin());

        // "Hello" (5) + "\n" (1) + "World" (5) = 11
        assert_eq!(snapshot.char_count(), 11);
    }

    #[test]
    fn test_line_count() {
        let buffer = Buffer::from_string("A\nB\nC");
        let snapshot = Snapshot::capture(&buffer, Position::origin());
        assert_eq!(snapshot.line_count(), 3);
    }

    #[test]
    fn test_empty_snapshot() {
        let buffer = Buffer::new();
        let snapshot = Snapshot::capture(&buffer, Position::origin());

        assert!(snapshot.is_empty());
        assert_eq!(snapshot.line_count(), 0);
        assert_eq!(snapshot.char_count(), 0);
    }

    #[test]
    fn test_snapshot_clone() {
        let buffer = Buffer::from_string("Hello\nWorld");
        let cursor = Position::new(1, 3);
        let snapshot = Snapshot::capture(&buffer, cursor);

        let cloned = snapshot.clone();
        assert_eq!(cloned.lines(), snapshot.lines());
        assert_eq!(cloned.cursor(), snapshot.cursor());
        assert_eq!(cloned.buffer_id(), snapshot.buffer_id());
        assert_eq!(cloned.line_count(), snapshot.line_count());
        assert_eq!(cloned.char_count(), snapshot.char_count());
    }

    #[test]
    fn test_snapshot_single_line_char_count() {
        let buffer = Buffer::from_string("Hello");
        let snapshot = Snapshot::capture(&buffer, Position::origin());

        // "Hello" = 5 chars, 1 line, 0 newlines
        assert_eq!(snapshot.char_count(), 5);
    }

    #[test]
    fn test_snapshot_three_lines_char_count() {
        let buffer = Buffer::from_string("AB\nCD\nEF");
        let snapshot = Snapshot::capture(&buffer, Position::origin());

        // "AB" (2) + "CD" (2) + "EF" (2) = 6 chars + 2 newlines = 8
        assert_eq!(snapshot.char_count(), 8);
        assert_eq!(snapshot.line_count(), 3);
    }

    #[test]
    fn test_snapshot_non_empty() {
        let buffer = Buffer::from_string("x");
        let snapshot = Snapshot::capture(&buffer, Position::origin());

        assert!(!snapshot.is_empty());
    }

    #[test]
    fn test_snapshot_restore_multiline() {
        let mut buffer = Buffer::from_string("Original\nContent");
        let cursor = Position::new(0, 3);
        let snapshot = Snapshot::capture(&buffer, cursor);

        buffer.set_content("Modified completely");

        let restored_cursor = snapshot.restore(&mut buffer);
        assert_eq!(buffer.content(), "Original\nContent");
        assert_eq!(restored_cursor, Position::new(0, 3));
    }

    #[test]
    fn test_from_parts_accessors() {
        let lines = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let cursor = Position::new(2, 1);
        let buffer_id = BufferId::new();
        let timestamp = std::time::SystemTime::now();

        let snapshot = Snapshot::from_parts(lines, cursor, buffer_id, timestamp);

        assert_eq!(snapshot.lines(), &["A", "B", "C"]);
        assert_eq!(snapshot.cursor(), Position::new(2, 1));
        assert_eq!(snapshot.buffer_id(), buffer_id);
        assert_eq!(snapshot.timestamp(), timestamp);
        assert_eq!(snapshot.line_count(), 3);
        assert!(!snapshot.is_empty());
    }
}

// === Additional Transaction Tests ===

mod transaction_extended_tests {
    use super::*;

    #[test]
    fn test_default_is_empty() {
        let txn = Transaction::default();
        assert!(txn.is_empty());
        assert_eq!(txn.len(), 0);
    }

    #[test]
    fn test_with_capacity() {
        let txn = Transaction::with_capacity(10);
        assert!(txn.is_empty());
        assert_eq!(txn.len(), 0);
    }

    #[test]
    fn test_with_capacity_push() {
        let mut txn = Transaction::with_capacity(2);
        txn.push(Edit::insert(Position::new(0, 0), "A"));
        txn.push(Edit::insert(Position::new(0, 1), "B"));
        assert_eq!(txn.len(), 2);
    }

    #[test]
    fn test_into_edits() {
        let mut txn = Transaction::new();
        txn.push(Edit::insert(Position::new(0, 0), "Hello"));
        txn.push(Edit::insert(Position::new(0, 5), " World"));

        let edits = txn.into_edits();
        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0].text(), "Hello");
        assert_eq!(edits[1].text(), " World");
    }

    #[test]
    fn test_iter() {
        let mut txn = Transaction::new();
        txn.push(Edit::insert(Position::new(0, 0), "A"));
        txn.push(Edit::insert(Position::new(0, 1), "B"));
        txn.push(Edit::insert(Position::new(0, 2), "C"));

        let texts: Vec<&str> = txn.iter().map(Edit::text).collect();
        assert_eq!(texts, vec!["A", "B", "C"]);
    }

    #[test]
    fn test_ref_into_iter() {
        let mut txn = Transaction::new();
        txn.push(Edit::insert(Position::new(0, 0), "X"));
        txn.push(Edit::insert(Position::new(0, 1), "Y"));

        let mut count = 0;
        for edit in &txn {
            assert!(edit.is_insert());
            count += 1;
        }
        assert_eq!(count, 2);
    }

    #[test]
    fn test_inverse_of_empty() {
        let txn = Transaction::new();
        let inverse = txn.inverse();
        assert!(inverse.is_empty());
    }

    #[test]
    fn test_inverse_of_deletes() {
        let mut txn = Transaction::new();
        txn.push(Edit::delete(Position::new(0, 0), "A"));
        txn.push(Edit::delete(Position::new(0, 0), "B"));

        let inverse = txn.inverse();
        let edits = inverse.edits();

        // Inverse of delete is insert, in reverse order
        assert_eq!(edits.len(), 2);
        assert!(edits[0].is_insert());
        assert!(edits[1].is_insert());
        assert_eq!(edits[0].text(), "B");
        assert_eq!(edits[1].text(), "A");
    }

    #[test]
    fn test_from_single_edit_content() {
        let edit = Edit::delete(Position::new(1, 2), "removed");
        let txn: Transaction = edit.into();
        assert_eq!(txn.len(), 1);
        assert!(txn.edits()[0].is_delete());
        assert_eq!(txn.edits()[0].text(), "removed");
    }

    #[test]
    fn test_clear_then_push() {
        let mut txn = Transaction::new();
        txn.push(Edit::insert(Position::new(0, 0), "A"));
        txn.clear();
        assert!(txn.is_empty());

        txn.push(Edit::insert(Position::new(0, 0), "B"));
        assert_eq!(txn.len(), 1);
        assert_eq!(txn.edits()[0].text(), "B");
    }

    #[test]
    fn test_into_iter_consumes() {
        let txn: Transaction = vec![
            Edit::insert(Position::new(0, 0), "A"),
            Edit::insert(Position::new(0, 1), "B"),
            Edit::insert(Position::new(0, 2), "C"),
        ]
        .into();

        let collected: Vec<Edit> = txn.into_iter().collect();
        assert_eq!(collected.len(), 3);
        assert_eq!(collected[0].text(), "A");
        assert_eq!(collected[1].text(), "B");
        assert_eq!(collected[2].text(), "C");
    }
}

// === Additional UndoTree Tests ===

mod undo_tree_extended_tests {
    use {super::*, crate::block::EditOrigin, std::time::Duration};

    #[test]
    fn test_default_is_new() {
        let tree = UndoTree::default();
        assert!(!tree.can_undo());
        assert!(!tree.can_redo());
        assert_eq!(tree.node_count(), 1);
    }

    #[test]
    fn test_current_node_is_root_initially() {
        let tree = UndoTree::new();
        let node = tree.current_node();
        assert!(node.is_root());
        assert!(node.edits().is_empty());
        assert!(node.children().is_empty());
    }

    #[test]
    fn test_current_node_after_push() {
        let mut tree = UndoTree::new();
        tree.push(
            vec![Edit::insert(Position::new(0, 0), "Hello")],
            Position::new(0, 0),
            Position::new(0, 5),
        );

        let node = tree.current_node();
        assert!(!node.is_root());
        assert_eq!(node.edits()[0].text(), "Hello");
        assert_eq!(node.cursor_before(), Position::new(0, 0));
        assert_eq!(node.cursor_after(), Position::new(0, 5));
    }

    #[test]
    fn test_current_index() {
        let mut tree = UndoTree::new();
        assert_eq!(tree.current_index(), 0);

        tree.push(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
        );
        assert_eq!(tree.current_index(), 1);

        tree.push(
            vec![Edit::insert(Position::new(0, 1), "B")],
            Position::new(0, 1),
            Position::new(0, 2),
        );
        assert_eq!(tree.current_index(), 2);

        tree.undo();
        assert_eq!(tree.current_index(), 1);
    }

    #[test]
    fn test_max_nodes_accessor() {
        let tree = UndoTree::new();
        assert_eq!(tree.max_nodes(), UndoTree::DEFAULT_MAX_NODES);

        let tree = UndoTree::with_max_nodes(42);
        assert_eq!(tree.max_nodes(), 42);
    }

    #[test]
    fn test_with_max_nodes_minimum_one() {
        let tree = UndoTree::with_max_nodes(0);
        assert_eq!(tree.max_nodes(), 1);
    }

    #[test]
    fn test_set_max_nodes() {
        let mut tree = UndoTree::new();
        tree.set_max_nodes(50);
        assert_eq!(tree.max_nodes(), 50);
    }

    #[test]
    fn test_set_max_nodes_minimum_one() {
        let mut tree = UndoTree::new();
        tree.set_max_nodes(0);
        assert_eq!(tree.max_nodes(), 1);
    }

    #[test]
    fn test_set_max_nodes_triggers_pruning() {
        let mut tree = UndoTree::new();

        // Create branching tree: root -> A -> [B, C, D]
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
        tree.push(
            vec![Edit::insert(Position::new(0, 1), "D")],
            Position::new(0, 1),
            Position::new(0, 2),
        );

        // Current path: root -> A -> D (3 nodes protected)
        // B and C are leaf nodes that can be pruned
        assert!(tree.node_count() > 3);

        tree.set_max_nodes(3);
        assert!(tree.node_count() <= 3);

        // Protected path should still be intact
        assert!(tree.can_undo());
    }

    #[test]
    fn test_seq_counter() {
        let mut tree = UndoTree::new();
        assert_eq!(tree.seq_counter(), 0);

        tree.push(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
        );
        assert_eq!(tree.seq_counter(), 1);

        tree.push(
            vec![Edit::insert(Position::new(0, 1), "B")],
            Position::new(0, 1),
            Position::new(0, 2),
        );
        assert_eq!(tree.seq_counter(), 2);

        // Undo/redo should not change seq_counter
        tree.undo();
        assert_eq!(tree.seq_counter(), 2);

        tree.redo();
        assert_eq!(tree.seq_counter(), 2);
    }

    #[test]
    fn test_seq_counter_increments_on_empty_edit_skip() {
        let mut tree = UndoTree::new();
        tree.push(vec![], Position::new(0, 0), Position::new(0, 0));
        // Empty edits are skipped, so seq_counter should not increment
        assert_eq!(tree.seq_counter(), 0);
    }

    #[test]
    fn test_undo_node_timestamp() {
        let mut tree = UndoTree::new();
        let before = std::time::Instant::now();

        tree.push(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
        );

        let after = std::time::Instant::now();
        let node = tree.node(1).unwrap();
        let ts = node.timestamp();

        assert!(ts >= before);
        assert!(ts <= after);
    }

    #[test]
    fn test_undo_node_seq_num() {
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

        assert_eq!(tree.node(0).unwrap().seq_num(), 0); // root
        assert_eq!(tree.node(1).unwrap().seq_num(), 1);
        assert_eq!(tree.node(2).unwrap().seq_num(), 2);
    }

    #[test]
    fn test_undo_node_branch_count() {
        let mut tree = UndoTree::new();

        tree.push(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
        );

        // A has no children yet
        assert_eq!(tree.node(1).unwrap().branch_count(), 0);

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

        // A now has 2 children (B and C)
        assert_eq!(tree.node(1).unwrap().branch_count(), 2);
    }

    #[test]
    fn test_redo_branch_valid() {
        let mut tree = UndoTree::new();

        // root -> A -> [B, C]
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

        // Redo branch 0 (B)
        let result = tree.redo_branch(0).unwrap();
        assert_eq!(result.edits[0].text(), "B");
        assert_eq!(result.cursor, Position::new(0, 2));
    }

    #[test]
    fn test_redo_branch_invalid() {
        let mut tree = UndoTree::new();

        tree.push(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
        );

        // A has no children (we're at A), so redo_branch(0) is invalid
        assert!(tree.redo_branch(0).is_none());
    }

    #[test]
    fn test_redo_branch_out_of_range() {
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

        // A has 1 child (B), so branch_idx=1 is out of range
        assert!(tree.redo_branch(1).is_none());
    }

    #[test]
    fn test_switch_branch_invalid() {
        let mut tree = UndoTree::new();
        // Root has no children, so switching branch is invalid
        assert!(!tree.switch_branch(0));
    }

    #[test]
    fn test_undo_at_root_returns_none() {
        let mut tree = UndoTree::new();
        assert!(tree.undo().is_none());
    }

    #[test]
    fn test_redo_at_leaf_returns_none() {
        let mut tree = UndoTree::new();
        tree.push(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
        );
        // At leaf node, redo returns None
        assert!(tree.redo().is_none());
    }

    #[test]
    fn test_clear_preserves_cursor_after() {
        let mut tree = UndoTree::new();

        tree.push(
            vec![Edit::insert(Position::new(0, 0), "Hello")],
            Position::new(0, 0),
            Position::new(0, 5),
        );

        // After clear, root should preserve cursor_after from original root
        // (which was default Position)
        tree.clear();
        let root = tree.node(0).unwrap();
        assert_eq!(root.cursor_after(), Position::default());
    }

    #[test]
    fn test_clear_preserves_max_nodes() {
        let mut tree = UndoTree::with_max_nodes(42);
        tree.push(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
        );
        tree.clear();
        assert_eq!(tree.max_nodes(), 42);
    }

    #[test]
    fn test_undo_result_edits_are_inverted() {
        let mut tree = UndoTree::new();
        tree.push(
            vec![
                Edit::insert(Position::new(0, 0), "A"),
                Edit::insert(Position::new(0, 1), "B"),
            ],
            Position::new(0, 0),
            Position::new(0, 2),
        );

        let result = tree.undo().unwrap();
        // Undo reverses edits and inverts them
        assert_eq!(result.edits.len(), 2);
        assert!(result.edits[0].is_delete());
        assert!(result.edits[1].is_delete());
        // Order is reversed
        assert_eq!(result.edits[0].text(), "B");
        assert_eq!(result.edits[1].text(), "A");
    }

    #[test]
    fn test_redo_result_edits_are_originals() {
        let mut tree = UndoTree::new();
        tree.push(
            vec![Edit::insert(Position::new(0, 0), "Hello")],
            Position::new(0, 0),
            Position::new(0, 5),
        );
        tree.undo();

        let result = tree.redo().unwrap();
        assert_eq!(result.edits.len(), 1);
        assert!(result.edits[0].is_insert());
        assert_eq!(result.edits[0].text(), "Hello");
    }

    #[test]
    fn test_from_serializable_basic() {
        let nodes_data = vec![
            // Root node
            (
                Vec::new(),             // edits
                Position::new(0, 0),    // cursor_before
                Position::new(0, 0),    // cursor_after
                Duration::from_secs(0), // relative_time
                None,                   // parent
                vec![1],                // children
                0u64,                   // seq_num
                EditOrigin::System,     // origin
            ),
            // Child node
            (
                vec![Edit::insert(Position::new(0, 0), "A")],
                Position::new(0, 0),
                Position::new(0, 1),
                Duration::from_secs(1),
                Some(0),
                Vec::new(),
                1u64,
                EditOrigin::Client(5),
            ),
        ];

        let tree = UndoTree::from_serializable(
            nodes_data,
            1,   // current at child
            1,   // seq_counter
            100, // max_nodes
            vec![0, 0],
        );

        assert_eq!(tree.node_count(), 2);
        assert_eq!(tree.current_index(), 1);
        assert_eq!(tree.seq_counter(), 1);
        assert_eq!(tree.max_nodes(), 100);
        assert!(tree.can_undo());
        assert!(!tree.can_redo());

        let root = tree.node(0).unwrap();
        assert!(root.is_root());
        assert_eq!(root.children(), &[1]);

        let child = tree.node(1).unwrap();
        assert_eq!(child.parent(), Some(0));
        assert_eq!(child.origin(), EditOrigin::Client(5));
        assert_eq!(child.edits()[0].text(), "A");
    }

    #[test]
    fn test_from_serializable_min_max_nodes() {
        let nodes_data = vec![(
            Vec::new(),
            Position::new(0, 0),
            Position::new(0, 0),
            Duration::from_secs(0),
            None,
            Vec::new(),
            0u64,
            EditOrigin::System,
        )];

        let tree = UndoTree::from_serializable(
            nodes_data,
            0,
            0,
            0, // should be clamped to 1
            vec![0],
        );

        assert_eq!(tree.max_nodes(), 1);
    }

    #[test]
    fn test_from_serializable_timestamps_ordered() {
        let nodes_data = vec![
            (
                Vec::new(),
                Position::new(0, 0),
                Position::new(0, 0),
                Duration::from_secs(0),
                None,
                vec![1],
                0u64,
                EditOrigin::System,
            ),
            (
                vec![Edit::insert(Position::new(0, 0), "A")],
                Position::new(0, 0),
                Position::new(0, 1),
                Duration::from_secs(5),
                Some(0),
                Vec::new(),
                1u64,
                EditOrigin::System,
            ),
        ];

        let tree = UndoTree::from_serializable(nodes_data, 1, 1, 100, vec![0, 0]);

        let root_ts = tree.node(0).unwrap().timestamp();
        let child_ts = tree.node(1).unwrap().timestamp();
        // Child has later relative_time, so its timestamp should be >= root's
        assert!(child_ts >= root_ts);
    }

    #[test]
    #[should_panic(expected = "UndoTree must have at least a root node")]
    fn test_from_serializable_empty_panics() {
        let _ = UndoTree::from_serializable(Vec::new(), 0, 0, 100, Vec::new());
    }

    #[test]
    fn test_from_serializable_branching() {
        let nodes_data = vec![
            // Root
            (
                Vec::new(),
                Position::new(0, 0),
                Position::new(0, 0),
                Duration::from_secs(0),
                None,
                vec![1],
                0u64,
                EditOrigin::System,
            ),
            // A
            (
                vec![Edit::insert(Position::new(0, 0), "A")],
                Position::new(0, 0),
                Position::new(0, 1),
                Duration::from_secs(1),
                Some(0),
                vec![2, 3],
                1u64,
                EditOrigin::Client(0),
            ),
            // B (branch 0 of A)
            (
                vec![Edit::insert(Position::new(0, 1), "B")],
                Position::new(0, 1),
                Position::new(0, 2),
                Duration::from_secs(2),
                Some(1),
                Vec::new(),
                2u64,
                EditOrigin::Client(0),
            ),
            // C (branch 1 of A)
            (
                vec![Edit::insert(Position::new(0, 1), "C")],
                Position::new(0, 1),
                Position::new(0, 2),
                Duration::from_secs(3),
                Some(1),
                Vec::new(),
                3u64,
                EditOrigin::Client(1),
            ),
        ];

        let tree = UndoTree::from_serializable(
            nodes_data,
            3, // current at C
            3,
            100,
            vec![0, 1, 0, 0],
        );

        assert_eq!(tree.node_count(), 4);
        assert_eq!(tree.current_index(), 3);

        let node_a = tree.node(1).unwrap();
        assert_eq!(node_a.branch_count(), 2);
        assert_eq!(node_a.children(), &[2, 3]);

        // Active branch at A should be 1 (C)
        assert_eq!(tree.active_branch_at(1), Some(1));
    }

    #[test]
    fn test_multiple_undo_redo_cycle() {
        let mut tree = UndoTree::new();

        for i in 0..5 {
            tree.push(
                vec![Edit::insert(Position::new(0, i), i.to_string())],
                Position::new(0, i),
                Position::new(0, i + 1),
            );
        }

        assert_eq!(tree.node_count(), 6); // root + 5 nodes

        // Undo all 5
        for _ in 0..5 {
            assert!(tree.undo().is_some());
        }
        assert!(!tree.can_undo());
        assert_eq!(tree.current_index(), 0);

        // Redo all 5
        for _ in 0..5 {
            assert!(tree.redo().is_some());
        }
        assert!(!tree.can_redo());
    }

    #[test]
    fn test_branches_empty_at_leaf() {
        let mut tree = UndoTree::new();
        tree.push(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
        );

        assert!(tree.branches().is_empty());
    }

    #[test]
    fn test_branches_after_multiple_pushes() {
        let mut tree = UndoTree::new();

        tree.push(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
        );

        // Create 3 branches from A
        for text in ["B", "C", "D"] {
            tree.push(
                vec![Edit::insert(Position::new(0, 1), text)],
                Position::new(0, 1),
                Position::new(0, 2),
            );
            tree.undo();
        }

        let branches = tree.branches();
        assert_eq!(branches.len(), 3);
    }

    #[test]
    fn test_push_with_origin_empty_edits_skipped() {
        let mut tree = UndoTree::new();
        tree.push_with_origin(
            vec![],
            Position::new(0, 0),
            Position::new(0, 0),
            EditOrigin::Client(1),
        );
        assert_eq!(tree.node_count(), 1); // Only root
    }

    #[test]
    fn test_edit_origin_copy_and_hash() {
        use std::collections::HashSet;

        let origin1 = EditOrigin::Client(1);
        let origin2 = origin1; // Copy
        assert_eq!(origin1, origin2);

        let mut set = HashSet::new();
        set.insert(EditOrigin::System);
        set.insert(EditOrigin::Client(1));
        set.insert(EditOrigin::Client(1)); // duplicate
        set.insert(EditOrigin::Client(2));

        assert_eq!(set.len(), 3);
    }

    #[test]
    fn test_undo_node_clone() {
        let mut tree = UndoTree::new();
        tree.push(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
        );

        // UndoTree implements Clone
        let cloned = tree.clone();
        assert_eq!(cloned.node_count(), tree.node_count());
        assert_eq!(cloned.current_index(), tree.current_index());
        assert_eq!(cloned.seq_counter(), tree.seq_counter());
    }
}

// === Additional History Tests ===

mod history_extended_tests {
    use super::*;

    #[test]
    fn test_default_is_new() {
        let history = History::default();
        assert!(history.is_empty());
        assert_eq!(history.max_entries(), History::DEFAULT_MAX_ENTRIES);
    }

    #[test]
    fn test_max_entries_accessor() {
        let history = History::new();
        assert_eq!(history.max_entries(), History::DEFAULT_MAX_ENTRIES);

        let history = History::with_max_entries(42);
        assert_eq!(history.max_entries(), 42);
    }

    #[test]
    fn test_with_max_entries_minimum_one() {
        let history = History::with_max_entries(0);
        assert_eq!(history.max_entries(), 1);
    }

    #[test]
    fn test_get_accessor() {
        let mut history = History::new();
        assert!(history.get(0).is_none());

        history.record(vec![Edit::insert(Position::new(0, 0), "A")]);
        history.record(vec![Edit::insert(Position::new(0, 1), "B")]);

        assert_eq!(history.get(0).unwrap().edits()[0].text(), "A");
        assert_eq!(history.get(1).unwrap().edits()[0].text(), "B");
        assert!(history.get(2).is_none());
    }

    #[test]
    fn test_set_max_entries() {
        let mut history = History::new();

        for i in 0..10 {
            history.record(vec![Edit::insert(Position::new(0, 0), i.to_string())]);
        }
        assert_eq!(history.len(), 10);

        // Shrink max - should trim old entries
        history.set_max_entries(3);
        assert_eq!(history.max_entries(), 3);
        assert_eq!(history.len(), 3);

        // Verify oldest were removed
        assert_eq!(history.entries()[0].edits()[0].text(), "7");
        assert_eq!(history.entries()[1].edits()[0].text(), "8");
        assert_eq!(history.entries()[2].edits()[0].text(), "9");
    }

    #[test]
    fn test_set_max_entries_minimum_one() {
        let mut history = History::new();
        history.set_max_entries(0);
        assert_eq!(history.max_entries(), 1);
    }

    #[test]
    fn test_set_max_entries_grow() {
        let mut history = History::with_max_entries(3);

        for i in 0..5 {
            history.record(vec![Edit::insert(Position::new(0, 0), i.to_string())]);
        }
        assert_eq!(history.len(), 3);

        history.set_max_entries(100);
        assert_eq!(history.max_entries(), 100);
        assert_eq!(history.len(), 3); // existing entries preserved
    }

    #[test]
    fn test_seq_counter() {
        let mut history = History::new();
        assert_eq!(history.seq_counter(), 0);

        history.record(vec![Edit::insert(Position::new(0, 0), "A")]);
        assert_eq!(history.seq_counter(), 1);

        history.record(vec![Edit::insert(Position::new(0, 0), "B")]);
        assert_eq!(history.seq_counter(), 2);
    }

    #[test]
    fn test_seq_counter_not_reset_on_clear() {
        let mut history = History::new();
        history.record(vec![Edit::insert(Position::new(0, 0), "A")]);
        history.record(vec![Edit::insert(Position::new(0, 0), "B")]);
        assert_eq!(history.seq_counter(), 2);

        history.clear();
        assert_eq!(history.seq_counter(), 2); // preserved

        history.record(vec![Edit::insert(Position::new(0, 0), "C")]);
        assert_eq!(history.seq_counter(), 3);
    }

    #[test]
    fn test_seq_counter_not_incremented_for_empty() {
        let mut history = History::new();
        history.record(vec![]); // empty, should not record
        assert_eq!(history.seq_counter(), 0);
    }

    #[test]
    fn test_entry_timestamp() {
        let mut history = History::new();
        let before = std::time::Instant::now();

        history.record(vec![Edit::insert(Position::new(0, 0), "A")]);

        let after = std::time::Instant::now();
        let entry = history.get(0).unwrap();
        let ts = entry.timestamp();

        assert!(ts >= before);
        assert!(ts <= after);
    }

    #[test]
    fn test_between() {
        let mut history = History::new();

        history.record(vec![Edit::insert(Position::new(0, 0), "A")]);
        let start = std::time::Instant::now();
        history.record(vec![Edit::insert(Position::new(0, 0), "B")]);
        history.record(vec![Edit::insert(Position::new(0, 0), "C")]);
        let end = std::time::Instant::now();

        let entries = history.between(start, end);
        // B and C should be within the range (they were recorded after start and before end)
        assert!(entries.len() >= 2);
        // All returned entries should have timestamps in range
        for entry in &entries {
            assert!(entry.timestamp() >= start);
            assert!(entry.timestamp() <= end);
        }
    }

    #[test]
    fn test_between_empty_range() {
        let mut history = History::new();
        history.record(vec![Edit::insert(Position::new(0, 0), "A")]);

        // Use a time range in the far past
        let start = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(100))
            .unwrap();
        let end = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(99))
            .unwrap();

        let entries = history.between(start, end);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_since_empty_history() {
        let history = History::new();
        let entries = history.since(0);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_since_all() {
        let mut history = History::new();
        for i in 0..3 {
            history.record(vec![Edit::insert(Position::new(0, 0), i.to_string())]);
        }

        let entries = history.since(0);
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_since_none() {
        let mut history = History::new();
        for i in 0..3 {
            history.record(vec![Edit::insert(Position::new(0, 0), i.to_string())]);
        }

        let entries = history.since(100);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_last_on_empty() {
        let history = History::new();
        assert!(history.last().is_none());
    }

    #[test]
    fn test_record_multiple_edits_per_entry() {
        let mut history = History::new();
        history.record(vec![
            Edit::insert(Position::new(0, 0), "A"),
            Edit::insert(Position::new(0, 1), "B"),
            Edit::insert(Position::new(0, 2), "C"),
        ]);

        assert_eq!(history.len(), 1);
        let entry = history.get(0).unwrap();
        assert_eq!(entry.edits().len(), 3);
        assert_eq!(entry.edits()[0].text(), "A");
        assert_eq!(entry.edits()[1].text(), "B");
        assert_eq!(entry.edits()[2].text(), "C");
    }

    #[test]
    fn test_max_entries_one() {
        let mut history = History::with_max_entries(1);

        history.record(vec![Edit::insert(Position::new(0, 0), "A")]);
        assert_eq!(history.len(), 1);

        history.record(vec![Edit::insert(Position::new(0, 0), "B")]);
        assert_eq!(history.len(), 1);
        assert_eq!(history.entries()[0].edits()[0].text(), "B");
    }

    #[test]
    fn test_history_entry_clone() {
        let mut history = History::new();
        history.record(vec![Edit::insert(Position::new(0, 0), "A")]);

        let entry = history.get(0).unwrap();
        let cloned = entry.clone();
        assert_eq!(cloned.edits()[0].text(), entry.edits()[0].text());
        assert_eq!(cloned.seq_num(), entry.seq_num());
        assert_eq!(cloned.timestamp(), entry.timestamp());
    }

    // === Coverage: remove_node root protection (L554-555) ===

    #[test]
    fn test_remove_node_root_protection() {
        // Create a tree with max_nodes=2. Push 2 edits on a linear path.
        // Tree: root(0) -> A(1) -> B(2). Current = 2.
        // When B is pushed, node_count=3 > max_nodes=2.
        // prune_if_needed tries to remove nodes not on the protected path.
        // Protected path: root(0) -> A(1) -> B(2). All are protected.
        // No leaf nodes to prune, so nothing removed. Root survives.
        let mut tree = UndoTree::with_max_nodes(2);
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
        // Root must still exist
        assert!(tree.node_count() >= 2);
        assert!(tree.can_undo());
    }

    // === Coverage: prune_adjusts_parent_indices (L565-568) ===

    #[test]
    fn test_prune_adjusts_parent_indices() {
        // Build a tree with branches, then trigger pruning that removes
        // a node with index lower than other nodes' parent indices,
        // forcing the index adjustment code at L565-568.
        //
        // Tree structure with max_nodes=4:
        //   root(0) -> A(1) -> B(2)   (branch 1, leaf)
        //                  \-> C(3)    (branch 2)
        //                       \-> D(4)  (current, protected)
        //
        // Protected path: root(0) -> A(1) -> C(3) -> D(4)
        // B(2) is a leaf, not protected -> gets pruned.
        // After removing B(2), indices shift: C was 3 -> becomes 2, D was 4 -> becomes 3.
        // Parent index of C (was pointing to A=1) stays 1.
        // Parent index of D (was pointing to C=3) must be adjusted to 2.
        let mut tree = UndoTree::with_max_nodes(4);

        // Push A
        tree.push(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
        );
        // Push B (branch from A)
        tree.push(
            vec![Edit::insert(Position::new(0, 1), "B")],
            Position::new(0, 1),
            Position::new(0, 2),
        );
        // Undo back to A
        tree.undo();
        // Push C (new branch from A)
        tree.push(
            vec![Edit::insert(Position::new(0, 1), "C")],
            Position::new(0, 1),
            Position::new(0, 2),
        );
        // Push D (child of C) - this puts us over max_nodes, triggers prune
        tree.push(
            vec![Edit::insert(Position::new(0, 2), "D")],
            Position::new(0, 2),
            Position::new(0, 3),
        );

        // After pruning, B should be removed and indices adjusted
        assert!(tree.node_count() <= 4);
        // The path root -> A -> C -> D should still be navigable
        assert!(tree.can_undo());
        let result = tree.undo().unwrap();
        assert_eq!(result.edits[0].text(), "D");
    }
}
