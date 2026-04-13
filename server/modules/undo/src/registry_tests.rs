use super::*;

#[test]
fn test_new_registry_empty() {
    let registry = UndoRegistry::new();
    assert_eq!(registry.buffer_count(), 0);
}

#[test]
fn test_record_stores_edit_in_correct_buffer() {
    let registry = UndoRegistry::new();
    let buffer1 = BufferId::from_raw(1);
    let buffer2 = BufferId::from_raw(2);

    let edit = Edit::insert(Position::new(0, 0), "hello");

    registry.record(buffer1, vec![edit], Position::new(0, 0), Position::new(0, 5));

    assert!(registry.has_history(buffer1));
    // buffer2 should not have history yet
    assert!(!registry.has_history(buffer2));
}

#[test]
fn test_undo_returns_edit() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);

    let edit = Edit::insert(Position::new(0, 0), "hello");
    registry.record(buffer_id, vec![edit], Position::new(0, 0), Position::new(0, 5));

    let result = registry.undo(buffer_id);
    assert!(result.is_some());

    let undo_result = result.unwrap();
    assert_eq!(undo_result.cursor, Position::new(0, 0));
}

#[test]
fn test_redo_after_undo() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);

    let edit = Edit::insert(Position::new(0, 0), "hello");
    registry.record(buffer_id, vec![edit], Position::new(0, 0), Position::new(0, 5));

    // Undo first
    let _undo_result = registry.undo(buffer_id);

    // Now redo should work
    let redo_result = registry.redo(buffer_id);
    assert!(redo_result.is_some());

    let result = redo_result.unwrap();
    assert_eq!(result.cursor, Position::new(0, 5));
}

#[test]
fn test_multiple_buffers_isolated() {
    let registry = UndoRegistry::new();
    let buffer1 = BufferId::from_raw(1);
    let buffer2 = BufferId::from_raw(2);

    // Record edits to both buffers
    let edit1 = Edit::insert(Position::new(0, 0), "hello");
    let edit2 = Edit::insert(Position::new(0, 0), "world");

    registry.record(buffer1, vec![edit1], Position::new(0, 0), Position::new(0, 5));
    registry.record(buffer2, vec![edit2], Position::new(0, 0), Position::new(0, 5));

    // Undo buffer1
    let result1 = registry.undo(buffer1);
    assert!(result1.is_some());

    // buffer2 should still have history to undo
    let result2 = registry.undo(buffer2);
    assert!(result2.is_some());
}

#[test]
fn test_undo_nonexistent_buffer_returns_none() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(999);

    let result = registry.undo(buffer_id);
    assert!(result.is_none());
}

#[test]
fn test_remove_clears_history() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);

    // Add some history
    let edit = Edit::insert(Position::new(0, 0), "hello");
    registry.record(buffer_id, vec![edit], Position::new(0, 0), Position::new(0, 5));

    assert!(registry.has_history(buffer_id));

    // Remove
    registry.remove(buffer_id);

    assert!(!registry.has_history(buffer_id));
    assert_eq!(registry.buffer_count(), 0);
}

#[test]
fn test_get_tree_existing_buffer() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);

    // Record an edit to create the tree
    let edit = Edit::insert(Position::new(0, 0), "hello");
    registry.record(buffer_id, vec![edit], Position::new(0, 0), Position::new(0, 5));

    // get_tree should return Some
    let tree = registry.get_tree(buffer_id);
    assert!(tree.is_some());

    // Verify we can read tree properties
    let tree = tree.unwrap();
    assert_eq!(tree.node_count(), 2); // root + 1 edit
}

#[test]
fn test_get_tree_nonexistent_buffer() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(999);

    // get_tree should return None for nonexistent buffer
    let tree = registry.get_tree(buffer_id);
    assert!(tree.is_none());
}

#[test]
fn test_encode_path_component_unix() {
    assert_eq!(encode_path_component("/home/user/file.rs"), "%2Fhome%2Fuser%2Ffile.rs");
}

#[test]
fn test_encode_path_component_windows() {
    assert_eq!(
        encode_path_component("C:\\Users\\Name\\file.txt"),
        "C%3A%5CUsers%5CName%5Cfile.txt"
    );
}

#[test]
fn test_encode_decode_roundtrip() {
    let paths = [
        "/home/user/project/src/main.rs",
        "C:\\Users\\Name\\Documents\\file.txt",
        "/tmp/test%file.txt",
        "/path/with spaces/file.rs",
        "/special<>|?*chars.txt",
    ];

    for path in paths {
        let encoded = encode_path_component(path);
        let decoded = decode_path_component(&encoded);
        assert_eq!(path, decoded, "Round-trip failed for: {path}");
    }
}

#[test]
fn test_undo_file_path() {
    let registry = UndoRegistry::with_data_dir(Path::new("/home/user/.local/share/reovim"));
    let undo_path = registry.undo_file_path("/home/user/project/main.rs");
    assert_eq!(
        undo_path.to_str().unwrap(),
        "/home/user/.local/share/reovim/undo/%2Fhome%2Fuser%2Fproject%2Fmain.rs.undo"
    );
}

#[test]
fn test_undo_dir() {
    let registry = UndoRegistry::with_data_dir(Path::new("/data"));
    assert_eq!(registry.undo_dir(), Path::new("/data/undo"));
}

// ========================================================================
// Multi-Client Undo Tests (#471)
// ========================================================================

