use super::*;

#[test]
fn from_raw_zero() {
    let id = BufferId::from_raw(0);
    assert_eq!(id.as_usize(), 0);
}

#[test]
fn from_raw_max() {
    let id = BufferId::from_raw(usize::MAX);
    assert_eq!(id.as_usize(), usize::MAX);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn display_zero() {
    let id = BufferId::from_raw(0);
    assert_eq!(format!("{id}"), "Buffer(0)");
}

#[test]
fn default_creates_unique_id() {
    let id1 = BufferId::default();
    let id2 = BufferId::default();
    assert_ne!(id1, id2);
}

#[test]
fn hash_consistent() {
    use std::collections::HashSet;

    let id1 = BufferId::from_raw(10);
    let id2 = BufferId::from_raw(20);
    let id1_dup = BufferId::from_raw(10);

    let mut set = HashSet::new();
    set.insert(id1);
    set.insert(id2);
    set.insert(id1_dup); // duplicate of id1

    assert_eq!(set.len(), 2);
}

#[test]
fn clone_and_copy() {
    let id = BufferId::from_raw(99);
    let cloned = id;
    assert_eq!(id, cloned);
    assert_eq!(id.as_usize(), cloned.as_usize());
}

#[test]
fn ordering_from_raw() {
    let a = BufferId::from_raw(1);
    let b = BufferId::from_raw(2);
    let c = BufferId::from_raw(3);
    assert!(a < b);
    assert!(b < c);
    assert!(a < c);
    assert!(c > a);
}

#[test]
fn eq_same_raw_value() {
    let a = BufferId::from_raw(42);
    let b = BufferId::from_raw(42);
    assert_eq!(a, b);
}

#[test]
fn ne_different_raw_value() {
    let a = BufferId::from_raw(1);
    let b = BufferId::from_raw(2);
    assert_ne!(a, b);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn debug_format() {
    let id = BufferId::from_raw(7);
    let debug = format!("{id:?}");
    assert!(debug.contains("BufferId"));
    assert!(debug.contains('7'));
}
