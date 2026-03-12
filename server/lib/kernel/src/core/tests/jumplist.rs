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

    // Jump backward twice
    let entry = list.backward().unwrap();
    assert_eq!(entry.position, Position::new(20, 10));

    let entry = list.backward().unwrap();
    assert_eq!(entry.position, Position::new(10, 5));

    // Jump forward once
    let entry = list.forward().unwrap();
    assert_eq!(entry.position, Position::new(10, 5));
}

#[test]
fn test_no_duplicate_entries() {
    let buf = test_buffer();
    let mut list = Jumplist::new();

    assert!(list.push(JumpEntry::new(buf, Position::new(5, 5))));
    assert!(!list.push(JumpEntry::new(buf, Position::new(5, 5)))); // Duplicate
    assert!(!list.push(JumpEntry::new(buf, Position::new(5, 5)))); // Duplicate

    assert_eq!(list.len(), 1);
}

#[test]
fn test_truncate_on_push_after_backward() {
    let buf = test_buffer();
    let mut list = Jumplist::new();

    list.push(JumpEntry::new(buf, Position::new(0, 0)));
    list.push(JumpEntry::new(buf, Position::new(10, 10)));
    list.push(JumpEntry::new(buf, Position::new(20, 20)));

    // Jump back twice
    list.backward();
    list.backward();

    // Push new entry - should truncate future entries
    list.push(JumpEntry::new(buf, Position::new(5, 5)));

    assert_eq!(list.len(), 2); // Only first entry + new entry
    assert_eq!(list.entries()[1].position, Position::new(5, 5));
}

#[test]
fn test_push_current_preserves_history() {
    let buf = test_buffer();
    let mut list = Jumplist::new();

    list.push(JumpEntry::new(buf, Position::new(0, 0)));
    list.push(JumpEntry::new(buf, Position::new(10, 10)));

    // Jump back
    list.backward();
    list.backward();

    // Push current - should NOT truncate
    list.push_current(JumpEntry::new(buf, Position::new(5, 5)));

    assert_eq!(list.len(), 3);
}

#[test]
fn test_max_size_enforcement() {
    let buf = test_buffer();
    let mut list = Jumplist::new();

    // Push more than MAX_JUMPLIST_SIZE entries
    for i in 0..150 {
        list.push(JumpEntry::new(buf, Position::new(i, 0)));
    }

    assert_eq!(list.len(), MAX_JUMPLIST_SIZE);
    // First entry should be the 51st one pushed (index 50)
    assert_eq!(list.entries()[0].position, Position::new(50, 0));
}

#[test]
fn test_forward_at_end_returns_none() {
    let buf = test_buffer();
    let mut list = Jumplist::new();

    list.push(JumpEntry::new(buf, Position::new(0, 0)));

    // At end, forward should return None
    assert_eq!(list.forward(), None);
}

#[test]
fn test_backward_at_beginning_returns_none() {
    let buf = test_buffer();
    let mut list = Jumplist::new();

    list.push(JumpEntry::new(buf, Position::new(0, 0)));
    list.backward(); // Go to first

    // At beginning, backward should return None
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

// === with_capacity ===

#[test]
fn test_with_capacity() {
    let list = Jumplist::with_capacity(50);
    assert!(list.is_empty());
    assert_eq!(list.len(), 0);
}

#[test]
fn test_with_capacity_clamped() {
    // capacity clamped to MAX_JUMPLIST_SIZE
    let list = Jumplist::with_capacity(500);
    assert!(list.is_empty());
}

// === push_current duplicate ===

#[test]
fn test_push_current_duplicate() {
    let buf = test_buffer();
    let mut list = Jumplist::new();

    list.push(JumpEntry::new(buf, Position::new(5, 5)));
    // push_current with same entry as last should return false
    assert!(!list.push_current(JumpEntry::new(buf, Position::new(5, 5))));
    assert_eq!(list.len(), 1);
}

// === current() non-empty ===

#[test]
fn test_current_after_push() {
    let buf = test_buffer();
    let mut list = Jumplist::new();
    list.push(JumpEntry::new(buf, Position::new(10, 5)));

    // current() should return the most recent entry
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

    // After two pushes, current = 2 (past end)
    // backward() decrements to current = 1
    // current() returns entries.get(1-1) = entries[0]
    list.backward();
    let current = list.current();
    assert!(current.is_some());
    assert_eq!(current.unwrap().position, Position::new(0, 0));
}

// === max size with push_current ===

#[test]
fn test_push_current_max_size() {
    let buf = test_buffer();
    let mut list = Jumplist::new();

    for i in 0..150 {
        list.push_current(JumpEntry::new(buf, Position::new(i, 0)));
    }

    assert_eq!(list.len(), MAX_JUMPLIST_SIZE);
}