#[test]
fn test_record_for_client_tags_origin() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);
    let client_id = 42_usize;

    let edit = Edit::insert(Position::new(0, 0), "hello");
    registry.record_for_client(
        buffer_id,
        client_id,
        vec![edit],
        Position::new(0, 0),
        Position::new(0, 5),
    );

    // Verify the edit was tagged with the correct origin
    let tree = registry.get_tree(buffer_id).expect("tree should exist");
    let current = tree.current_node();
    assert_eq!(current.origin(), EditOrigin::Client(client_id));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_undo_for_client_only_undoes_own_edits() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);
    let client_a = 1_usize;
    let client_b = 2_usize;

    // Client A makes edit 1
    let edit1 = Edit::insert(Position::new(0, 0), "AAA");
    registry.record_for_client(
        buffer_id,
        client_a,
        vec![edit1],
        Position::new(0, 0),
        Position::new(0, 3),
    );

    // Client B makes edit 2
    let edit2 = Edit::insert(Position::new(0, 3), "BBB");
    registry.record_for_client(
        buffer_id,
        client_b,
        vec![edit2],
        Position::new(0, 3),
        Position::new(0, 6),
    );

    // Client A makes edit 3
    let edit3 = Edit::insert(Position::new(0, 6), "CCC");
    registry.record_for_client(
        buffer_id,
        client_a,
        vec![edit3],
        Position::new(0, 6),
        Position::new(0, 9),
    );

    // Client A undoes - should undo their edit 3, skipping B's edit 2
    let result = registry.undo_for_client(buffer_id, client_a);
    assert!(result.is_some(), "Client A should be able to undo");

    let undo_result = result.unwrap();
    // Cursor should restore to position before edit 3
    assert_eq!(undo_result.cursor, Position::new(0, 6));
    // Should have inverse edit (delete "CCC")
    assert_eq!(undo_result.edits.len(), 1);
    assert!(matches!(&undo_result.edits[0], Edit::Delete { text, .. } if text == "CCC"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_undo_for_client_skips_other_clients() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);
    let client_a = 1_usize;
    let client_b = 2_usize;

    // Client A makes an edit
    let edit1 = Edit::insert(Position::new(0, 0), "AAA");
    registry.record_for_client(
        buffer_id,
        client_a,
        vec![edit1],
        Position::new(0, 0),
        Position::new(0, 3),
    );

    // Client B makes two edits
    let edit2 = Edit::insert(Position::new(0, 3), "BBB");
    registry.record_for_client(
        buffer_id,
        client_b,
        vec![edit2],
        Position::new(0, 3),
        Position::new(0, 6),
    );
    let edit3 = Edit::insert(Position::new(0, 6), "CCC");
    registry.record_for_client(
        buffer_id,
        client_b,
        vec![edit3],
        Position::new(0, 6),
        Position::new(0, 9),
    );

    // Client A undoes - should undo their edit 1, skipping B's edits
    let result = registry.undo_for_client(buffer_id, client_a);
    assert!(result.is_some());

    let undo_result = result.unwrap();
    // Should undo "AAA", not "BBB" or "CCC"
    assert_eq!(undo_result.edits.len(), 1);
    assert!(matches!(&undo_result.edits[0], Edit::Delete { text, .. } if text == "AAA"));
}

#[test]
fn test_undo_for_client_returns_none_when_nothing_to_undo() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);
    let client_a = 1_usize;
    let client_b = 2_usize;

    // Only Client B makes edits
    let edit = Edit::insert(Position::new(0, 0), "BBB");
    registry.record_for_client(
        buffer_id,
        client_b,
        vec![edit],
        Position::new(0, 0),
        Position::new(0, 3),
    );

    // Client A tries to undo - should return None (nothing to undo)
    let result = registry.undo_for_client(buffer_id, client_a);
    assert!(result.is_none(), "Client A has no edits to undo");
}

#[test]
fn test_init_client_sets_cursor() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);
    let client_id = 42_usize;

    // Make some edits first
    let edit = Edit::insert(Position::new(0, 0), "hello");
    registry.record(buffer_id, vec![edit], Position::new(0, 0), Position::new(0, 5));

    // Initialize client
    registry.init_client(buffer_id, client_id);

    // Client cursor should be set
    assert!(
        registry
            .client_cursors
            .read()
            .contains_key(&(buffer_id, client_id))
    );
}

#[test]
fn test_remove_client_cleans_up_cursor() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);
    let client_id = 42_usize;

    // Make an edit and init client
    let edit = Edit::insert(Position::new(0, 0), "hello");
    registry.record_for_client(
        buffer_id,
        client_id,
        vec![edit],
        Position::new(0, 0),
        Position::new(0, 5),
    );

    // Client cursor should exist
    assert!(
        registry
            .client_cursors
            .read()
            .contains_key(&(buffer_id, client_id))
    );

    // Remove client
    registry.remove_client(buffer_id, client_id);

    // Client cursor should be gone
    assert!(
        !registry
            .client_cursors
            .read()
            .contains_key(&(buffer_id, client_id))
    );
}

// ========================================================================
// OT-Lite Transformation Tests (#495)
// ========================================================================

#[test]
fn test_undo_for_client_transforms_same_line_insert() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);
    let client_a = 1_usize;
    let client_b = 2_usize;

    // Client A inserts "AAA" at (0,5)
    registry.record_for_client(
        buffer_id,
        client_a,
        vec![Edit::insert(Position::new(0, 5), "AAA")],
        Position::new(0, 5),
        Position::new(0, 8),
    );

    // Client B inserts "BBB" at (0,0) — shifts A's text right by 3
    registry.record_for_client(
        buffer_id,
        client_b,
        vec![Edit::insert(Position::new(0, 0), "BBB")],
        Position::new(0, 0),
        Position::new(0, 3),
    );

    // Client A undoes — inverse Delete "AAA" should be at (0,8), not (0,5)
    let result = registry.undo_for_client(buffer_id, client_a).unwrap();
    assert_eq!(result.edits.len(), 1);
    assert!(result.edits[0].is_delete());
    assert_eq!(result.edits[0].text(), "AAA");
    assert_eq!(result.edits[0].position(), Position::new(0, 8));
}

#[test]
fn test_undo_for_client_transforms_multiline_insert() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);
    let client_a = 1_usize;
    let client_b = 2_usize;

    // Client A inserts "AAA" at (0,0)
    registry.record_for_client(
        buffer_id,
        client_a,
        vec![Edit::insert(Position::new(0, 0), "AAA")],
        Position::new(0, 0),
        Position::new(0, 3),
    );

    // Client B inserts a multiline text "BB\nCC" at (0,3)
    registry.record_for_client(
        buffer_id,
        client_b,
        vec![Edit::insert(Position::new(0, 3), "BB\nCC")],
        Position::new(0, 3),
        Position::new(1, 2),
    );

    // Client A undoes — inverse Delete "AAA" at (0,0) is before B's insert (0,3),
    // so no shift needed. Position stays (0,0).
    let result = registry.undo_for_client(buffer_id, client_a).unwrap();
    assert_eq!(result.edits[0].position(), Position::new(0, 0));
    assert_eq!(result.edits[0].text(), "AAA");
}

