use crate::{FullBlockRasterizer, ViewHint, ViewRasterizer};

#[test]
fn view_hint_default_is_full_block() {
    assert_eq!(ViewHint::default(), ViewHint::FullBlock);
}

#[test]
fn view_hint_debug_covers_all_variants() {
    assert!(format!("{:?}", ViewHint::FullBlock).contains("FullBlock"));
    assert!(format!("{:?}", ViewHint::HalfBlock).contains("HalfBlock"));
    assert!(format!("{:?}", ViewHint::Braille).contains("Braille"));
}

#[test]
fn view_hint_variants_are_distinct() {
    assert_ne!(ViewHint::FullBlock, ViewHint::HalfBlock);
    assert_ne!(ViewHint::FullBlock, ViewHint::Braille);
    assert_ne!(ViewHint::HalfBlock, ViewHint::Braille);
}

#[test]
fn view_hint_copy_equality() {
    let a = ViewHint::FullBlock;
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn full_block_reports_its_hint() {
    let r = FullBlockRasterizer::new();
    assert_eq!(r.hint(), ViewHint::FullBlock);
}
