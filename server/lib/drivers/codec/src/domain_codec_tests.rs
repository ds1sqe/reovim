//! Tests for domain-generic codec traits.

use reovim_domain::Domain;
use reovim_driver_vfs::ByteEdit;

use crate::{CodecError, CodecMetadata, ContentType};

use super::{Decode, DecodeOutput, Encode, Index};

// ─── Test Domain ────────────────────────────────────────────────────────────

/// Minimal test domain for verifying trait mechanics.
struct TestDomain;

#[derive(Debug, Clone, PartialEq)]
struct TestPos(usize);

#[derive(Debug, Clone)]
struct TestEdit {
    offset: usize,
    data: Vec<u8>,
}

impl Domain for TestDomain {
    type Position = TestPos;
    type Edit = TestEdit;
    type Content = Vec<u8>;
}

// ─── Mock Implementations ───────────────────────────────────────────────────

/// A trivial passthrough codec for testing.
struct PassthroughCodec;

impl Decode<TestDomain> for PassthroughCodec {
    fn decode(&self, raw: &[u8]) -> Result<DecodeOutput<TestDomain>, CodecError> {
        Ok(DecodeOutput {
            content: raw.to_vec(),
            metadata: CodecMetadata::new(ContentType::new("test/raw")),
            lossy: false,
            readonly: false,
            truncated: false,
        })
    }
}

impl Encode<TestDomain> for PassthroughCodec {
    fn encode(
        &self,
        content: &Vec<u8>,
        _metadata: &CodecMetadata,
    ) -> Result<Vec<u8>, CodecError> {
        Ok(content.clone())
    }
}

/// A trivial index that maps positions 1:1 with byte offsets.
struct IdentityIndex {
    len: usize,
}

impl IdentityIndex {
    const fn new() -> Self {
        Self { len: 0 }
    }
}

impl Index<TestDomain> for IdentityIndex {
    fn build(&mut self, raw: &[u8]) {
        self.len = raw.len();
    }

    fn notify(&mut self, edit: &ByteEdit) {
        // Adjust length: remove old, add new
        self.len = self.len - edit.old_bytes.len() + edit.new_bytes.len();
    }

    fn to_bytes(&self, pos: &TestPos) -> Option<usize> {
        if pos.0 <= self.len {
            Some(pos.0)
        } else {
            None
        }
    }

    fn offset_to_position(&self, offset: usize) -> Option<TestPos> {
        if offset <= self.len {
            Some(TestPos(offset))
        } else {
            None
        }
    }

    fn translate_edit(&self, edit: &TestEdit) -> Option<ByteEdit> {
        if edit.offset <= self.len {
            Some(ByteEdit::insert(edit.offset, &edit.data))
        } else {
            None
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[test]
fn decode_passthrough() {
    let codec = PassthroughCodec;
    let result = codec.decode(b"hello").unwrap();
    assert_eq!(result.content, b"hello");
    assert!(!result.lossy);
    assert!(!result.readonly);
    assert!(!result.truncated);
}

#[test]
fn encode_roundtrip() {
    let codec = PassthroughCodec;
    let decoded = codec.decode(b"test data").unwrap();
    let re_encoded = codec.encode(&decoded.content, &decoded.metadata).unwrap();
    assert_eq!(re_encoded, b"test data");
}

#[test]
fn index_build_and_query() {
    let mut index = IdentityIndex::new();
    index.build(b"abcde");

    assert_eq!(index.to_bytes(&TestPos(0)), Some(0));
    assert_eq!(index.to_bytes(&TestPos(3)), Some(3));
    assert_eq!(index.to_bytes(&TestPos(5)), Some(5));
    assert_eq!(index.to_bytes(&TestPos(6)), None);

    assert_eq!(index.offset_to_position(0), Some(TestPos(0)));
    assert_eq!(index.offset_to_position(4), Some(TestPos(4)));
    assert_eq!(index.offset_to_position(6), None);
}

#[test]
fn index_notify_insert() {
    let mut index = IdentityIndex::new();
    index.build(b"abc");
    assert_eq!(index.to_bytes(&TestPos(3)), Some(3));
    assert_eq!(index.to_bytes(&TestPos(4)), None);

    // Insert 2 bytes
    index.notify(&ByteEdit::insert(1, b"xy"));
    assert_eq!(index.to_bytes(&TestPos(5)), Some(5));
    assert_eq!(index.to_bytes(&TestPos(6)), None);
}

#[test]
fn index_notify_delete() {
    let mut index = IdentityIndex::new();
    index.build(b"abcde");

    // Delete 2 bytes
    index.notify(&ByteEdit::delete(1, b"bc"));
    assert_eq!(index.to_bytes(&TestPos(3)), Some(3));
    assert_eq!(index.to_bytes(&TestPos(4)), None);
}

#[test]
fn index_translate_edit() {
    let mut index = IdentityIndex::new();
    index.build(b"hello");

    let edit = TestEdit {
        offset: 2,
        data: vec![0x41, 0x42],
    };
    let byte_edit = index.translate_edit(&edit).unwrap();
    assert_eq!(byte_edit.offset, 2);
    assert_eq!(byte_edit.new_bytes, vec![0x41, 0x42]);
}

#[test]
fn index_translate_edit_out_of_bounds() {
    let mut index = IdentityIndex::new();
    index.build(b"hi");

    let edit = TestEdit {
        offset: 10,
        data: vec![0x41],
    };
    assert!(index.translate_edit(&edit).is_none());
}

#[test]
fn decode_output_lossy_readonly_invariant() {
    let output: DecodeOutput<TestDomain> = DecodeOutput {
        content: vec![],
        metadata: CodecMetadata::new(ContentType::new("test/raw")),
        lossy: true,
        readonly: true,
        truncated: false,
    };
    // lossy implies readonly
    assert!(output.lossy);
    assert!(output.readonly);
}

// ─── Trait Object Safety ────────────────────────────────────────────────────

#[test]
fn decode_as_trait_object() {
    let codec: Box<dyn Decode<TestDomain>> = Box::new(PassthroughCodec);
    let result = codec.decode(b"dynamic dispatch").unwrap();
    assert_eq!(result.content, b"dynamic dispatch");
}

#[test]
fn encode_as_trait_object() {
    let codec: Box<dyn Encode<TestDomain>> = Box::new(PassthroughCodec);
    let metadata = CodecMetadata::new(ContentType::new("test/raw"));
    let result = codec.encode(&vec![1, 2, 3], &metadata).unwrap();
    assert_eq!(result, vec![1, 2, 3]);
}

#[test]
fn index_as_trait_object() {
    let mut index: Box<dyn Index<TestDomain>> = Box::new(IdentityIndex::new());
    index.build(b"abc");
    assert_eq!(index.to_bytes(&TestPos(1)), Some(1));
    assert_eq!(index.offset_to_position(2), Some(TestPos(2)));
}