#[test]
fn test_undo_for_client_no_intervening_edits() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);
    let client_a = 1_usize;

    // Client A makes a single edit — no other clients
    registry.record_for_client(
        buffer_id,
        client_a,
        vec![Edit::insert(Position::new(0, 0), "AAA")],
        Position::new(0, 0),
        Position::new(0, 3),
    );

    // Undo with no intervening edits — should work exactly like Phase 2
    let result = registry.undo_for_client(buffer_id, client_a).unwrap();
    assert_eq!(result.edits.len(), 1);
    assert!(result.edits[0].is_delete());
    assert_eq!(result.edits[0].text(), "AAA");
    assert_eq!(result.edits[0].position(), Position::new(0, 0));
    assert_eq!(result.cursor, Position::new(0, 0));
}

#[test]
fn test_undo_for_client_transforms_delete() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);
    let client_a = 1_usize;
    let client_b = 2_usize;

    // Client A inserts "AAA" at (0,5)
    registry.record_for_client(
        buffer_id,
        client_a,
        vec![Edit::insert(Position::new(0, 5), "AAA")],
        Position::new(0, 5),
        Position::new(0, 8),
    );

    // Client B deletes "XX" at (0,0) — shifts A's text left by 2
    registry.record_for_client(
        buffer_id,
        client_b,
        vec![Edit::delete(Position::new(0, 0), "XX")],
        Position::new(0, 0),
        Position::new(0, 0),
    );

    // Client A undoes — inverse Delete "AAA" should be at (0,3), not (0,5)
    let result = registry.undo_for_client(buffer_id, client_a).unwrap();
    assert_eq!(result.edits[0].position(), Position::new(0, 3));
    assert_eq!(result.edits[0].text(), "AAA");
}

#[test]
fn test_undo_for_client_multiple_intervening_edits() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);
    let client_a = 1_usize;
    let client_b = 2_usize;
    let client_c = 3_usize;

    // Client A inserts "AAA" at (0,0)
    registry.record_for_client(
        buffer_id,
        client_a,
        vec![Edit::insert(Position::new(0, 0), "AAA")],
        Position::new(0, 0),
        Position::new(0, 3),
    );

    // Client B inserts "B1" at (0,0) — shifts A's text right by 2
    registry.record_for_client(
        buffer_id,
        client_b,
        vec![Edit::insert(Position::new(0, 0), "B1")],
        Position::new(0, 0),
        Position::new(0, 2),
    );

    // Client C inserts "C1" at (0,0) — shifts A's text right by another 2
    registry.record_for_client(
        buffer_id,
        client_c,
        vec![Edit::insert(Position::new(0, 0), "C1")],
        Position::new(0, 0),
        Position::new(0, 2),
    );

    // Client A undoes — inverse Delete "AAA" at (0,0) → (0,4) after two shifts
    let result = registry.undo_for_client(buffer_id, client_a).unwrap();
    assert_eq!(result.edits[0].position(), Position::new(0, 4));
    assert_eq!(result.edits[0].text(), "AAA");
}

#[test]
fn test_undo_for_client_cursor_position_transformed() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);
    let client_a = 1_usize;
    let client_b = 2_usize;

    // Client A inserts "AAA" at (0,5), cursor_before=(0,5)
    registry.record_for_client(
        buffer_id,
        client_a,
        vec![Edit::insert(Position::new(0, 5), "AAA")],
        Position::new(0, 5),
        Position::new(0, 8),
    );

    // Client B inserts "BBB" at (0,0) — shifts cursor right by 3
    registry.record_for_client(
        buffer_id,
        client_b,
        vec![Edit::insert(Position::new(0, 0), "BBB")],
        Position::new(0, 0),
        Position::new(0, 3),
    );

    // Client A undoes — cursor should be transformed from (0,5) to (0,8)
    let result = registry.undo_for_client(buffer_id, client_a).unwrap();
    assert_eq!(result.cursor, Position::new(0, 8));
}

#[test]
fn test_undo_for_client_batch_edits_in_intervening_node() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);
    let client_a = 1_usize;
    let client_b = 2_usize;

    // Client A inserts "X" at (0,5)
    registry.record_for_client(
        buffer_id,
        client_a,
        vec![Edit::insert(Position::new(0, 5), "X")],
        Position::new(0, 5),
        Position::new(0, 6),
    );

    // Client B batches two edits at (0,0) and (0,2):
    // First "AA" at (0,0), then "BB" at (0,2)
    // Total shift: +4
    registry.record_for_client(
        buffer_id,
        client_b,
        vec![
            Edit::insert(Position::new(0, 0), "AA"),
            Edit::insert(Position::new(0, 2), "BB"),
        ],
        Position::new(0, 0),
        Position::new(0, 4),
    );

    // Client A undoes — inverse Delete "X" at (0,5) → (0,9) after both batch shifts
    let result = registry.undo_for_client(buffer_id, client_a).unwrap();
    assert_eq!(result.edits[0].position(), Position::new(0, 9));
    assert_eq!(result.edits[0].text(), "X");
}

