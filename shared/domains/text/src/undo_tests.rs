use {
    super::*,
    crate::{Edit, Position},
};

fn make_edit(text: &str) -> Vec<Edit> {
    vec![Edit::Insert {
        position: Position::default(),
        text: text.to_string(),
    }]
}

#[test]
fn test_push_undo_redo_cycle() {
    let mut tree = UndoTree::new();
    assert!(!tree.can_undo());
    assert!(!tree.can_redo());

    tree.push(make_edit("a"), Position::new(0, 0), Position::new(0, 1));
    assert!(tree.can_undo());
    assert!(!tree.can_redo());

    let undo_result = tree.undo();
    assert!(undo_result.is_some());
    assert!(!tree.can_undo());
    assert!(tree.can_redo());

    let redo_result = tree.redo();
    assert!(redo_result.is_some());
    assert!(tree.can_undo());
    assert!(!tree.can_redo());
}

#[test]
fn test_branching_after_undo() {
    let mut tree = UndoTree::new();
    tree.push(make_edit("a"), Position::default(), Position::new(0, 1));
    tree.push(make_edit("b"), Position::new(0, 1), Position::new(0, 2));

    tree.undo();
    tree.undo();

    tree.push(make_edit("c"), Position::default(), Position::new(0, 1));
    assert_eq!(tree.node(0).unwrap().children().len(), 2);
}

#[test]
fn test_active_branches_growth_via_deserialization() {
    use std::time::Duration;

    let tree = UndoTree::from_serializable(
        vec![
            (
                Vec::new(),
                Position::default(),
                Position::default(),
                Duration::ZERO,
                None,
                vec![1],
                0,
                EditOrigin::System,
            ),
            (
                make_edit("a"),
                Position::default(),
                Position::new(0, 1),
                Duration::from_secs(1),
                Some(0),
                Vec::new(),
                1,
                EditOrigin::System,
            ),
        ],
        1,
        1,
        100,
        vec![0],
    );

    let mut tree = tree;
    tree.push(make_edit("b"), Position::new(0, 1), Position::new(0, 2));

    assert!(tree.can_undo());
    assert_eq!(tree.current_node().edits().len(), 1);
}

#[test]
fn test_switch_branch_active_branches_growth() {
    use std::time::Duration;

    let mut tree = UndoTree::from_serializable(
        vec![
            (
                Vec::new(),
                Position::default(),
                Position::default(),
                Duration::ZERO,
                None,
                vec![1, 2],
                0,
                EditOrigin::System,
            ),
            (
                make_edit("a"),
                Position::default(),
                Position::new(0, 1),
                Duration::from_secs(1),
                Some(0),
                Vec::new(),
                1,
                EditOrigin::System,
            ),
            (
                make_edit("b"),
                Position::default(),
                Position::new(0, 1),
                Duration::from_secs(2),
                Some(0),
                Vec::new(),
                2,
                EditOrigin::System,
            ),
        ],
        0,
        2,
        100,
        vec![],
    );

    assert!(tree.switch_branch(1));
}

#[test]
fn test_pruning_removes_unprotected_leaf_nodes() {
    let mut tree = UndoTree::with_max_nodes(3);
    tree.push(make_edit("a"), Position::default(), Position::new(0, 1));

    tree.undo();
    tree.push(make_edit("b"), Position::default(), Position::new(0, 1));
    tree.push(make_edit("c"), Position::new(0, 1), Position::new(0, 2));

    assert!(tree.can_undo());
    let undo = tree.undo();
    assert!(undo.is_some());
}

#[test]
fn test_push_with_origin() {
    let mut tree = UndoTree::new();
    tree.push_with_origin(
        make_edit("a"),
        Position::default(),
        Position::new(0, 1),
        EditOrigin::Client(42),
    );
    assert_eq!(tree.current_node().origin(), EditOrigin::Client(42));
}

