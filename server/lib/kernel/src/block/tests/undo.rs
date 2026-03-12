use super::*;

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

    // Undo twice back to root
    tree.undo();
    tree.undo();

    // Push new edit creates a branch at root
    tree.push(make_edit("c"), Position::default(), Position::new(0, 1));
    // Root (node 0) should have 2 children (branches)
    assert_eq!(tree.node(0).unwrap().children().len(), 2);
}

#[test]
fn test_active_branches_growth_via_deserialization() {
    // Deserialize with a short active_branches vector (only root entry)
    // Then push — forces the while loop at line 253 to extend active_branches
    use std::time::Duration;
    let tree = UndoTree::from_serializable(
        vec![
            // root node
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
            // child node (current)
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
        1,       // current = node 1
        1,       // seq_counter
        100,     // max_nodes
        vec![0], // active_branches only has 1 entry (for root) — short!
    );

    // Now push a new edit — current=1, active_branches.len()=1
    // The while loop (line 253) should extend active_branches
    let mut tree = tree;
    tree.push(make_edit("b"), Position::new(0, 1), Position::new(0, 2));

    assert!(tree.can_undo());
    assert_eq!(tree.current_node().edits().len(), 1);
}

#[test]
fn test_switch_branch_active_branches_growth() {
    // Same deserialization approach but test switch_branch (line 376-377)
    use std::time::Duration;
    let mut tree = UndoTree::from_serializable(
        vec![
            // root node with two children
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
            // child 1 (branch 0)
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
            // child 2 (branch 1)
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
        0,      // current = root
        2,      // seq_counter
        100,    // max_nodes
        vec![], // empty active_branches — forces growth in switch_branch
    );

    // switch_branch at current=0 with empty active_branches
    // forces the while loop (line 376) to grow it
    assert!(tree.switch_branch(1));
}

#[test]
fn test_pruning_removes_unprotected_leaf_nodes() {
    // max_nodes=3, push 4 nodes — should trigger pruning
    let mut tree = UndoTree::with_max_nodes(3);
    tree.push(make_edit("a"), Position::default(), Position::new(0, 1));

    // Undo back to root and create a branch
    tree.undo();
    tree.push(make_edit("b"), Position::default(), Position::new(0, 1));

    // Now we have: root -> [node1, node2], current=node2
    // Push another — node count becomes 4, exceeding max_nodes=3
    tree.push(make_edit("c"), Position::new(0, 1), Position::new(0, 2));

    // Pruning should have removed node1 (unprotected leaf)
    // Tree should still work correctly
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

    // Root has 2 branches
    assert_eq!(tree.branches().len(), 2);

    // Redo branch 0 (first child)
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

    // Root only has 1 child, branch 99 doesn't exist
    assert!(!tree.switch_branch(99));
}

#[test]
fn test_active_branch_at() {
    let mut tree = UndoTree::new();
    tree.push(make_edit("a"), Position::default(), Position::new(0, 1));
    tree.undo();
    tree.push(make_edit("b"), Position::default(), Position::new(0, 1));
    tree.undo();

    // Root should have active_branch set (last created was branch 1)
    let active = tree.active_branch_at(0);
    assert!(active.is_some());
    assert_eq!(active.unwrap(), 1);
}

#[test]
fn test_node_access() {
    let mut tree = UndoTree::new();
    tree.push(make_edit("a"), Position::default(), Position::new(0, 1));

    assert!(tree.node(0).is_some()); // root
    assert!(tree.node(1).is_some()); // first edit
    assert!(tree.node(99).is_none()); // out of bounds
}

#[test]
fn test_set_max_nodes() {
    let mut tree = UndoTree::new();
    tree.push(make_edit("a"), Position::default(), Position::new(0, 1));
    tree.undo();
    tree.push(make_edit("b"), Position::default(), Position::new(0, 1));
    tree.push(make_edit("c"), Position::new(0, 1), Position::new(0, 2));
    tree.push(make_edit("d"), Position::new(0, 2), Position::new(0, 3));

    // Lowering max_nodes triggers pruning
    tree.set_max_nodes(3);
    assert!(tree.can_undo());
}