/// Simulate the exact insert-mode batching scenario from the integration test.
///
/// Client 0 types "iAAAA<Esc>" → batch of 4 char inserts at (0,0)-(0,3)
/// Client 1 types "0iBBBB<Esc>" → batch of 4 char inserts at (0,0)-(0,3)
/// Client 0 undoes → should remove AAAA (now at col 4-7), not BBBB (at col 0-3)
#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_undo_for_client_batched_insert_mode_scenario() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);

    // Client 0 enters insert mode: begin_batch
    registry.begin_batch(buffer_id, Position::new(0, 0));

    // Client 0 types 'A' 4 times
    for col in 0..4_usize {
        registry.record_for_client(
            buffer_id,
            0,
            vec![Edit::insert(Position::new(0, col), "A")],
            Position::new(0, col),
            Position::new(0, col + 1),
        );
    }

    // Client 0 exits insert mode: end_batch
    registry.end_batch(buffer_id, Position::new(0, 4));

    // Client 1 enters insert mode: begin_batch
    registry.begin_batch(buffer_id, Position::new(0, 0));

    // Client 1 types 'B' 4 times (at position 0, before AAAA)
    for col in 0..4_usize {
        registry.record_for_client(
            buffer_id,
            1,
            vec![Edit::insert(Position::new(0, col), "B")],
            Position::new(0, col),
            Position::new(0, col + 1),
        );
    }

    // Client 1 exits insert mode: end_batch
    registry.end_batch(buffer_id, Position::new(0, 4));

    // Client 0 undoes — OT-lite should transform the inverse edits
    // #554: Batch collapse means Client 0's 4 char inserts are collapsed
    // into a single Insert(0,0,"AAAA"). Undo produces one Delete(0,0,"AAAA"),
    // which OT transforms through Client 1's 4 char inserts at (0,0)-(0,3).
    let result = registry.undo_for_client(buffer_id, 0);
    assert!(result.is_some(), "undo_for_client should return Some");

    let result = result.unwrap();

    // Collapsed batch produces 1 inverse edit
    assert_eq!(result.edits.len(), 1, "Should have 1 collapsed inverse edit");
    assert!(result.edits[0].is_delete(), "Inverse should be Delete");
    assert_eq!(result.edits[0].text(), "AAAA", "Should delete all of AAAA");

    // After OT transformation, position should be shifted right by 4
    // (past Client 1's BBBB inserts)
    assert!(
        result.edits[0].position().column >= 4,
        "Delete position should be >= 4 (shifted past BBBB), got: {}",
        result.edits[0].position().column
    );
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_redo_for_client_restores_own_edits() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);
    let client_a = 1_usize;

    // Client A makes an edit
    let edit = Edit::insert(Position::new(0, 0), "AAA");
    registry.record_for_client(
        buffer_id,
        client_a,
        vec![edit],
        Position::new(0, 0),
        Position::new(0, 3),
    );

    // Client A undoes
    let undo_result = registry.undo_for_client(buffer_id, client_a);
    assert!(undo_result.is_some());

    // Client A redoes
    let redo_result = registry.redo_for_client(buffer_id, client_a);
    assert!(redo_result.is_some());

    let result = redo_result.unwrap();
    assert_eq!(result.cursor, Position::new(0, 3));
    assert_eq!(result.edits.len(), 1);
    assert!(matches!(&result.edits[0], Edit::Insert { text, .. } if text == "AAA"));
}

// ========================================================================
// Additional Coverage Tests
// ========================================================================

#[test]
fn test_default_registry() {
    let registry = UndoRegistry::default();
    assert_eq!(registry.buffer_count(), 0);
}

#[test]
fn test_with_data_dir() {
    let registry = UndoRegistry::with_data_dir(Path::new("/custom/path"));
    assert_eq!(registry.undo_dir(), Path::new("/custom/path/undo"));
}

#[test]
fn test_set_tree() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);

    // No tree initially
    assert!(registry.get_tree_cloned(buffer_id).is_none());

    // Set a tree directly
    let tree = UndoTree::default();
    registry.set_tree(buffer_id, tree);

    // Now it should exist
    assert!(registry.get_tree_cloned(buffer_id).is_some());
    assert!(registry.has_history(buffer_id));
}

#[test]
fn test_redo_nonexistent_buffer_returns_none() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(999);
    assert!(registry.redo(buffer_id).is_none());
}

#[test]
fn test_redo_without_undo_returns_none() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);

    let edit = Edit::insert(Position::new(0, 0), "hello");
    registry.record(buffer_id, vec![edit], Position::new(0, 0), Position::new(0, 5));

    // Redo without prior undo should return None
    assert!(registry.redo(buffer_id).is_none());
}

#[test]
fn test_redo_branch_nonexistent_buffer() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(999);
    assert!(registry.redo_branch(buffer_id, 0).is_none());
}

#[test]
fn test_remove_nonexistent_buffer_is_noop() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(999);
    // Should not panic
    registry.remove(buffer_id);
    assert_eq!(registry.buffer_count(), 0);
}

#[test]
fn test_begin_end_batch_empty() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);

    // Begin batch with no edits
    registry.begin_batch(buffer_id, Position::new(0, 0));
    assert!(registry.is_batching(buffer_id));

    // End batch with no edits - should not create a tree entry
    registry.end_batch(buffer_id, Position::new(0, 0));
    assert!(!registry.is_batching(buffer_id));
    assert!(!registry.has_history(buffer_id));
}

#[test]
fn test_begin_end_batch_with_edits() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);

    registry.begin_batch(buffer_id, Position::new(0, 0));

    // Record during batch - should accumulate
    let edit = Edit::insert(Position::new(0, 0), "hello");
    registry.record(buffer_id, vec![edit], Position::new(0, 0), Position::new(0, 5));

    // During batch, tree should not have entries yet
    // (record accumulates into batch instead of directly into tree)
    assert!(!registry.has_history(buffer_id));

    // End batch
    registry.end_batch(buffer_id, Position::new(0, 5));

    // Now tree should exist with the edit
    assert!(registry.has_history(buffer_id));
}

#[test]
fn test_is_batching_false_by_default() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);
    assert!(!registry.is_batching(buffer_id));
}

#[test]
fn test_record_for_client_empty_edits_is_noop() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);

    registry.record_for_client(buffer_id, 1, vec![], Position::new(0, 0), Position::new(0, 0));

    assert!(!registry.has_history(buffer_id));
}

#[test]
fn test_init_client_no_tree() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);
    let client_id = 1_usize;

    // Init client before any edits - should set cursor to 0
    registry.init_client(buffer_id, client_id);
    assert!(
        registry
            .client_cursors
            .read()
            .contains_key(&(buffer_id, client_id))
    );
}

#[test]
fn test_undo_for_client_nonexistent_buffer() {
    let registry = UndoRegistry::new();
    let result = registry.undo_for_client(BufferId::from_raw(999), 1);
    assert!(result.is_none());
}

#[test]
fn test_redo_for_client_nonexistent_buffer() {
    let registry = UndoRegistry::new();
    let result = registry.redo_for_client(BufferId::from_raw(999), 1);
    assert!(result.is_none());
}

