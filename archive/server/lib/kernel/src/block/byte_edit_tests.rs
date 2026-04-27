use super::ByteEdit;

#[test]
fn insert_creates_pure_insert() {
    let edit = ByteEdit::insert(10, b"hello");
    assert_eq!(edit.offset, 10);
    assert!(edit.old_bytes.is_empty());
    assert_eq!(edit.new_bytes, b"hello");
    assert!(edit.is_insert());
    assert!(!edit.is_delete());
    assert!(!edit.is_empty());
}

#[test]
fn delete_creates_pure_delete() {
    let edit = ByteEdit::delete(5, b"abc");
    assert_eq!(edit.offset, 5);
    assert_eq!(edit.old_bytes, b"abc");
    assert!(edit.new_bytes.is_empty());
    assert!(!edit.is_insert());
    assert!(edit.is_delete());
    assert!(!edit.is_empty());
}

#[test]
fn replace_has_both_old_and_new() {
    let edit = ByteEdit::replace(0, b"foo", b"bar");
    assert_eq!(edit.offset, 0);
    assert_eq!(edit.old_bytes, b"foo");
    assert_eq!(edit.new_bytes, b"bar");
    assert!(!edit.is_insert());
    assert!(!edit.is_delete());
    assert!(!edit.is_empty());
}

#[test]
fn affected_range_spans_old_bytes() {
    let edit = ByteEdit::delete(10, b"abcde");
    assert_eq!(edit.affected_range(), 10..15);
}

#[test]
fn affected_range_empty_for_insert() {
    let edit = ByteEdit::insert(10, b"hello");
    assert_eq!(edit.affected_range(), 10..10);
}

#[test]
fn inverse_swaps_old_and_new() {
    let edit = ByteEdit::replace(5, b"old", b"new");
    let inv = edit.inverse();
    assert_eq!(inv.offset, 5);
    assert_eq!(inv.old_bytes, b"new");
    assert_eq!(inv.new_bytes, b"old");
}

#[test]
fn inverse_of_insert_is_delete() {
    let edit = ByteEdit::insert(0, b"data");
    let inv = edit.inverse();
    assert!(inv.is_delete());
    assert_eq!(inv.old_bytes, b"data");
}

#[test]
fn inverse_of_delete_is_insert() {
    let edit = ByteEdit::delete(0, b"data");
    let inv = edit.inverse();
    assert!(inv.is_insert());
    assert_eq!(inv.new_bytes, b"data");
}

#[test]
fn empty_edit() {
    let edit = ByteEdit {
        offset: 0,
        old_bytes: Vec::new(),
        new_bytes: Vec::new(),
    };
    assert!(edit.is_empty());
    assert!(!edit.is_insert());
    assert!(!edit.is_delete());
}

#[test]
fn double_inverse_is_identity() {
    let edit = ByteEdit::replace(42, b"hello", b"world");
    let double_inv = edit.inverse().inverse();
    assert_eq!(edit, double_inv);
}
