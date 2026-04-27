use super::*;

#[test]
fn test_service_key_impl() {
    assert_eq!(SessionHandlerKey::service_name(), "SessionHandler");
}

#[test]
fn test_equality() {
    assert_eq!(SessionHandlerKey::Empty, SessionHandlerKey::Empty);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_debug() {
    assert_eq!(format!("{:?}", SessionHandlerKey::Empty), "Empty");
}

#[test]
fn test_clone_copy() {
    let key = SessionHandlerKey::Empty;
    let cloned = key;
    assert_eq!(key, cloned);
}

#[test]
fn test_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(SessionHandlerKey::Empty);
    set.insert(SessionHandlerKey::Empty); // duplicate
    assert_eq!(set.len(), 1);
}