#[test]
fn test_redo_for_client_no_child() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);
    let client_a = 1_usize;

    // Client A makes an edit (no undo, so no redo target)
    let edit = Edit::insert(Position::new(0, 0), "AAA");
    registry.record_for_client(
        buffer_id,
        client_a,
        vec![edit],
        Position::new(0, 0),
        Position::new(0, 3),
    );

    // Without undo, redo should return None
    let result = registry.redo_for_client(buffer_id, client_a);
    assert!(result.is_none());
}

#[test]
fn test_encode_path_component_percent() {
    assert_eq!(encode_path_component("100%done"), "100%25done");
}

#[test]
fn test_encode_path_component_all_special_chars() {
    let encoded = encode_path_component(r#"/<>"|?*:\test"#);
    assert!(encoded.contains("%2F"));
    assert!(encoded.contains("%3C"));
    assert!(encoded.contains("%3E"));
    assert!(encoded.contains("%22"));
    assert!(encoded.contains("%7C"));
    assert!(encoded.contains("%3F"));
    assert!(encoded.contains("%2A"));
    assert!(encoded.contains("%3A"));
    assert!(encoded.contains("%5C"));
}

#[test]
fn test_encode_path_component_no_special_chars() {
    assert_eq!(encode_path_component("hello.rs"), "hello.rs");
}

#[test]
fn test_decode_path_component_invalid_hex() {
    // Invalid hex after % - should keep as-is
    let decoded = decode_path_component("%ZZ");
    assert_eq!(decoded, "%ZZ");
}

#[test]
fn test_decode_path_component_truncated_percent() {
    // Only one char after % instead of two
    let decoded = decode_path_component("%2");
    assert_eq!(decoded, "%2");
}

#[test]
fn test_decode_path_component_plain_text() {
    assert_eq!(decode_path_component("hello.rs"), "hello.rs");
}

#[test]
fn test_undo_file_path_windows() {
    let registry = UndoRegistry::with_data_dir(Path::new("/data"));
    let undo_path = registry.undo_file_path("C:\\Users\\file.txt");
    let path_str = undo_path.to_str().unwrap();
    assert!(path_str.starts_with("/data/undo/"));
    assert!(
        Path::new(path_str)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("undo"))
    );
    assert!(path_str.contains("%3A"));
    assert!(path_str.contains("%5C"));
}

#[test]
fn test_multiple_undo_redo_cycles() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);

    // Record two edits
    let edit1 = Edit::insert(Position::new(0, 0), "hello");
    registry.record(buffer_id, vec![edit1], Position::new(0, 0), Position::new(0, 5));

    let edit2 = Edit::insert(Position::new(0, 5), " world");
    registry.record(buffer_id, vec![edit2], Position::new(0, 5), Position::new(0, 11));

    // Undo twice
    let r1 = registry.undo(buffer_id);
    assert!(r1.is_some());
    let r2 = registry.undo(buffer_id);
    assert!(r2.is_some());

    // Undo a third time should fail (at root)
    let r3 = registry.undo(buffer_id);
    assert!(r3.is_none());

    // Redo twice
    let r4 = registry.redo(buffer_id);
    assert!(r4.is_some());
    let r5 = registry.redo(buffer_id);
    assert!(r5.is_some());

    // Redo a third time should fail (at latest)
    let r6 = registry.redo(buffer_id);
    assert!(r6.is_none());
}

#[test]
fn test_buffer_count_tracks_correctly() {
    let registry = UndoRegistry::new();
    assert_eq!(registry.buffer_count(), 0);

    let b1 = BufferId::from_raw(1);
    let b2 = BufferId::from_raw(2);

    registry.record(
        b1,
        vec![Edit::insert(Position::new(0, 0), "a")],
        Position::new(0, 0),
        Position::new(0, 1),
    );
    assert_eq!(registry.buffer_count(), 1);

    registry.record(
        b2,
        vec![Edit::insert(Position::new(0, 0), "b")],
        Position::new(0, 0),
        Position::new(0, 1),
    );
    assert_eq!(registry.buffer_count(), 2);

    registry.remove(b1);
    assert_eq!(registry.buffer_count(), 1);

    registry.remove(b2);
    assert_eq!(registry.buffer_count(), 0);
}

#[test]
fn test_record_for_client_during_batch_sets_origin() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);
    let client_id = 42_usize;

    registry.begin_batch(buffer_id, Position::new(0, 0));

    registry.record_for_client(
        buffer_id,
        client_id,
        vec![Edit::insert(Position::new(0, 0), "a")],
        Position::new(0, 0),
        Position::new(0, 1),
    );

    registry.end_batch(buffer_id, Position::new(0, 1));

    // Verify origin was set on the batch
    let tree = registry.get_tree(buffer_id).expect("tree should exist");
    let current = tree.current_node();
    assert_eq!(current.origin(), EditOrigin::Client(client_id));
}

// ========================================================================
// Persistence Tests (persist/load via MockVfs)
// ========================================================================

#[test]
fn test_persist_and_load_roundtrip() {
    use reovim_subsys_vfs::MockVfs;

    let registry = UndoRegistry::with_data_dir(Path::new("/test-data"));
    let buffer_id = BufferId::from_raw(1);
    let buffer_path = "/home/user/file.rs";

    // Record some edits
    let edit = Edit::insert(Position::new(0, 0), "hello");
    registry.record(buffer_id, vec![edit], Position::new(0, 0), Position::new(0, 5));

    let vfs = MockVfs::new();

    // Persist the undo tree
    let result = registry.persist(buffer_id, buffer_path, &vfs);
    assert!(result.is_ok(), "persist should succeed");

    // Verify directory was created and file was written
    let undo_path = registry.undo_file_path(buffer_path);
    assert!(vfs.exists(&undo_path), "undo file should exist after persist");

    // Load into a new registry
    let registry2 = UndoRegistry::with_data_dir(Path::new("/test-data"));
    let buffer_id2 = BufferId::from_raw(2);
    let loaded = registry2.load(buffer_id2, buffer_path, &vfs);
    assert!(loaded.is_ok());
    assert!(loaded.unwrap(), "load should return true for existing file");
    assert!(registry2.has_history(buffer_id2));
}

#[test]
fn test_persist_no_history_is_noop() {
    use reovim_subsys_vfs::MockVfs;

    let registry = UndoRegistry::with_data_dir(Path::new("/test-data"));
    let buffer_id = BufferId::from_raw(99);
    let vfs = MockVfs::new();

    // Persist with no history - should succeed without writing
    let result = registry.persist(buffer_id, "/some/file.rs", &vfs);
    assert!(result.is_ok());
    assert!(vfs.write_calls().is_empty(), "no writes should occur for empty history");
}

