use std::borrow::Cow;

use reovim_types_text::TextGeometry;
use crate::mm::Buffer;

// ── Buffer (Rope) ──────────────────────────────────────────────────────────

#[test]
fn buffer_line_count() {
    let buf = Buffer::from_string("hello\nworld\nfoo");
    let tg: &dyn TextGeometry = &buf;
    assert_eq!(tg.line_count(), 3);
}

#[test]
fn buffer_line_zero_copy() {
    let buf = Buffer::from_string("hello\nworld");
    let tg: &dyn TextGeometry = &buf;
    let line = tg.line(0).unwrap();
    assert!(matches!(line, Cow::Borrowed(_)));
    assert_eq!(&*line, "hello");
}

#[test]
fn buffer_line_out_of_bounds() {
    let buf = Buffer::from_string("hello");
    let tg: &dyn TextGeometry = &buf;
    assert!(tg.line(1).is_none());
}

#[test]
fn buffer_line_len() {
    let buf = Buffer::from_string("hello\nab");
    let tg: &dyn TextGeometry = &buf;
    assert_eq!(tg.line_len(0), Some(5));
    assert_eq!(tg.line_len(1), Some(2));
    assert_eq!(tg.line_len(2), None);
}

#[test]
fn buffer_is_empty() {
    let empty = Buffer::new();
    let non_empty = Buffer::from_string("x");
    let tg_empty: &dyn TextGeometry = &empty;
    let tg_full: &dyn TextGeometry = &non_empty;
    assert!(tg_empty.is_empty());
    assert!(!tg_full.is_empty());
}

#[test]
fn buffer_auto_coercion() {
    // &Buffer auto-coerces to &dyn TextGeometry — zero test rewrite guarantee
    fn accepts_geometry(tg: &dyn TextGeometry) -> usize {
        tg.line_count()
    }
    let buf = Buffer::from_string("a\nb\nc");
    assert_eq!(accepts_geometry(&buf), 3);
}

// VirtualBuffer TextGeometry tests moved to reovim-provider-text (#740)