#[test]
fn test_redo_branch_specific() {
    let mut tree = UndoTree::new();
    tree.push(make_edit("a"), Position::default(), Position::new(0, 1));
    tree.undo();
    tree.push(make_edit("b"), Position::default(), Position::new(0, 1));
    tree.undo();

    assert_eq!(tree.branches().len(), 2);

    let result = tree.redo_branch(0);
    assert!(result.is_some());
}

#[test]
fn test_clear_resets_tree() {
    let mut tree = UndoTree::new();
    tree.push(make_edit("a"), Position::default(), Position::new(0, 1));
    tree.push(make_edit("b"), Position::new(0, 1), Position::new(0, 2));

    tree.clear();
    assert!(!tree.can_undo());
    assert!(!tree.can_redo());
}

#[test]
fn test_edits_since() {
    let mut tree = UndoTree::new();
    tree.push(make_edit("a"), Position::default(), Position::new(0, 1));
    tree.push(make_edit("b"), Position::new(0, 1), Position::new(0, 2));

    let edits = tree.edits_since(0);
    assert!(edits.is_some());
    assert_eq!(edits.unwrap().len(), 2);
}

#[test]
fn test_push_empty_edits_is_noop() {
    let mut tree = UndoTree::new();
    tree.push(vec![], Position::default(), Position::default());
    assert!(!tree.can_undo());
}

#[test]
fn test_switch_branch_out_of_bounds() {
    let mut tree = UndoTree::new();
    tree.push(make_edit("a"), Position::default(), Position::new(0, 1));
    tree.undo();

    assert!(!tree.switch_branch(99));
}

#[test]
fn test_active_branch_at() {
    let mut tree = UndoTree::new();
    tree.push(make_edit("a"), Position::default(), Position::new(0, 1));
    tree.undo();
    tree.push(make_edit("b"), Position::default(), Position::new(0, 1));
    tree.undo();

    let active = tree.active_branch_at(0);
    assert!(active.is_some());
    assert_eq!(active.unwrap(), 1);
}

#[test]
fn test_node_access() {
    let mut tree = UndoTree::new();
    tree.push(make_edit("a"), Position::default(), Position::new(0, 1));

    assert!(tree.node(0).is_some());
    assert!(tree.node(1).is_some());
    assert!(tree.node(99).is_none());
}

#[test]
fn test_set_max_nodes() {
    let mut tree = UndoTree::new();
    tree.push(make_edit("a"), Position::default(), Position::new(0, 1));
    tree.undo();
    tree.push(make_edit("b"), Position::default(), Position::new(0, 1));
    tree.push(make_edit("c"), Position::new(0, 1), Position::new(0, 2));
    tree.push(make_edit("d"), Position::new(0, 2), Position::new(0, 3));

    tree.set_max_nodes(3);
    assert!(tree.can_undo());
}

mod undo_tree_tests {
    use super::*;

    #[test]
    fn test_new_tree_at_root() {
        let tree = UndoTree::new();
        assert!(!tree.can_undo());
        assert!(!tree.can_redo());
        assert_eq!(tree.node_count(), 1);
    }

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

