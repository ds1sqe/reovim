use super::*;

// --- Mock types for codec roundtrip testing ---

#[derive(Clone, PartialEq, Eq)]
struct TestPosition {
    header: PositionHeader,
    value: u32,
    encoded: [u8; 4],
}

impl TestPosition {
    fn new(domain_id: u32, value: u32) -> Self {
        Self {
            header: PositionHeader::new(domain_id, 0, 0),
            value,
            encoded: value.to_le_bytes(),
        }
    }
}

impl Position for TestPosition {
    fn header(&self) -> &PositionHeader {
        &self.header
    }

    fn content(&self) -> &[u8] {
        &self.encoded
    }

    fn display(&self) -> String {
        format!("test-pos({})", self.value)
    }

    fn clone_box(&self) -> Box<dyn Position> {
        Box::new(self.clone())
    }
}

struct TestPositionCodec {
    domain_id: u32,
}

impl PositionCodec for TestPositionCodec {
    fn domain_id(&self) -> u32 {
        self.domain_id
    }

    fn inner_id(&self) -> u16 {
        0
    }

    fn decode(&self, header: &PositionHeader, content: &[u8]) -> Option<Box<dyn Position>> {
        if content.len() != 4 {
            return None;
        }
        let value = u32::from_le_bytes([content[0], content[1], content[2], content[3]]);
        let encoded = value.to_le_bytes();
        Some(Box::new(TestPosition {
            header: *header,
            value,
            encoded,
        }))
    }
}

#[derive(Clone, PartialEq, Eq)]
struct TestCursor {
    header: CursorHeader,
    data: Vec<u8>,
}

impl TestCursor {
    fn new(domain_id: u32, inner_id: u16, data: Vec<u8>) -> Self {
        Self {
            header: CursorHeader::new(domain_id, inner_id, 0),
            data,
        }
    }
}

impl Cursor for TestCursor {
    fn header(&self) -> &CursorHeader {
        &self.header
    }

    fn content(&self) -> &[u8] {
        &self.data
    }

    fn display(&self) -> String {
        format!("test-cursor({:?})", self.data)
    }

    fn clone_box(&self) -> Box<dyn Cursor> {
        Box::new(self.clone())
    }
}

struct TestCursorCodec {
    domain_id: u32,
    inner_id: u16,
}

impl CursorCodec for TestCursorCodec {
    fn domain_id(&self) -> u32 {
        self.domain_id
    }

    fn inner_id(&self) -> u16 {
        self.inner_id
    }

    fn decode(&self, header: &CursorHeader, content: &[u8]) -> Option<Box<dyn Cursor>> {
        Some(Box::new(TestCursor {
            header: *header,
            data: content.to_vec(),
        }))
    }
}

// --- PositionCodec tests ---

#[test]
fn position_codec_object_safety() {
    let _: Box<dyn PositionCodec> = Box::new(TestPositionCodec { domain_id: 1 });
}

#[test]
fn position_codec_roundtrip() {
    let original = TestPosition::new(1, 42);
    let encoded = original.encode();

    let codec = TestPositionCodec { domain_id: 1 };
    let header = PositionHeader::new(1, 0, 0);
    let decoded = codec.decode(&header, &encoded[8..]).unwrap();

    let original_boxed: Box<dyn Position> = Box::new(original);
    assert_eq!(*decoded, *original_boxed);
}

#[test]
fn position_codec_invalid_content() {
    let codec = TestPositionCodec { domain_id: 1 };
    let header = PositionHeader::new(1, 0, 0);
    // Wrong length — decode returns None
    assert!(codec.decode(&header, &[1, 2]).is_none());
}

#[test]
fn position_codec_empty_content() {
    let codec = TestPositionCodec { domain_id: 1 };
    let header = PositionHeader::new(1, 0, 0);
    assert!(codec.decode(&header, &[]).is_none());
}

#[test]
fn position_codec_domain_id() {
    let codec = TestPositionCodec { domain_id: 7 };
    assert_eq!(codec.domain_id(), 7);
    assert_eq!(codec.inner_id(), 0);
}

// --- CursorCodec tests ---

#[test]
fn cursor_codec_object_safety() {
    let _: Box<dyn CursorCodec> = Box::new(TestCursorCodec {
        domain_id: 1,
        inner_id: 0,
    });
}

#[test]
fn cursor_codec_roundtrip() {
    let original = TestCursor::new(1, 0, vec![10, 20, 30]);
    let encoded = original.encode();

    let codec = TestCursorCodec {
        domain_id: 1,
        inner_id: 0,
    };
    let header = CursorHeader::new(1, 0, 0);
    let decoded = codec.decode(&header, &encoded[8..]).unwrap();

    let original_boxed: Box<dyn Cursor> = Box::new(original);
    assert_eq!(*decoded, *original_boxed);
}

#[test]
fn cursor_codec_empty_content() {
    let codec = TestCursorCodec {
        domain_id: 1,
        inner_id: 0,
    };
    let header = CursorHeader::new(1, 0, 0);
    let decoded = codec.decode(&header, &[]).unwrap();
    assert!(decoded.content().is_empty());
}

#[test]
fn cursor_codec_domain_inner_id() {
    let codec = TestCursorCodec {
        domain_id: 3,
        inner_id: 2,
    };
    assert_eq!(codec.domain_id(), 3);
    assert_eq!(codec.inner_id(), 2);
}

#[test]
fn cursor_codec_multiple_inner_ids() {
    let codec_a = TestCursorCodec {
        domain_id: 1,
        inner_id: 0,
    };
    let codec_b = TestCursorCodec {
        domain_id: 1,
        inner_id: 1,
    };
    assert_eq!(codec_a.domain_id(), codec_b.domain_id());
    assert_ne!(codec_a.inner_id(), codec_b.inner_id());
}
