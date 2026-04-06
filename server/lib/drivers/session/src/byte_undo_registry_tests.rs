use reovim_kernel::api::v1::{BufferId, ByteEdit};

use super::ByteUndoRegistry;

#[test]
fn new_is_empty() {
    let reg = ByteUndoRegistry::new();
    assert!(reg.is_empty());
    assert_eq!(reg.len(), 0);
}

#[test]
fn push_creates_log_on_first_use() {
    let reg = ByteUndoRegistry::new();
    let id = BufferId::from_raw(1);

    reg.push(id, vec![ByteEdit::insert(0, b"hello")]);
    assert_eq!(reg.len(), 1);
    assert!(reg.can_undo(id));
    assert!(!reg.can_redo(id));
}

#[test]
fn push_multiple_buffers() {
    let reg = ByteUndoRegistry::new();
    let id1 = BufferId::from_raw(1);
    let id2 = BufferId::from_raw(2);

    reg.push(id1, vec![ByteEdit::insert(0, b"a")]);
    reg.push(id2, vec![ByteEdit::insert(0, b"b")]);
    assert_eq!(reg.len(), 2);
    assert!(reg.can_undo(id1));
    assert!(reg.can_undo(id2));
}

#[test]
fn can_undo_redo_unknown_buffer() {
    let reg = ByteUndoRegistry::new();
    let id = BufferId::from_raw(99);
    assert!(!reg.can_undo(id));
    assert!(!reg.can_redo(id));
}

#[test]
fn remove_clears_log() {
    let reg = ByteUndoRegistry::new();
    let id = BufferId::from_raw(1);

    reg.push(id, vec![ByteEdit::insert(0, b"hello")]);
    assert_eq!(reg.len(), 1);

    reg.remove(id);
    assert!(reg.is_empty());
    assert!(!reg.can_undo(id));
}

#[test]
fn clear_removes_all() {
    let reg = ByteUndoRegistry::new();
    let id1 = BufferId::from_raw(1);
    let id2 = BufferId::from_raw(2);

    reg.push(id1, vec![ByteEdit::insert(0, b"a")]);
    reg.push(id2, vec![ByteEdit::insert(0, b"b")]);
    assert_eq!(reg.len(), 2);

    reg.clear();
    assert!(reg.is_empty());
}

#[test]
fn default_is_empty() {
    let reg = ByteUndoRegistry::default();
    assert!(reg.is_empty());
}

#[test]
fn push_multiple_edits_to_same_buffer() {
    let reg = ByteUndoRegistry::new();
    let id = BufferId::from_raw(1);

    reg.push(id, vec![ByteEdit::insert(0, b"hello")]);
    reg.push(id, vec![ByteEdit::insert(5, b" world")]);
    assert_eq!(reg.len(), 1); // still one buffer
    assert!(reg.can_undo(id));
}