        let node = tree.node(1).expect("node should exist");
        assert_eq!(node.origin(), EditOrigin::System);
    }

    #[test]
    fn test_push_with_origin_tags_node() {
        let mut tree = UndoTree::new();

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
            EditOrigin::Client(0),
        );

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

        tree.push(
            vec![Edit::insert(Position::new(0, 0), "Hello")],
            Position::new(0, 0),
            Position::new(0, 5),
        );
        assert!(tree.can_undo());
        assert!(!tree.can_redo());

        tree.push(
            vec![Edit::insert(Position::new(0, 5), " World")],
            Position::new(0, 5),
            Position::new(0, 11),
        );

        let result = tree.undo().expect("should be able to undo");
        assert_eq!(result.cursor, Position::new(0, 5));
        assert!(result.edits[0].is_delete());

        assert!(tree.can_undo());
        assert!(tree.can_redo());

        let result = tree.undo().expect("should be able to undo");
        assert_eq!(result.cursor, Position::new(0, 0));

        assert!(!tree.can_undo());
        assert!(tree.can_redo());

        let result = tree.redo().expect("should be able to redo");
        assert_eq!(result.cursor, Position::new(0, 5));
        assert!(result.edits[0].is_insert());

        let result = tree.redo().expect("should be able to redo");
        assert_eq!(result.cursor, Position::new(0, 11));
    }

    #[test]
    fn test_undo_restores_cursor() {
        let mut tree = UndoTree::new();

        tree.push(
            vec![Edit::insert(Position::new(5, 10), "text")],
            Position::new(5, 10),
            Position::new(5, 14),
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
        assert!(tree.can_redo());

        tree.push(
            vec![Edit::insert(Position::new(0, 1), "C")],
            Position::new(0, 1),
            Position::new(0, 2),
        );

        tree.undo();

        let branches = tree.branches();
        assert_eq!(branches.len(), 2);
    }

    #[test]
    fn test_switch_branch() {
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

        tree.undo();

        let branches = tree.branches();
        assert_eq!(branches.len(), 2);

        assert!(tree.switch_branch(0));

        let result = tree.redo().unwrap();
        assert_eq!(result.edits[0].text(), "B");
    }

    #[test]
    fn test_max_nodes_pruning() {
        let mut tree = UndoTree::with_max_nodes(5);

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

        for c in ['D', 'E', 'F', 'G'] {
            tree.push(
                vec![Edit::insert(Position::new(0, 1), c.to_string())],
                Position::new(0, 1),
                Position::new(0, 2),
            );
            tree.undo();
        }

        tree.push(
            vec![Edit::insert(Position::new(0, 1), "H")],
            Position::new(0, 1),
            Position::new(0, 2),
        );

        assert!(tree.node_count() <= 5);
        assert!(tree.can_undo());

        let result = tree.undo().unwrap();
        assert_eq!(result.edits[0].text(), "H");
    }

    #[test]
    fn test_empty_edits_not_pushed() {
        let mut tree = UndoTree::new();

        tree.push(vec![], Position::new(0, 0), Position::new(0, 0));

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

        assert!(tree.node(0).is_some());
        assert!(tree.node(tree.node_count()).is_none());
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

        assert!(tree.node(0).is_some());
        assert!(tree.node(1).is_some());
        assert!(tree.node(2).is_some());
        assert!(tree.node(3).is_none());

        let node_a = tree.node(1).unwrap();
        assert_eq!(node_a.parent(), Some(0));
        assert_eq!(node_a.children(), &[2]);
    }

    #[test]
    fn test_node_indices_empty_tree() {
        let tree = UndoTree::new();
        let indices: Vec<_> = tree.node_indices().collect();
        assert_eq!(indices, vec![0]);
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

        for idx in tree.node_indices() {
            assert!(tree.node(idx).is_some(), "Index {idx} should be valid");
        }

        assert_eq!(tree.node_indices().count(), tree.node_count());
    }

    #[test]
    fn test_active_branch_at_with_branches() {
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
        tree.undo();

        let active = tree.active_branch_at(1);
        assert!(active.is_some());
        assert_eq!(active.unwrap(), 1);
    }

    #[test]
    fn test_active_branch_at_edge_cases() {
        let tree = UndoTree::new();

        assert!(tree.active_branch_at(999).is_none());

        let root = tree.node(0).unwrap();
        assert!(root.children().is_empty());
    }

    #[test]
    fn test_active_branch_after_switch() {
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
        tree.undo();

        tree.switch_branch(0);
        assert_eq!(tree.active_branch_at(1), Some(0));

        tree.switch_branch(1);
        assert_eq!(tree.active_branch_at(1), Some(1));
    }

    #[test]
    fn test_undo_node_parent_accessor() {
        let mut tree = UndoTree::new();
        tree.push(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
        );

        assert!(tree.node(0).unwrap().parent().is_none());
        assert_eq!(tree.node(1).unwrap().parent(), Some(0));
    }

    #[test]
    fn test_undo_node_children_accessor() {
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

        assert_eq!(tree.node(0).unwrap().children(), &[1]);
        assert_eq!(tree.node(1).unwrap().children(), &[2, 3]);
        assert!(tree.node(2).unwrap().children().is_empty());
        assert!(tree.node(3).unwrap().children().is_empty());
    }

    #[test]
    fn test_tree_structure_invariants() {
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

        for index in tree.node_indices() {
            let node = tree.node(index).unwrap();

            if let Some(parent_idx) = node.parent() {
                let parent = tree.node(parent_idx).unwrap();
                assert!(
                    parent.children().contains(&index),
                    "Node {index} has parent {parent_idx}, but parent doesn't list it"
                );
            }

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

        tree.push(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
        );

        for c in ['B', 'C', 'D', 'E', 'F'] {
            tree.push(
                vec![Edit::insert(Position::new(0, 1), c.to_string())],
                Position::new(0, 1),
                Position::new(0, 2),
            );
            tree.undo();
        }

        tree.push(
            vec![Edit::insert(Position::new(0, 1), "G")],
            Position::new(0, 1),
            Position::new(0, 2),
        );

        assert!(tree.node_count() <= 5);

        for index in tree.node_indices() {
            assert!(tree.node(index).is_some(), "node_indices() returned invalid index {index}");
        }

        let current_idx = tree.current_index();
        assert!(tree.node(current_idx).is_some());

        for index in tree.node_indices() {
            let node = tree.node(index).unwrap();
            if let Some(parent_idx) = node.parent() {
                assert!(tree.node(parent_idx).is_some());
            }
        }
    }
}

mod edits_since_tests {
    use super::*;

    #[test]
    fn test_edits_since_same_as_current() {
        let mut tree = UndoTree::new();

        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 0), "AAA")],
            Position::new(0, 0),
            Position::new(0, 3),
            EditOrigin::Client(0),
        );

        let edits = tree.edits_since(tree.current_index()).unwrap();
        assert!(edits.is_empty());
    }

    #[test]
    fn test_edits_since_one_step() {
        let mut tree = UndoTree::new();

        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 0), "AAA")],
            Position::new(0, 0),
            Position::new(0, 3),
            EditOrigin::Client(0),
        );

        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 3), "BBB")],
            Position::new(0, 3),
            Position::new(0, 6),
            EditOrigin::Client(1),
        );

        let edits = tree.edits_since(1).unwrap();
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].text(), "BBB");
        assert!(edits[0].is_insert());
    }

    #[test]
    fn test_edits_since_multiple_steps() {
        let mut tree = UndoTree::new();

        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 0), "A1")],
            Position::new(0, 0),
            Position::new(0, 2),
            EditOrigin::Client(0),
        );

        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 2), "B1")],
            Position::new(0, 2),
            Position::new(0, 4),
            EditOrigin::Client(1),
        );

        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 4), "C1")],
            Position::new(0, 4),
            Position::new(0, 6),
            EditOrigin::Client(2),
        );

        let edits = tree.edits_since(1).unwrap();
        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0].text(), "B1");
        assert_eq!(edits[1].text(), "C1");
    }

    #[test]
    fn test_edits_since_not_ancestor() {
        let mut tree = UndoTree::new();

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

        tree.undo();

        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 1), "C")],
            Position::new(0, 1),
            Position::new(0, 2),
            EditOrigin::Client(2),
        );

        assert!(tree.edits_since(2).is_none());
    }

    #[test]
    fn test_edits_since_with_branches() {
        let mut tree = UndoTree::new();

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

        tree.undo();
        tree.undo();

        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 1), "D")],
            Position::new(0, 1),
            Position::new(0, 2),
            EditOrigin::Client(3),
        );

        let edits = tree.edits_since(1).unwrap();
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].text(), "D");

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

        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 0), "X")],
            Position::new(0, 0),
            Position::new(0, 1),
            EditOrigin::Client(0),
        );

        tree.push_with_origin(
            vec![
                Edit::insert(Position::new(0, 1), "AA"),
                Edit::insert(Position::new(0, 3), "BB"),
            ],
            Position::new(0, 1),
            Position::new(0, 5),
            EditOrigin::Client(1),
        );

        let edits = tree.edits_since(1).unwrap();
        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0].text(), "AA");
        assert_eq!(edits[1].text(), "BB");
    }

    #[test]
    fn test_edits_since_skips_empty() {
        let mut tree = UndoTree::new();

        tree.push_with_origin(
            vec![Edit::insert(Position::new(0, 0), "A")],
            Position::new(0, 0),
            Position::new(0, 1),
            EditOrigin::Client(0),
        );

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
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].text(), "B");
    }
}

