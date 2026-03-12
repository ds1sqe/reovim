use super::*;

#[test]
fn test_window_id_new() {
    let id1 = WindowId::new();
    let id2 = WindowId::new();
    assert_ne!(id1, id2);
    assert!(id2.as_usize() > id1.as_usize());
}

#[test]
fn test_window_id_clone() {
    let id1 = WindowId::new();
    let id2 = id1;
    assert_eq!(id1, id2);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_window_id_display() {
    let id = WindowId(42);
    assert_eq!(format!("{id}"), "window:42");
}

#[test]
fn test_window_id_default() {
    let id = WindowId::default();
    assert!(id.as_usize() > 0);
}

#[test]
fn test_window_id_from_raw() {
    let id = WindowId::from_raw(42);
    assert_eq!(id.as_usize(), 42);
}

#[test]
fn test_window_id_ordering() {
    let id1 = WindowId::from_raw(1);
    let id2 = WindowId::from_raw(2);
    let id3 = WindowId::from_raw(3);

    // PartialOrd
    assert!(id1 < id2);
    assert!(id2 < id3);
    assert!(id1 < id3);

    // Ord (for sorting)
    let mut ids = vec![id3, id1, id2];
    ids.sort();
    assert_eq!(ids, vec![id1, id2, id3]);
}
