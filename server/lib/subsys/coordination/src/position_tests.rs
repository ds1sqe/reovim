use super::*;

// --- PositionHeader tests ---

#[test]
fn header_roundtrip() {
    let h = PositionHeader::new(42, 7, 0xFF00);
    assert_eq!(h.domain_id(), 42);
    assert_eq!(h.inner_id(), 7);
    assert_eq!(h.flags(), 0xFF00);
}

#[test]
fn header_zero_values() {
    let h = PositionHeader::new(0, 0, 0);
    assert_eq!(h.domain_id(), 0);
    assert_eq!(h.inner_id(), 0);
    assert_eq!(h.flags(), 0);
}

#[test]
fn header_max_values() {
    let h = PositionHeader::new(u32::MAX, u16::MAX, u16::MAX);
    assert_eq!(h.domain_id(), u32::MAX);
    assert_eq!(h.inner_id(), u16::MAX);
    assert_eq!(h.flags(), u16::MAX);
}

#[test]
fn header_endian_correctness() {
    let h = PositionHeader::new(0x0102_0304, 0x0506, 0x0708);
    let bytes = h.as_bytes();
    // Little-endian: least significant byte first
    assert_eq!(bytes[0], 0x04); // domain_id LE
    assert_eq!(bytes[1], 0x03);
    assert_eq!(bytes[2], 0x02);
    assert_eq!(bytes[3], 0x01);
    assert_eq!(bytes[4], 0x06); // inner_id LE
    assert_eq!(bytes[5], 0x05);
    assert_eq!(bytes[6], 0x08); // flags LE
    assert_eq!(bytes[7], 0x07);
}

#[test]
fn header_same_domain() {
    let a = PositionHeader::new(1, 0, 0);
    let b = PositionHeader::new(1, 5, 0xFF);
    assert!(a.same_domain(&b));
}

#[test]
fn header_different_domain() {
    let a = PositionHeader::new(1, 0, 0);
    let b = PositionHeader::new(2, 0, 0);
    assert!(!a.same_domain(&b));
}

