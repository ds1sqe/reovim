use std::borrow::Cow;

use std::sync::Arc;

use crate::mm::{Buffer, HeapMapping, LineIndex, VirtualBuffer};

use super::TextGeometry;

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

// ── VirtualBuffer ──────────────────────────────────────────────────────────

fn make_vbuf(content: &str) -> VirtualBuffer {
    let mapping = Arc::new(HeapMapping(content.as_bytes().to_vec()));
    let line_index = LineIndex::from_bytes(content.as_bytes()).unwrap();
    VirtualBuffer::new(mapping, line_index)
}

#[test]
fn vbuf_line_count() {
    let vbuf = make_vbuf("hello\nworld\nfoo");
    let tg: &dyn TextGeometry = &vbuf;
    assert_eq!(tg.line_count(), 3);
}

#[test]
fn vbuf_line_owned() {
    let vbuf = make_vbuf("hello\nworld");
    let tg: &dyn TextGeometry = &vbuf;
    let line = tg.line(0).unwrap();
    assert!(matches!(line, Cow::Owned(_)));
    assert_eq!(&*line, "hello");
}

#[test]
fn vbuf_line_out_of_bounds() {
    let vbuf = make_vbuf("hello");
    let tg: &dyn TextGeometry = &vbuf;
    assert!(tg.line(1).is_none());
}

#[test]
fn vbuf_line_len() {
    let vbuf = make_vbuf("hello\nab");
    let tg: &dyn TextGeometry = &vbuf;
    assert_eq!(tg.line_len(0), Some(5));
    assert_eq!(tg.line_len(1), Some(2));
    assert_eq!(tg.line_len(2), None);
}

#[test]
fn vbuf_is_empty() {
    let empty = make_vbuf("");
    let non_empty = make_vbuf("x");
    let tg_empty: &dyn TextGeometry = &empty;
    let tg_full: &dyn TextGeometry = &non_empty;
    assert!(tg_empty.is_empty());
    assert!(!tg_full.is_empty());
}

#[test]
fn vbuf_auto_coercion() {
    fn accepts_geometry(tg: &dyn TextGeometry) -> usize {
        tg.line_count()
    }
    let vbuf = make_vbuf("a\nb\nc");
    assert_eq!(accepts_geometry(&vbuf), 3);
}

// ── Polymorphism ───────────────────────────────────────────────────────────

#[test]
fn polymorphic_dispatch() {
    fn first_line(tg: &dyn TextGeometry) -> Option<String> {
        tg.line(0).map(Cow::into_owned)
    }

    let buf = Buffer::from_string("rope line");
    let vbuf = make_vbuf("virtual line");

    assert_eq!(first_line(&buf), Some("rope line".to_string()));
    assert_eq!(first_line(&vbuf), Some("virtual line".to_string()));
}

#[test]
fn polymorphic_line_count_matches() {
    let content = "line1\nline2\nline3";
    let buf = Buffer::from_string(content);
    let vbuf = make_vbuf(content);

    let buf_tg: &dyn TextGeometry = &buf;
    let vbuf_tg: &dyn TextGeometry = &vbuf;

    assert_eq!(buf_tg.line_count(), vbuf_tg.line_count());
    for i in 0..3 {
        assert_eq!(buf_tg.line(i).map(Cow::into_owned), vbuf_tg.line(i).map(Cow::into_owned),);
    }
}
