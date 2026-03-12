use super::*;

#[test]
fn service_name() {
    assert_eq!(ClipboardKey::service_name(), "Clipboard");
}

#[test]
fn debug() {
    let key = ClipboardKey::Default;
    assert_eq!(format!("{key:?}"), "Default");
}

#[test]
fn clone_copy_eq() {
    let a = ClipboardKey::Default;
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(ClipboardKey::Default);
    set.insert(ClipboardKey::Default);
    assert_eq!(set.len(), 1);
}
