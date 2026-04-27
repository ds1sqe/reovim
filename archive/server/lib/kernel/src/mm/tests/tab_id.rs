use super::*;

#[test]
fn test_tab_id_new() {
    let id1 = TabId::new();
    let id2 = TabId::new();
    assert_ne!(id1, id2);
    assert!(id2.as_usize() > id1.as_usize());
}

#[test]
fn test_tab_id_clone() {
    let id1 = TabId::new();
    let id2 = id1;
    assert_eq!(id1, id2);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_tab_id_display() {
    let id = TabId(42);
    assert_eq!(format!("{id}"), "tab:42");
}

#[test]
fn test_tab_id_default() {
    let id = TabId::default();
    assert!(id.as_usize() > 0);
}

#[test]
fn test_tab_id_from_raw() {
    let id = TabId::from_raw(42);
    assert_eq!(id.as_usize(), 42);
}

#[test]
fn test_tab_id_ordering() {
    let id1 = TabId::from_raw(1);
    let id2 = TabId::from_raw(2);
    let id3 = TabId::from_raw(3);

    // PartialOrd
    assert!(id1 < id2);
    assert!(id2 < id3);
    assert!(id1 < id3);

    // Ord (for sorting)
    let mut ids = vec![id3, id1, id2];
    ids.sort();
    assert_eq!(ids, vec![id1, id2, id3]);
}

#[test]
fn test_tab_id_hash() {
    use std::collections::HashSet;

    let id1 = TabId::from_raw(1);
    let id2 = TabId::from_raw(2);
    let id1_copy = TabId::from_raw(1);

    let mut set = HashSet::new();
    set.insert(id1);
    set.insert(id2);
    set.insert(id1_copy);

    assert_eq!(set.len(), 2);
    assert!(set.contains(&id1));
    assert!(set.contains(&id2));
}