#[test]
fn test_load_nonexistent_file_returns_false() {
    use reovim_subsys_vfs::MockVfs;

    let registry = UndoRegistry::with_data_dir(Path::new("/test-data"));
    let buffer_id = BufferId::from_raw(1);
    let vfs = MockVfs::new();

    let loaded = registry.load(buffer_id, "/no/such/file.rs", &vfs);
    assert!(loaded.is_ok());
    assert!(!loaded.unwrap(), "load should return false for nonexistent file");
}

#[test]
fn test_load_invalid_data_returns_error() {
    use reovim_subsys_vfs::MockVfs;

    let registry = UndoRegistry::with_data_dir(Path::new("/test-data"));
    let buffer_id = BufferId::from_raw(1);
    let vfs = MockVfs::new();

    // Write garbage data to the undo file
    let undo_path = registry.undo_file_path("/file.rs");
    vfs.add_file(&undo_path, b"not a valid undo file");

    let loaded = registry.load(buffer_id, "/file.rs", &vfs);
    assert!(loaded.is_err(), "load of invalid data should return error");
}

#[test]
fn test_persist_with_io_error() {
    use reovim_subsys_vfs::{MockErrorKind, MockVfs};

    let registry = UndoRegistry::with_data_dir(Path::new("/test-data"));
    let buffer_id = BufferId::from_raw(1);
    let buffer_path = "/file.rs";

    let edit = Edit::insert(Position::new(0, 0), "hello");
    registry.record(buffer_id, vec![edit], Position::new(0, 0), Position::new(0, 5));

    let vfs = MockVfs::new();
    // Make the undo file path fail on write
    let undo_path = registry.undo_file_path(buffer_path);
    vfs.set_error(&undo_path, MockErrorKind::PermissionDenied);

    let result = registry.persist(buffer_id, buffer_path, &vfs);
    assert!(result.is_err(), "persist should fail on write error");
}

#[test]
fn test_load_with_io_error() {
    use reovim_subsys_vfs::{MockErrorKind, MockVfs};

    let registry = UndoRegistry::with_data_dir(Path::new("/test-data"));
    let buffer_id = BufferId::from_raw(1);
    let buffer_path = "/file.rs";
    let vfs = MockVfs::new();

    // Add file but make read fail
    let undo_path = registry.undo_file_path(buffer_path);
    vfs.add_file(&undo_path, b"dummy");
    vfs.set_error(&undo_path, MockErrorKind::PermissionDenied);

    let loaded = registry.load(buffer_id, buffer_path, &vfs);
    assert!(loaded.is_err(), "load should fail on read error");
}

#[test]
fn test_end_batch_without_begin_is_noop() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);

    // end_batch with no active batch should be a no-op
    registry.end_batch(buffer_id, Position::new(0, 0));
    assert!(!registry.has_history(buffer_id));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_end_batch_with_client_origin_updates_cursor() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);
    let client_id = 7_usize;

    registry.begin_batch(buffer_id, Position::new(0, 0));

    // Record via client so origin gets set
    registry.record_for_client(
        buffer_id,
        client_id,
        vec![Edit::insert(Position::new(0, 0), "abc")],
        Position::new(0, 0),
        Position::new(0, 3),
    );

    registry.end_batch(buffer_id, Position::new(0, 3));

    // Verify client cursor was updated
    let key = (buffer_id, client_id);
    let contains = registry.client_cursors.read().contains_key(&key);
    assert!(contains, "client cursor should be set after batch end");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_record_for_client_during_batch_first_origin_wins() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);

    registry.begin_batch(buffer_id, Position::new(0, 0));

    // First client contributes
    registry.record_for_client(
        buffer_id,
        1,
        vec![Edit::insert(Position::new(0, 0), "a")],
        Position::new(0, 0),
        Position::new(0, 1),
    );

    // Second client contributes to same batch
    registry.record_for_client(
        buffer_id,
        2,
        vec![Edit::insert(Position::new(0, 1), "b")],
        Position::new(0, 1),
        Position::new(0, 2),
    );

    registry.end_batch(buffer_id, Position::new(0, 2));

    // First contributor's origin should win
    let tree = registry.get_tree(buffer_id).expect("tree should exist");
    let current = tree.current_node();
    assert_eq!(current.origin(), EditOrigin::Client(1));
}

#[test]
fn test_redo_for_client_finds_recursive_child() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);
    let client_a = 1_usize;
    let client_b = 2_usize;

    // Client A makes edit, then client B, then client A again
    registry.record_for_client(
        buffer_id,
        client_a,
        vec![Edit::insert(Position::new(0, 0), "A1")],
        Position::new(0, 0),
        Position::new(0, 2),
    );
    registry.record_for_client(
        buffer_id,
        client_b,
        vec![Edit::insert(Position::new(0, 2), "B1")],
        Position::new(0, 2),
        Position::new(0, 4),
    );
    registry.record_for_client(
        buffer_id,
        client_a,
        vec![Edit::insert(Position::new(0, 4), "A2")],
        Position::new(0, 4),
        Position::new(0, 6),
    );

    // Undo A2 and A1
    let r1 = registry.undo_for_client(buffer_id, client_a);
    assert!(r1.is_some(), "first undo should succeed");
    let r2 = registry.undo_for_client(buffer_id, client_a);
    assert!(r2.is_some(), "second undo should succeed");

    // Redo should find A1 (not B1, skipping over B1)
    let redo = registry.redo_for_client(buffer_id, client_a);
    assert!(redo.is_some(), "redo should find client A's edit");
}

