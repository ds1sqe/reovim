use super::*;

#[test]
fn service_name() {
    assert_eq!(SearchKey::service_name(), "Search");
}

#[test]
fn debug() {
    assert_eq!(format!("{:?}", SearchKey::Regex), "Regex");
}

#[test]
fn clone_copy_eq() {
    let a = SearchKey::Regex;
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(SearchKey::Regex);
    set.insert(SearchKey::Regex);
    assert_eq!(set.len(), 1);
}
