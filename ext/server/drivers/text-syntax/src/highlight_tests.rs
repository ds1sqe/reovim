use super::*;

// ========================================================================
// HighlightCategory Tests
// ========================================================================

#[test]
fn test_highlight_category_new_from_str() {
    let cat = HighlightCategory::new("keyword.function");
    assert_eq!(cat.as_str(), "keyword.function");
}

#[test]
fn test_highlight_category_new_from_string() {
    let s = String::from("variable.builtin");
    let cat = HighlightCategory::new(s);
    assert_eq!(cat.as_str(), "variable.builtin");
}

#[test]
fn test_highlight_category_new_from_arc_str() {
    let arc: Arc<str> = Arc::from("type.builtin");
    let cat = HighlightCategory::new(arc);
    assert_eq!(cat.as_str(), "type.builtin");
}

#[test]
fn test_highlight_category_equality_same_arc() {
    let cat1 = HighlightCategory::new("keyword");
    let cat2 = cat1.clone();
    assert_eq!(cat1, cat2);
}

#[test]
fn test_highlight_category_equality_different_arcs() {
    let cat1 = HighlightCategory::new(String::from("keyword"));
    let cat2 = HighlightCategory::new(String::from("keyword"));
    assert_eq!(cat1, cat2);
}

#[test]
fn test_highlight_category_inequality() {
    let cat1 = HighlightCategory::new("keyword");
    let cat2 = HighlightCategory::new("function");
    assert_ne!(cat1, cat2);
}

#[test]
fn test_highlight_category_hash_consistency() {
    use std::collections::HashSet;
    let cat1 = HighlightCategory::new(String::from("keyword"));
    let cat2 = HighlightCategory::new(String::from("keyword"));
    let mut set = HashSet::new();
    set.insert(cat1);
    assert!(set.contains(&cat2));
}

#[test]
fn test_highlight_category_display() {
    let cat = HighlightCategory::new("keyword.function");
    assert_eq!(format!("{cat}"), "keyword.function");
}

#[test]
fn test_highlight_category_debug() {
    let cat = HighlightCategory::new("keyword");
    let debug = format!("{cat:?}");
    assert!(debug.contains("keyword"));
}

// ========================================================================
// Annotation Tests
// ========================================================================

#[test]
fn test_annotation_new() {
    let ann = Annotation::new(10, 20, HighlightCategory::new("keyword"));
    assert_eq!(ann.start_byte, 10);
    assert_eq!(ann.end_byte, 20);
    assert_eq!(ann.category.as_str(), "keyword");
}

#[test]
fn test_annotation_len() {
    let ann = Annotation::new(10, 20, HighlightCategory::new("keyword"));
    assert_eq!(ann.len(), 10);
}

#[test]
fn test_annotation_is_empty() {
    let ann = Annotation::new(5, 5, HighlightCategory::new("keyword"));
    assert!(ann.is_empty());

    let ann = Annotation::new(5, 10, HighlightCategory::new("keyword"));
    assert!(!ann.is_empty());
}

#[test]
fn test_annotation_overlaps() {
    let ann = Annotation::new(10, 20, HighlightCategory::new("keyword"));
    assert!(ann.overlaps(&(5..15)));
    assert!(ann.overlaps(&(15..25)));
    assert!(ann.overlaps(&(12..18)));
    assert!(!ann.overlaps(&(0..10)));
    assert!(!ann.overlaps(&(20..30)));
}

#[test]
fn test_annotation_contains() {
    let ann = Annotation::new(10, 20, HighlightCategory::new("keyword"));
    assert!(ann.contains(10));
    assert!(ann.contains(15));
    assert!(ann.contains(19));
    assert!(!ann.contains(9));
    assert!(!ann.contains(20));
}

#[test]
fn test_annotation_byte_range() {
    let ann = Annotation::new(10, 20, HighlightCategory::new("keyword"));
    assert_eq!(ann.byte_range(), 10..20);
}

#[test]
fn test_annotation_clone() {
    let ann = Annotation::new(0, 5, HighlightCategory::new("keyword"));
    let cloned = ann.clone();
    assert_eq!(ann, cloned);
}

#[test]
fn test_annotation_debug() {
    let ann = Annotation::new(0, 5, HighlightCategory::new("keyword"));
    let debug = format!("{ann:?}");
    assert!(debug.contains("Annotation"));
    assert!(debug.contains("keyword"));
}

#[test]
fn test_annotation_equality() {
    let a1 = Annotation::new(0, 5, HighlightCategory::new("keyword"));
    let a2 = Annotation::new(0, 5, HighlightCategory::new("keyword"));
    let a3 = Annotation::new(0, 5, HighlightCategory::new("function"));
    assert_eq!(a1, a2);
    assert_ne!(a1, a3);
}
