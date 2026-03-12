use super::*;

#[test]
fn service_name() {
    assert_eq!(UndoKey::service_name(), "Undo");
}

#[test]
fn debug() {
    assert_eq!(format!("{:?}", UndoKey::Buffer), "Buffer");
}

#[test]
fn clone_copy_eq() {
    let a = UndoKey::Buffer;
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(UndoKey::Buffer);
    set.insert(UndoKey::Buffer);
    assert_eq!(set.len(), 1);
}