mod undo_tree_extended_tests {
    use {super::*, std::time::Duration};

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

        assert!(tree.node_count() > 3);

        tree.set_max_nodes(3);
        assert!(tree.node_count() <= 3);
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

        tree.undo();
        assert_eq!(tree.seq_counter(), 2);

        tree.redo();
        assert_eq!(tree.seq_counter(), 2);
    }

    #[test]
    fn test_seq_counter_increments_on_empty_edit_skip() {
        let mut tree = UndoTree::new();
        tree.push(vec![], Position::new(0, 0), Position::new(0, 0));
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

        assert_eq!(tree.node(0).unwrap().seq_num(), 0);
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

        assert_eq!(tree.node(1).unwrap().branch_count(), 2);
    }

    #[test]
    fn test_redo_branch_valid() {
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
        tree.undo();

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

        assert!(tree.redo_branch(1).is_none());
    }

    #[test]
    fn test_switch_branch_invalid() {
        let mut tree = UndoTree::new();
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
        assert_eq!(result.edits.len(), 2);
        assert!(result.edits[0].is_delete());
        assert!(result.edits[1].is_delete());
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
                Duration::from_secs(1),
                Some(0),
                Vec::new(),
                1u64,
                EditOrigin::Client(5),
            ),
        ];

        let tree = UndoTree::from_serializable(nodes_data, 1, 1, 100, vec![0, 0]);

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

        let tree = UndoTree::from_serializable(nodes_data, 0, 0, 0, vec![0]);

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
                Duration::from_secs(1),
                Some(0),
                vec![2, 3],
                1u64,
                EditOrigin::Client(0),
            ),
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

        let tree = UndoTree::from_serializable(nodes_data, 3, 3, 100, vec![0, 1, 0, 0]);

        assert_eq!(tree.node_count(), 4);
        assert_eq!(tree.current_index(), 3);

        let node_a = tree.node(1).unwrap();
        assert_eq!(node_a.branch_count(), 2);
        assert_eq!(node_a.children(), &[2, 3]);
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

        assert_eq!(tree.node_count(), 6);

        for _ in 0..5 {
            assert!(tree.undo().is_some());
        }
        assert!(!tree.can_undo());
        assert_eq!(tree.current_index(), 0);

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
        assert_eq!(tree.node_count(), 1);
    }

    #[test]
    fn test_edit_origin_copy_and_hash() {
        use std::collections::HashSet;

        let origin1 = EditOrigin::Client(1);
        let origin2 = origin1;
        assert_eq!(origin1, origin2);

        let mut set = HashSet::new();
        set.insert(EditOrigin::System);
        set.insert(EditOrigin::Client(1));
        set.insert(EditOrigin::Client(1));
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

        let cloned = tree.clone();
        assert_eq!(cloned.node_count(), tree.node_count());
        assert_eq!(cloned.current_index(), tree.current_index());
        assert_eq!(cloned.seq_counter(), tree.seq_counter());
    }

    #[test]
    fn test_remove_node_root_protection() {
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

        assert!(tree.node_count() >= 2);
        assert!(tree.can_undo());
    }

    #[test]
    fn test_prune_adjusts_parent_indices() {
        let mut tree = UndoTree::with_max_nodes(4);

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
        tree.push(
            vec![Edit::insert(Position::new(0, 2), "D")],
            Position::new(0, 2),
            Position::new(0, 3),
        );

        assert!(tree.node_count() <= 4);
        assert!(tree.can_undo());
        let result = tree.undo().unwrap();
        assert_eq!(result.edits[0].text(), "D");
    }
}
