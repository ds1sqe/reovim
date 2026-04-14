use super::*;

// --- CursorHeader tests ---

#[test]
fn header_roundtrip() {
    let h = CursorHeader::new(42, 7, 0xFF00);
    assert_eq!(h.domain_id(), 42);
    assert_eq!(h.inner_id(), 7);
    assert_eq!(h.flags(), 0xFF00);
}

#[test]
fn header_zero_values() {
    let h = CursorHeader::new(0, 0, 0);
    assert_eq!(h.domain_id(), 0);
    assert_eq!(h.inner_id(), 0);
    assert_eq!(h.flags(), 0);
}

#[test]
fn header_max_values() {
    let h = CursorHeader::new(u32::MAX, u16::MAX, u16::MAX);
    assert_eq!(h.domain_id(), u32::MAX);
    assert_eq!(h.inner_id(), u16::MAX);
    assert_eq!(h.flags(), u16::MAX);
}

#[test]
fn header_endian_correctness() {
    let h = CursorHeader::new(0x0102_0304, 0x0506, 0x0708);
    let bytes = h.as_bytes();
    assert_eq!(bytes[0], 0x04);
    assert_eq!(bytes[1], 0x03);
    assert_eq!(bytes[2], 0x02);
    assert_eq!(bytes[3], 0x01);
    assert_eq!(bytes[4], 0x06);
    assert_eq!(bytes[5], 0x05);
    assert_eq!(bytes[6], 0x08);
    assert_eq!(bytes[7], 0x07);
}

#[test]
fn header_same_domain() {
    let a = CursorHeader::new(1, 0, 0);
    let b = CursorHeader::new(1, 5, 0xFF);
    assert!(a.same_domain(&b));
}

#[test]
fn header_different_domain() {
    let a = CursorHeader::new(1, 0, 0);
    let b = CursorHeader::new(2, 0, 0);
    assert!(!a.same_domain(&b));
}