#[test]
fn header_equality() {
    let a = PositionHeader::new(1, 2, 3);
    let b = PositionHeader::new(1, 2, 3);
    let c = PositionHeader::new(1, 2, 4);
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn header_clone_copy() {
    let a = PositionHeader::new(10, 20, 30);
    let b = a;
    let c = a;
    assert_eq!(a, b);
    assert_eq!(b, c);
}

#[test]
fn header_hash_consistent() {
    use std::collections::HashSet;
    let a = PositionHeader::new(1, 2, 3);
    let b = PositionHeader::new(1, 2, 3);
    let c = PositionHeader::new(4, 5, 6);
    let mut set = HashSet::new();
    set.insert(a);
    assert!(set.contains(&b));
    assert!(!set.contains(&c));
}

#[test]
fn header_debug_format() {
    let h = PositionHeader::new(1, 0, 0);
    let debug = format!("{h:?}");
    assert!(debug.contains("PositionHeader"));
    assert!(debug.contains("domain_id: 1"));
    assert!(debug.contains("inner_id: 0"));
    assert!(debug.contains("flags: 0"));
}

#[test]
fn header_sentinel_domain_id() {
    assert_eq!(PositionHeader::SENTINEL_DOMAIN_ID, 0);
    let h = PositionHeader::new(PositionHeader::SENTINEL_DOMAIN_ID, 0, 0);
    assert_eq!(h.domain_id(), 0);
}

// --- Position trait tests ---

/// Mock position for testing the trait.
#[derive(Clone, PartialEq, Eq)]
struct MockPosition {
    header: PositionHeader,
    data: Vec<u8>,
}

impl MockPosition {
    fn new(domain_id: u32, inner_id: u16, data: Vec<u8>) -> Self {
        Self {
            header: PositionHeader::new(domain_id, inner_id, 0),
            data,
        }
    }
}

impl Position for MockPosition {
    fn header(&self) -> &PositionHeader {
        &self.header
    }

    fn content(&self) -> &[u8] {
        &self.data
    }

    fn display(&self) -> String {
        format!("mock({:?})", self.data)
    }

    fn clone_box(&self) -> Box<dyn Position> {
        Box::new(self.clone())
    }
}

/// Second mock type to test cross-type equality.
#[derive(Clone)]
struct AltPosition {
    header: PositionHeader,
    data: Vec<u8>,
}

impl Position for AltPosition {
    fn header(&self) -> &PositionHeader {
        &self.header
    }

    fn content(&self) -> &[u8] {
        &self.data
    }

    fn display(&self) -> String {
        format!("alt({:?})", self.data)
    }

    fn clone_box(&self) -> Box<dyn Position> {
        Box::new(self.clone())
    }
}

#[test]
fn trait_object_safety() {
    let _: Box<dyn Position> = Box::new(MockPosition::new(1, 0, vec![1, 2]));
}

#[test]
fn trait_eq_reflexive() {
    let a: Box<dyn Position> = Box::new(MockPosition::new(1, 0, vec![1, 2]));
    assert_eq!(*a, *a);
}

#[test]
fn trait_eq_symmetric() {
    let a: Box<dyn Position> = Box::new(MockPosition::new(1, 0, vec![1, 2]));
    let b: Box<dyn Position> = Box::new(MockPosition::new(1, 0, vec![1, 2]));
    assert_eq!(*a, *b);
    assert_eq!(*b, *a);
}

#[test]
fn trait_eq_different_content() {
    let a: Box<dyn Position> = Box::new(MockPosition::new(1, 0, vec![1, 2]));
    let b: Box<dyn Position> = Box::new(MockPosition::new(1, 0, vec![3, 4]));
    assert_ne!(*a, *b);
}

#[test]
fn trait_eq_different_header() {
    let a: Box<dyn Position> = Box::new(MockPosition::new(1, 0, vec![1, 2]));
    let b: Box<dyn Position> = Box::new(MockPosition::new(2, 0, vec![1, 2]));
    assert_ne!(*a, *b);
}

#[test]
fn trait_eq_cross_type_same_bytes() {
    // Two different concrete types with identical header + content are equal
    let a: Box<dyn Position> = Box::new(MockPosition::new(1, 0, vec![1, 2]));
    let b: Box<dyn Position> = Box::new(AltPosition {
        header: PositionHeader::new(1, 0, 0),
        data: vec![1, 2],
    });
    assert_eq!(*a, *b);
}

#[test]
fn trait_clone_box() {
    let a: Box<dyn Position> = Box::new(MockPosition::new(1, 0, vec![5, 6]));
    let b = a.clone_box();
    assert_eq!(*a, *b);
}

#[test]
fn trait_encode_default() {
    let p = MockPosition::new(1, 0, vec![0xAA, 0xBB]);
    let encoded = p.encode();
    assert_eq!(encoded.len(), 10); // 8 header + 2 content
    assert_eq!(&encoded[..8], p.header().as_bytes());
    assert_eq!(&encoded[8..], &[0xAA, 0xBB]);
}

#[test]
fn trait_encode_empty_content() {
    let p = MockPosition::new(1, 0, vec![]);
    let encoded = p.encode();
    assert_eq!(encoded.len(), 8); // header only
}

#[test]
fn trait_display() {
    let p = MockPosition::new(1, 0, vec![1, 2, 3]);
    assert_eq!(p.display(), "mock([1, 2, 3])");
}

#[test]
fn trait_eq_empty_content() {
    let a: Box<dyn Position> = Box::new(MockPosition::new(1, 0, vec![]));
    let b: Box<dyn Position> = Box::new(MockPosition::new(1, 0, vec![]));
    assert_eq!(*a, *b);
}

#[test]
fn trait_eq_different_inner_id() {
    let a: Box<dyn Position> = Box::new(MockPosition::new(1, 0, vec![1]));
    let b: Box<dyn Position> = Box::new(MockPosition::new(1, 1, vec![1]));
    assert_ne!(*a, *b);
}
