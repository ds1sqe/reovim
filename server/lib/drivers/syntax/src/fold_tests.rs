use super::*;

#[test]
fn test_fold_kind_is_definition() {
    assert!(FoldKind::Function.is_definition());
    assert!(FoldKind::Class.is_definition());
    assert!(!FoldKind::Import.is_definition());
    assert!(!FoldKind::Comment.is_definition());
    assert!(!FoldKind::Block.is_definition());
}

#[test]
fn test_fold_range_new() {
    let fold = FoldRange::new(5, 10, FoldKind::Function, "fn foo() {");

    assert_eq!(fold.start_line, 5);
    assert_eq!(fold.end_line, 10);
    assert_eq!(fold.kind, FoldKind::Function);
    assert_eq!(fold.preview, "fn foo() {");
}

#[test]
fn test_fold_range_without_preview() {
    let fold = FoldRange::without_preview(5, 10, FoldKind::Block);

    assert_eq!(fold.start_line, 5);
    assert_eq!(fold.end_line, 10);
    assert_eq!(fold.kind, FoldKind::Block);
    assert!(fold.preview.is_empty());
}

#[test]
fn test_fold_range_is_foldable() {
    let foldable = FoldRange::new(5, 10, FoldKind::Function, "");
    assert!(foldable.is_foldable());

    let single = FoldRange::new(5, 5, FoldKind::Block, "");
    assert!(!single.is_foldable());
}

#[test]
fn test_fold_range_line_count() {
    let fold = FoldRange::new(5, 10, FoldKind::Function, "");
    assert_eq!(fold.line_count(), 6); // 5, 6, 7, 8, 9, 10

    let single = FoldRange::new(5, 5, FoldKind::Block, "");
    assert_eq!(single.line_count(), 1);
}

#[test]
fn test_fold_range_hidden_lines() {
    let fold = FoldRange::new(5, 10, FoldKind::Function, "");
    assert_eq!(fold.hidden_lines(), 5); // Lines 6-10 are hidden

    let single = FoldRange::new(5, 5, FoldKind::Block, "");
    assert_eq!(single.hidden_lines(), 0);
}

#[test]
fn test_fold_range_contains_line() {
    let fold = FoldRange::new(5, 10, FoldKind::Function, "");

    assert!(fold.contains_line(5));
    assert!(fold.contains_line(7));
    assert!(fold.contains_line(10));
    assert!(!fold.contains_line(4));
    assert!(!fold.contains_line(11));
}

#[test]
fn test_fold_range_overlaps() {
    let fold1 = FoldRange::new(5, 10, FoldKind::Function, "");
    let fold2 = FoldRange::new(8, 15, FoldKind::Block, "");
    let fold3 = FoldRange::new(11, 15, FoldKind::Block, "");

    assert!(fold1.overlaps(&fold2)); // 8-10 overlap
    assert!(fold2.overlaps(&fold1));
    assert!(!fold1.overlaps(&fold3)); // No overlap
    assert!(!fold3.overlaps(&fold1));
}

#[test]
fn test_fold_range_contains() {
    let outer = FoldRange::new(5, 15, FoldKind::Class, "");
    let inner = FoldRange::new(7, 12, FoldKind::Function, "");
    let overlapping = FoldRange::new(10, 20, FoldKind::Block, "");

    assert!(outer.contains(&inner));
    assert!(!inner.contains(&outer));
    assert!(!outer.contains(&overlapping));
}
