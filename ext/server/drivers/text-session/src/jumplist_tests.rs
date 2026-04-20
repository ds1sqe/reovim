use super::*;

fn test_buffer() -> BufferId {
    BufferId::new()
}

#[test]
fn test_empty_jumplist() {
    let mut list = Jumplist::new();
    assert!(list.is_empty());
    assert_eq!(list.len(), 0);
    assert_eq!(list.backward(), None);
    assert_eq!(list.forward(), None);
    assert_eq!(list.current(), None);
}

#[test]
fn test_push_and_backward() {
    let buf = test_buffer();
    let mut list = Jumplist::new();

    list.push(JumpEntry::new(buf, Position::new(0, 0)));
    list.push(JumpEntry::new(buf, Position::new(10, 5)));
    list.push(JumpEntry::new(buf, Position::new(20, 10)));

    assert_eq!(list.len(), 3);

    let entry = list.backward().unwrap();
    assert_eq!(entry.position, Position::new(20, 10));

    let entry = list.backward().unwrap();
    assert_eq!(entry.position, Position::new(10, 5));

    let entry = list.forward().unwrap();
    assert_eq!(entry.position, Position::new(10, 5));
}

#[test]
fn test_no_duplicate_entries() {
    let buf = test_buffer();
    let mut list = Jumplist::new();

    assert!(list.push(JumpEntry::new(buf, Position::new(5, 5))));
    assert!(!list.push(JumpEntry::new(buf, Position::new(5, 5))));
    assert!(!list.push(JumpEntry::new(buf, Position::new(5, 5))));

    assert_eq!(list.len(), 1);
}

#[test]
fn test_truncate_on_push_after_backward() {
    let buf = test_buffer();
    let mut list = Jumplist::new();

    list.push(JumpEntry::new(buf, Position::new(0, 0)));
    list.push(JumpEntry::new(buf, Position::new(10, 10)));
    list.push(JumpEntry::new(buf, Position::new(20, 20)));

    list.backward();
    list.backward();

    list.push(JumpEntry::new(buf, Position::new(5, 5)));

    assert_eq!(list.len(), 2);
    assert_eq!(list.entries()[1].position, Position::new(5, 5));
}

#[test]
fn test_push_current_preserves_history() {
    let buf = test_buffer();
    let mut list = Jumplist::new();

    list.push(JumpEntry::new(buf, Position::new(0, 0)));
    list.push(JumpEntry::new(buf, Position::new(10, 10)));

    list.backward();
    list.backward();

    list.push_current(JumpEntry::new(buf, Position::new(5, 5)));

    assert_eq!(list.len(), 3);
}

#[test]
fn test_max_size_enforcement() {
    let buf = test_buffer();
    let mut list = Jumplist::new();

    for i in 0..150 {
        list.push(JumpEntry::new(buf, Position::new(i, 0)));
    }

    assert_eq!(list.len(), MAX_JUMPLIST_SIZE);
    assert_eq!(list.entries()[0].position, Position::new(50, 0));
}

#[test]
fn test_forward_at_end_returns_none() {
    let buf = test_buffer();
    let mut list = Jumplist::new();

    list.push(JumpEntry::new(buf, Position::new(0, 0)));

    assert_eq!(list.forward(), None);
}

#[test]
fn test_backward_at_beginning_returns_none() {
    let buf = test_buffer();
    let mut list = Jumplist::new();

    list.push(JumpEntry::new(buf, Position::new(0, 0)));
    list.backward();

    assert_eq!(list.backward(), None);
}

#[test]
fn test_clear() {
    let buf = test_buffer();
    let mut list = Jumplist::new();

    list.push(JumpEntry::new(buf, Position::new(0, 0)));
    list.push(JumpEntry::new(buf, Position::new(10, 10)));

    list.clear();

    assert!(list.is_empty());
    assert_eq!(list.current_index(), 0);
}

#[test]
fn test_entries_slice() {
    let buf = test_buffer();
    let mut list = Jumplist::new();

    list.push(JumpEntry::new(buf, Position::new(0, 0)));
    list.push(JumpEntry::new(buf, Position::new(10, 10)));

    let entries = list.entries();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].position, Position::new(0, 0));
    assert_eq!(entries[1].position, Position::new(10, 10));
}

#[test]
fn test_different_buffers() {
    let buf1 = BufferId::new();
    let buf2 = BufferId::new();
    let mut list = Jumplist::new();

    list.push(JumpEntry::new(buf1, Position::new(0, 0)));
    list.push(JumpEntry::new(buf2, Position::new(10, 10)));
    list.push(JumpEntry::new(buf1, Position::new(20, 20)));

    assert_eq!(list.len(), 3);

    let entry = list.backward().unwrap();
    assert_eq!(entry.buffer, buf1);
    assert_eq!(entry.position, Position::new(20, 20));
}

#[test]
fn test_with_capacity() {
    let list = Jumplist::with_capacity(50);
    assert!(list.is_empty());
    assert_eq!(list.len(), 0);
}

#[test]
fn test_with_capacity_clamped() {
    let list = Jumplist::with_capacity(500);
    assert!(list.is_empty());
}

#[test]
fn test_push_current_duplicate() {
    let buf = test_buffer();
    let mut list = Jumplist::new();

    list.push(JumpEntry::new(buf, Position::new(5, 5)));
    assert!(!list.push_current(JumpEntry::new(buf, Position::new(5, 5))));
    assert_eq!(list.len(), 1);
}

#[test]
fn test_current_after_push() {
    let buf = test_buffer();
    let mut list = Jumplist::new();
    list.push(JumpEntry::new(buf, Position::new(10, 5)));

    let current = list.current();
    assert!(current.is_some());
    assert_eq!(current.unwrap().position, Position::new(10, 5));
}

#[test]
fn test_current_after_backward() {
    let buf = test_buffer();
    let mut list = Jumplist::new();
    list.push(JumpEntry::new(buf, Position::new(0, 0)));
    list.push(JumpEntry::new(buf, Position::new(10, 0)));

    list.backward();
    let current = list.current();
    assert!(current.is_some());
    assert_eq!(current.unwrap().position, Position::new(0, 0));
}

#[test]
fn test_push_current_max_size() {
    let buf = test_buffer();
    let mut list = Jumplist::new();

    for i in 0..150 {
        list.push_current(JumpEntry::new(buf, Position::new(i, 0)));
    }

    assert_eq!(list.len(), MAX_JUMPLIST_SIZE);
}
