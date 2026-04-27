use super::*;

#[test]
fn test_kind_capture_names() {
    assert_eq!(TextObjectKind::Function.capture_name(), "function");
    assert_eq!(TextObjectKind::Class.capture_name(), "class");
    assert_eq!(TextObjectKind::Argument.capture_name(), "argument");
    assert_eq!(TextObjectKind::Conditional.capture_name(), "conditional");
    assert_eq!(TextObjectKind::Loop.capture_name(), "loop");
    assert_eq!(TextObjectKind::Comment.capture_name(), "comment");
    assert_eq!(TextObjectKind::Block.capture_name(), "block");
}

#[test]
fn test_kind_display() {
    assert_eq!(format!("{}", TextObjectKind::Function), "function");
    assert_eq!(format!("{}", TextObjectKind::Class), "class");
}

#[test]
fn test_scope_suffix() {
    assert_eq!(TextObjectScope::Inner.suffix(), "inner");
    assert_eq!(TextObjectScope::Outer.suffix(), "outer");
}

#[test]
fn test_scope_display() {
    assert_eq!(format!("{}", TextObjectScope::Inner), "inner");
    assert_eq!(format!("{}", TextObjectScope::Outer), "outer");
}

#[test]
fn test_range_construction() {
    let range = TextObjectRange::new(10, 50, 1, 0, 3, 5);
    assert_eq!(range.start_byte, 10);
    assert_eq!(range.end_byte, 50);
    assert_eq!(range.start_row, 1);
    assert_eq!(range.start_col, 0);
    assert_eq!(range.end_row, 3);
    assert_eq!(range.end_col, 5);
}

#[test]
fn test_range_byte_len() {
    let range = TextObjectRange::new(10, 50, 0, 0, 0, 0);
    assert_eq!(range.byte_len(), 40);
}

#[test]
fn test_kind_eq_and_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(TextObjectKind::Function);
    set.insert(TextObjectKind::Function);
    assert_eq!(set.len(), 1);
}

#[test]
fn test_scope_eq() {
    assert_eq!(TextObjectScope::Inner, TextObjectScope::Inner);
    assert_ne!(TextObjectScope::Inner, TextObjectScope::Outer);
}

#[test]
fn test_range_eq() {
    let a = TextObjectRange::new(0, 10, 0, 0, 0, 10);
    let b = TextObjectRange::new(0, 10, 0, 0, 0, 10);
    assert_eq!(a, b);
}
