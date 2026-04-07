use super::*;

// -- Construction --

#[test]
fn new_log_is_empty() {
    let log = ByteUndoLog::new();
    assert_eq!(log.len(), 0);
    assert!(!log.can_undo());
    assert!(!log.can_redo());
}

#[test]
fn default_log_is_empty() {
    let log = ByteUndoLog::default();
    assert_eq!(log.len(), 0);
}

// -- Push --

#[test]
fn push_single_entry() {
    let mut log = ByteUndoLog::new();
    log.push(vec![ByteEdit::insert(0, b"hello")]);
    assert_eq!(log.len(), 1);
    assert!(log.can_undo());
    assert!(!log.can_redo());
}

#[test]
fn push_multiple_entries() {
    let mut log = ByteUndoLog::new();
    log.push(vec![ByteEdit::insert(0, b"hello")]);
    log.push(vec![ByteEdit::insert(5, b" world")]);
    assert_eq!(log.len(), 2);
}

#[test]
fn push_empty_edits_is_valid() {
    let mut log = ByteUndoLog::new();
    log.push(vec![]);
    assert_eq!(log.len(), 1);
    assert!(log.can_undo());
}

#[test]
fn push_clears_redo_stack() {
    let mut log = ByteUndoLog::new();
    log.push(vec![ByteEdit::insert(0, b"a")]);
    log.push(vec![ByteEdit::insert(1, b"b")]);

    // Undo one step, creating a redo entry
    log.undo();
    assert!(log.can_redo());
    assert_eq!(log.redo_len(), 1);

    // Push a new entry — redo stack should be cleared
    log.push(vec![ByteEdit::insert(1, b"c")]);
    assert!(!log.can_redo());
    assert_eq!(log.redo_len(), 0);
    assert_eq!(log.len(), 2);
}

// -- Undo --

#[test]
fn undo_returns_none_when_empty() {
    let mut log = ByteUndoLog::new();
    assert!(log.undo().is_none());
}

#[test]
fn undo_returns_last_entry() {
    let mut log = ByteUndoLog::new();
    log.push(vec![ByteEdit::insert(0, b"hello")]);
    log.push(vec![ByteEdit::insert(5, b" world")]);

    let entry = log.undo().unwrap();
    assert_eq!(entry.edits().len(), 1);
    assert_eq!(entry.edits()[0].new_bytes, b" world");
}

#[test]
fn undo_moves_cursor_backward() {
    let mut log = ByteUndoLog::new();
    log.push(vec![ByteEdit::insert(0, b"a")]);
    log.push(vec![ByteEdit::insert(1, b"b")]);

    assert_eq!(log.len(), 2);
    log.undo();
    assert_eq!(log.len(), 1);
    log.undo();
    assert_eq!(log.len(), 0);
    assert!(!log.can_undo());
}

#[test]
fn undo_enables_redo() {
    let mut log = ByteUndoLog::new();
    log.push(vec![ByteEdit::insert(0, b"a")]);
    assert!(!log.can_redo());

    log.undo();
    assert!(log.can_redo());
}

#[test]
fn undo_inverse_edits_are_reversed_and_inverted() {
    let mut log = ByteUndoLog::new();
    let e1 = ByteEdit::insert(0, b"hello");
    let e2 = ByteEdit::insert(5, b" world");
    log.push(vec![e1.clone(), e2.clone()]);

    let entry = log.undo().unwrap();
    let inv = entry.inverse_edits();

    // Inverse of [insert, insert] should be [delete, delete] in reverse order
    assert_eq!(inv.len(), 2);
    // Last edit inverted first
    assert_eq!(inv[0], e2.inverse());
    assert_eq!(inv[1], e1.inverse());
}

// -- Redo --

#[test]
fn redo_returns_none_when_at_end() {
    let mut log = ByteUndoLog::new();
    assert!(log.redo().is_none());

    log.push(vec![ByteEdit::insert(0, b"a")]);
    assert!(log.redo().is_none()); // cursor is at end
}

#[test]
fn redo_after_undo() {
    let mut log = ByteUndoLog::new();
    log.push(vec![ByteEdit::insert(0, b"hello")]);
    log.undo();

    let entry = log.redo().unwrap();
    assert_eq!(entry.edits()[0].new_bytes, b"hello");
}

#[test]
fn redo_moves_cursor_forward() {
    let mut log = ByteUndoLog::new();
    log.push(vec![ByteEdit::insert(0, b"a")]);
    log.push(vec![ByteEdit::insert(1, b"b")]);

    log.undo();
    log.undo();
    assert_eq!(log.len(), 0);

    log.redo();
    assert_eq!(log.len(), 1);
    log.redo();
    assert_eq!(log.len(), 2);
    assert!(!log.can_redo());
}

// -- Clear --

#[test]
fn clear_resets_everything() {
    let mut log = ByteUndoLog::new();
    log.push(vec![ByteEdit::insert(0, b"a")]);
    log.push(vec![ByteEdit::insert(1, b"b")]);
    log.undo(); // create redo entry

    log.clear();
    assert_eq!(log.len(), 0);
    assert!(!log.can_undo());
    assert!(!log.can_redo());
    assert_eq!(log.redo_len(), 0);
}

// -- ByteUndoEntry --

#[test]
fn entry_edits_accessor() {
    let edit = ByteEdit::insert(0, b"test");
    let entry = ByteUndoEntry {
        edits: vec![edit.clone()],
    };
    assert_eq!(entry.edits(), &[edit]);
}

#[test]
fn entry_inverse_edits_single() {
    let edit = ByteEdit::insert(0, b"hello");
    let entry = ByteUndoEntry {
        edits: vec![edit.clone()],
    };
    let inv = entry.inverse_edits();
    assert_eq!(inv, vec![edit.inverse()]);
}

// -- Redo length --

#[test]
fn redo_len_tracks_correctly() {
    let mut log = ByteUndoLog::new();
    log.push(vec![ByteEdit::insert(0, b"a")]);
    log.push(vec![ByteEdit::insert(1, b"b")]);
    log.push(vec![ByteEdit::insert(2, b"c")]);

    assert_eq!(log.redo_len(), 0);
    log.undo();
    assert_eq!(log.redo_len(), 1);
    log.undo();
    assert_eq!(log.redo_len(), 2);
    log.redo();
    assert_eq!(log.redo_len(), 1);
}