#[test]
fn test_persist_load_path_mismatch_still_loads() {
    use reovim_subsys_vfs::MockVfs;

    let registry = UndoRegistry::with_data_dir(Path::new("/test-data"));
    let buffer_id = BufferId::from_raw(1);
    let original_path = "/home/user/original.rs";

    let edit = Edit::insert(Position::new(0, 0), "test");
    registry.record(buffer_id, vec![edit], Position::new(0, 0), Position::new(0, 4));

    let vfs = MockVfs::new();
    registry.persist(buffer_id, original_path, &vfs).unwrap();

    // Load using a different buffer path but same undo file
    // We manually move the undo file to simulate path mismatch
    let undo_path = registry.undo_file_path(original_path);
    let undo_bytes = vfs.read(&undo_path).unwrap();

    let different_path = "/home/user/moved.rs";
    let different_undo_path = registry.undo_file_path(different_path);
    vfs.add_file(&different_undo_path, &undo_bytes);

    let registry2 = UndoRegistry::with_data_dir(Path::new("/test-data"));
    let buffer_id2 = BufferId::from_raw(2);
    // This will trigger the path mismatch warning but should still load
    let loaded = registry2.load(buffer_id2, different_path, &vfs);
    assert!(loaded.is_ok());
    assert!(loaded.unwrap(), "load should succeed despite path mismatch");
    assert!(registry2.has_history(buffer_id2));
}

#[test]
fn test_ensure_dir_already_exists() {
    use reovim_subsys_vfs::MockVfs;

    let registry = UndoRegistry::with_data_dir(Path::new("/test-data"));
    let buffer_id = BufferId::from_raw(1);
    let buffer_path = "/file.rs";

    let edit = Edit::insert(Position::new(0, 0), "x");
    registry.record(buffer_id, vec![edit], Position::new(0, 0), Position::new(0, 1));

    let vfs = MockVfs::new();
    // Pre-create the undo directory
    vfs.add_dir("/test-data/undo");

    let result = registry.persist(buffer_id, buffer_path, &vfs);
    assert!(result.is_ok());
}

#[test]
fn test_transform_through_intervening_empty() {
    // Test the helper directly with empty intervening edits
    let inverse_edits = vec![Edit::delete(Position::new(0, 5), "AAA")];
    let cursor = Position::new(0, 5);
    let (edits, pos) = transform_through_intervening(&[], inverse_edits, cursor);
    assert_eq!(edits.len(), 1);
    assert_eq!(pos, Position::new(0, 5));
}

#[test]
fn test_redo_branch_with_branches() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);

    // Create edit 1, undo, create edit 2 (branch)
    registry.record(
        buffer_id,
        vec![Edit::insert(Position::new(0, 0), "A")],
        Position::new(0, 0),
        Position::new(0, 1),
    );
    registry.record(
        buffer_id,
        vec![Edit::insert(Position::new(0, 1), "B")],
        Position::new(0, 1),
        Position::new(0, 2),
    );
    // Undo to get back to node 1
    registry.undo(buffer_id);
    // Create a branch
    registry.record(
        buffer_id,
        vec![Edit::insert(Position::new(0, 1), "C")],
        Position::new(0, 1),
        Position::new(0, 2),
    );
    // Undo again to node 1
    registry.undo(buffer_id);

    // redo_branch(0) should follow the first branch (node with "B")
    let result = registry.redo_branch(buffer_id, 0);
    assert!(result.is_some(), "redo_branch(0) should succeed");
}

#[test]
fn test_load_too_short_file() {
    use reovim_subsys_vfs::MockVfs;

    let registry = UndoRegistry::with_data_dir(Path::new("/test-data"));
    let buffer_id = BufferId::from_raw(1);
    let vfs = MockVfs::new();

    // Write data shorter than 4 bytes (magic header)
    let undo_path = registry.undo_file_path("/file.rs");
    vfs.add_file(&undo_path, b"RU");

    let loaded = registry.load(buffer_id, "/file.rs", &vfs);
    assert!(loaded.is_err(), "load of too-short file should return error");
}

#[test]
fn test_load_valid_magic_corrupt_payload() {
    use reovim_subsys_vfs::MockVfs;

    let registry = UndoRegistry::with_data_dir(Path::new("/test-data"));
    let buffer_id = BufferId::from_raw(1);
    let vfs = MockVfs::new();

    // Write valid magic (RUND) + garbage payload
    let mut data = b"RUND".to_vec();
    data.extend_from_slice(b"\xff\xff\xff\xff\xff");
    let undo_path = registry.undo_file_path("/file.rs");
    vfs.add_file(&undo_path, &data);

    let loaded = registry.load(buffer_id, "/file.rs", &vfs);
    assert!(loaded.is_err(), "load of corrupt payload should return error");
}

#[test]
fn test_redo_for_client_recursive_grandchild() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);
    let client_a = 1_usize;
    let client_b = 2_usize;

    // Client B makes node 1, Client A makes node 2
    registry.record_for_client(
        buffer_id,
        client_b,
        vec![Edit::insert(Position::new(0, 0), "B")],
        Position::new(0, 0),
        Position::new(0, 1),
    );
    registry.record_for_client(
        buffer_id,
        client_a,
        vec![Edit::insert(Position::new(0, 1), "A")],
        Position::new(0, 1),
        Position::new(0, 2),
    );

    // Move tree current back to root via regular undo
    registry.undo(buffer_id); // current → node 1
    registry.undo(buffer_id); // current → node 0

    // Re-init client_a so its cursor is at node 0
    registry.init_client(buffer_id, client_a);

    // redo_for_client should find node 2 (ClientA) as grandchild of node 0
    // via recursive search through node 1 (ClientB)
    let redo = registry.redo_for_client(buffer_id, client_a);
    assert!(redo.is_some(), "redo should find client A's edit via recursive search");
}

// ========================================================================
// collapse_batch_edits tests (#554)
// ========================================================================

#[test]
fn test_collapse_single_edit_unchanged() {
    let edits = vec![Edit::insert(Position::new(0, 0), "a")];
    let result = collapse_batch_edits(&edits, Position::new(0, 0));
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].text(), "a");
}

#[test]
fn test_collapse_empty_edits() {
    let result = collapse_batch_edits(&[], Position::new(0, 0));
    assert!(result.is_empty());
}

#[test]
fn test_collapse_pure_inserts() {
    // Simulates typing "hello" character by character
    let edits = vec![
        Edit::insert(Position::new(0, 0), "h"),
        Edit::insert(Position::new(0, 1), "e"),
        Edit::insert(Position::new(0, 2), "l"),
        Edit::insert(Position::new(0, 3), "l"),
        Edit::insert(Position::new(0, 4), "o"),
    ];
    let result = collapse_batch_edits(&edits, Position::new(0, 0));
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].text(), "hello");
    assert_eq!(result[0].position(), Position::new(0, 0));
}