#[test]
fn header_equality() {
    let a = CursorHeader::new(1, 2, 3);
    let b = CursorHeader::new(1, 2, 3);
    let c = CursorHeader::new(1, 2, 4);
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn header_clone_copy() {
    let a = CursorHeader::new(10, 20, 30);
    let b = a;
    let c = a;
    assert_eq!(a, b);
    assert_eq!(b, c);
}

#[test]
fn header_hash_consistent() {
    use std::collections::HashSet;
    let a = CursorHeader::new(1, 2, 3);
    let b = CursorHeader::new(1, 2, 3);
    let c = CursorHeader::new(4, 5, 6);
    let mut set = HashSet::new();
    set.insert(a);
    assert!(set.contains(&b));
    assert!(!set.contains(&c));
}

#[test]
fn header_debug_format() {
    let h = CursorHeader::new(2, 1, 0);
    let debug = format!("{h:?}");
    assert!(debug.contains("CursorHeader"));
    assert!(debug.contains("domain_id: 2"));
    assert!(debug.contains("inner_id: 1"));
}

#[test]
fn header_sentinel_domain_id() {
    assert_eq!(CursorHeader::SENTINEL_DOMAIN_ID, 0);
    let h = CursorHeader::new(CursorHeader::SENTINEL_DOMAIN_ID, 0, 0);
    assert_eq!(h.domain_id(), 0);
}

// --- Cursor trait tests ---

#[derive(Clone, PartialEq, Eq)]
struct MockCursor {
    header: CursorHeader,
    data: Vec<u8>,
}

impl MockCursor {
    fn new(domain_id: u32, inner_id: u16, data: Vec<u8>) -> Self {
        Self {
            header: CursorHeader::new(domain_id, inner_id, 0),
            data,
        }
    }
}

impl Cursor for MockCursor {
    fn header(&self) -> &CursorHeader {
        &self.header
    }

    fn content(&self) -> &[u8] {
        &self.data
    }

    fn display(&self) -> String {
        format!("mock-cursor({:?})", self.data)
    }

    fn clone_box(&self) -> Box<dyn Cursor> {
        Box::new(self.clone())
    }
}

#[derive(Clone)]
struct AltCursor {
    header: CursorHeader,
    data: Vec<u8>,
}

impl Cursor for AltCursor {
    fn header(&self) -> &CursorHeader {
        &self.header
    }

    fn content(&self) -> &[u8] {
        &self.data
    }

    fn display(&self) -> String {
        format!("alt-cursor({:?})", self.data)
    }

    fn clone_box(&self) -> Box<dyn Cursor> {
        Box::new(self.clone())
    }
}

#[test]
fn trait_object_safety() {
    let _: Box<dyn Cursor> = Box::new(MockCursor::new(1, 0, vec![1, 2]));
}

#[test]
fn trait_eq_reflexive() {
    let a: Box<dyn Cursor> = Box::new(MockCursor::new(1, 0, vec![1, 2]));
    assert_eq!(*a, *a);
}

#[test]
fn trait_eq_symmetric() {
    let a: Box<dyn Cursor> = Box::new(MockCursor::new(1, 0, vec![1, 2]));
    let b: Box<dyn Cursor> = Box::new(MockCursor::new(1, 0, vec![1, 2]));
    assert_eq!(*a, *b);
    assert_eq!(*b, *a);
}

#[test]
fn trait_eq_different_content() {
    let a: Box<dyn Cursor> = Box::new(MockCursor::new(1, 0, vec![1, 2]));
    let b: Box<dyn Cursor> = Box::new(MockCursor::new(1, 0, vec![3, 4]));
    assert_ne!(*a, *b);
}

#[test]
fn trait_eq_different_header() {
    let a: Box<dyn Cursor> = Box::new(MockCursor::new(1, 0, vec![1, 2]));
    let b: Box<dyn Cursor> = Box::new(MockCursor::new(2, 0, vec![1, 2]));
    assert_ne!(*a, *b);
}

#[test]
fn trait_eq_cross_type_same_bytes() {
    let a: Box<dyn Cursor> = Box::new(MockCursor::new(1, 0, vec![1, 2]));
    let b: Box<dyn Cursor> = Box::new(AltCursor {
        header: CursorHeader::new(1, 0, 0),
        data: vec![1, 2],
    });
    assert_eq!(*a, *b);
}

#[test]
fn trait_clone_box() {
    let a: Box<dyn Cursor> = Box::new(MockCursor::new(1, 0, vec![5, 6]));
    let b = a.clone_box();
    assert_eq!(*a, *b);
}

#[test]
fn trait_encode_default() {
    let c = MockCursor::new(1, 0, vec![0xAA, 0xBB]);
    let encoded = c.encode();
    assert_eq!(encoded.len(), 10);
    assert_eq!(&encoded[..8], c.header().as_bytes());
    assert_eq!(&encoded[8..], &[0xAA, 0xBB]);
}

#[test]
fn trait_encode_empty_content() {
    let c = MockCursor::new(1, 0, vec![]);
    let encoded = c.encode();
    assert_eq!(encoded.len(), 8);
}

#[test]
fn trait_display() {
    let c = MockCursor::new(1, 0, vec![1, 2, 3]);
    assert_eq!(c.display(), "mock-cursor([1, 2, 3])");
}

#[test]
fn trait_eq_empty_content() {
    let a: Box<dyn Cursor> = Box::new(MockCursor::new(1, 0, vec![]));
    let b: Box<dyn Cursor> = Box::new(MockCursor::new(1, 0, vec![]));
    assert_eq!(*a, *b);
}

#[test]
fn trait_eq_different_inner_id() {
    let a: Box<dyn Cursor> = Box::new(MockCursor::new(1, 0, vec![1]));
    let b: Box<dyn Cursor> = Box::new(MockCursor::new(1, 1, vec![1]));
    assert_ne!(*a, *b);
}