#[test]
fn test_collapse_inserts_at_offset() {
    // Insert starts at column 5 (appending to existing text)
    let edits = vec![
        Edit::insert(Position::new(0, 5), "a"),
        Edit::insert(Position::new(0, 6), "b"),
        Edit::insert(Position::new(0, 7), "c"),
    ];
    let result = collapse_batch_edits(&edits, Position::new(0, 5));
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].text(), "abc");
    assert_eq!(result[0].position(), Position::new(0, 5));
}

#[test]
fn test_collapse_with_backspace() {
    // Type "helo" then backspace, then "lo" -> net result "hello"
    let edits = vec![
        Edit::insert(Position::new(0, 0), "h"),
        Edit::insert(Position::new(0, 1), "e"),
        Edit::insert(Position::new(0, 2), "l"),
        Edit::insert(Position::new(0, 3), "o"),
        // Backspace: delete 'o' at position 3
        Edit::delete(Position::new(0, 3), "o"),
        Edit::insert(Position::new(0, 3), "l"),
        Edit::insert(Position::new(0, 4), "o"),
    ];
    let result = collapse_batch_edits(&edits, Position::new(0, 0));
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].text(), "hello");
}

#[test]
fn test_collapse_multiline_insert() {
    // Type "hi\nbye" (with newline)
    let edits = vec![
        Edit::insert(Position::new(0, 0), "h"),
        Edit::insert(Position::new(0, 1), "i"),
        Edit::insert(Position::new(0, 2), "\n"),
        Edit::insert(Position::new(1, 0), "b"),
        Edit::insert(Position::new(1, 1), "y"),
        Edit::insert(Position::new(1, 2), "e"),
    ];
    let result = collapse_batch_edits(&edits, Position::new(0, 0));
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].text(), "hi\nbye");
}

#[test]
fn test_collapse_unicode() {
    // Type unicode characters
    let edits = vec![
        Edit::insert(Position::new(0, 0), "a"),
        Edit::insert(Position::new(0, 1), "\u{00e9}"), // e-acute
        Edit::insert(Position::new(0, 2), "\u{1f600}"), // emoji
    ];
    let result = collapse_batch_edits(&edits, Position::new(0, 0));
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].text(), "a\u{00e9}\u{1f600}");
}

#[test]
fn test_collapse_all_deleted() {
    // Insert then delete everything -> empty
    let edits = vec![
        Edit::insert(Position::new(0, 0), "a"),
        Edit::insert(Position::new(0, 1), "b"),
        Edit::delete(Position::new(0, 1), "b"),
        Edit::delete(Position::new(0, 0), "a"),
    ];
    let result = collapse_batch_edits(&edits, Position::new(0, 0));
    assert!(result.is_empty());
}

#[test]
fn test_end_batch_collapses_inserts() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);

    registry.begin_batch(buffer_id, Position::new(0, 0));

    // Simulate typing "abc" char by char
    registry.record(
        buffer_id,
        vec![Edit::insert(Position::new(0, 0), "a")],
        Position::new(0, 0),
        Position::new(0, 1),
    );
    registry.record(
        buffer_id,
        vec![Edit::insert(Position::new(0, 1), "b")],
        Position::new(0, 1),
        Position::new(0, 2),
    );
    registry.record(
        buffer_id,
        vec![Edit::insert(Position::new(0, 2), "c")],
        Position::new(0, 2),
        Position::new(0, 3),
    );

    registry.end_batch(buffer_id, Position::new(0, 3));

    // Undo should produce a single Delete for "abc"
    let undo = registry.undo(buffer_id).expect("undo should succeed");
    assert_eq!(undo.edits.len(), 1, "collapsed batch should produce 1 undo edit");
    assert!(undo.edits[0].is_delete());
    assert_eq!(undo.edits[0].text(), "abc");
    assert_eq!(undo.edits[0].position(), Position::new(0, 0));
}

#[test]
fn test_end_batch_with_origin() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);
    let client_id = 1_usize;

    registry.begin_batch(buffer_id, Position::new(0, 0));
    registry.init_client(buffer_id, client_id);

    // Record via client API to set origin
    registry.record_for_client(
        buffer_id,
        client_id,
        vec![Edit::insert(Position::new(0, 0), "x")],
        Position::new(0, 0),
        Position::new(0, 1),
    );
    registry.record_for_client(
        buffer_id,
        client_id,
        vec![Edit::insert(Position::new(0, 1), "y")],
        Position::new(0, 1),
        Position::new(0, 2),
    );

    registry.end_batch(buffer_id, Position::new(0, 2));

    // Undo via client API should work with collapsed edits
    let undo = registry.undo_for_client(buffer_id, client_id);
    assert!(undo.is_some(), "client undo should succeed after batch");
    let undo = undo.unwrap();
    assert_eq!(undo.edits.len(), 1);
    assert_eq!(undo.edits[0].text(), "xy");
}

#[test]
fn test_undo_redo_cycle_no_corruption() {
    let registry = UndoRegistry::new();
    let buffer_id = BufferId::from_raw(1);

    // Insert "hello"
    registry.begin_batch(buffer_id, Position::new(0, 0));
    for (i, ch) in "hello".chars().enumerate() {
        registry.record(
            buffer_id,
            vec![Edit::insert(Position::new(0, i), ch.to_string())],
            Position::new(0, i),
            Position::new(0, i + 1),
        );
    }
    registry.end_batch(buffer_id, Position::new(0, 5));

    // Undo
    let undo1 = registry.undo(buffer_id).expect("undo should work");
    assert_eq!(undo1.edits.len(), 1);
    assert_eq!(undo1.edits[0].text(), "hello");
    assert!(undo1.edits[0].is_delete());

    // Redo
    let redo1 = registry.redo(buffer_id).expect("redo should work");
    assert_eq!(redo1.edits.len(), 1);
    assert_eq!(redo1.edits[0].text(), "hello");
    assert!(redo1.edits[0].is_insert());

    // Undo again - should be identical
    let undo2 = registry.undo(buffer_id).expect("second undo should work");
    assert_eq!(undo2.edits[0].text(), "hello");
    assert!(undo2.edits[0].is_delete());
}
