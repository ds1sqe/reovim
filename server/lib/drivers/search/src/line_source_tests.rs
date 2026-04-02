//! Tests for `LineSource` adapters.

use std::sync::Arc;

use {
    reovim_kernel::api::v1::{Buffer, FileMapping, LineIndex},
    reovim_provider_text::{HeapMapping, VirtualBuffer},
    reovim_types_text::Position,
};

use super::{BufferLineSource, LineSource, VirtualBufferLineSource};

fn make_vbuf(content: &str) -> VirtualBuffer {
    let original: Arc<dyn FileMapping> = Arc::new(HeapMapping(content.as_bytes().to_vec()));
    let line_index = LineIndex::build_from_str(content);
    VirtualBuffer::new(original, line_index)
}

// ── BufferLineSource ───────────────────────────────────────────────────

#[test]
fn buffer_line_count() {
    let buffer = Buffer::from_string("a\nb\nc");
    let source = BufferLineSource(&buffer);
    assert_eq!(source.line_count(), 3);
}

#[test]
fn buffer_line() {
    let buffer = Buffer::from_string("hello\nworld");
    let source = BufferLineSource(&buffer);
    assert_eq!(source.line(0).as_deref(), Some("hello"));
    assert_eq!(source.line(1).as_deref(), Some("world"));
    assert!(source.line(2).is_none());
}

#[test]
fn buffer_line_len() {
    let buffer = Buffer::from_string("abc\nde");
    let source = BufferLineSource(&buffer);
    assert_eq!(source.line_len(0), Some(3));
    assert_eq!(source.line_len(1), Some(2));
}

#[test]
fn buffer_position_conversion() {
    let buffer = Buffer::from_string("abc\ndef");
    let source = BufferLineSource(&buffer);
    let byte = source.position_to_byte(Position::new(1, 1));
    let pos = source.byte_to_position(byte);
    assert_eq!(pos, Position::new(1, 1));
}

#[test]
fn buffer_content() {
    let buffer = Buffer::from_string("hello\nworld");
    let source = BufferLineSource(&buffer);
    assert_eq!(source.content(), "hello\nworld");
}

// ── VirtualBufferLineSource ────────────────────────────────────────────

#[test]
fn vbuf_line_count() {
    let vbuf = make_vbuf("a\nb\nc");
    let source = VirtualBufferLineSource(&vbuf);
    assert_eq!(source.line_count(), 3);
}

#[test]
fn vbuf_line() {
    let vbuf = make_vbuf("hello\nworld");
    let source = VirtualBufferLineSource(&vbuf);
    assert_eq!(source.line(0).as_deref(), Some("hello"));
    assert_eq!(source.line(1).as_deref(), Some("world"));
    assert!(source.line(2).is_none());
}

#[test]
fn vbuf_line_len() {
    let vbuf = make_vbuf("abc\nde");
    let source = VirtualBufferLineSource(&vbuf);
    assert_eq!(source.line_len(0), Some(3));
    assert_eq!(source.line_len(1), Some(2));
}

#[test]
fn vbuf_position_conversion() {
    let vbuf = make_vbuf("abc\ndef");
    let source = VirtualBufferLineSource(&vbuf);
    let byte = source.position_to_byte(Position::new(1, 1));
    let pos = source.byte_to_position(byte);
    assert_eq!(pos, Position::new(1, 1));
}

#[test]
fn vbuf_content() {
    let vbuf = make_vbuf("hello\nworld");
    let source = VirtualBufferLineSource(&vbuf);
    assert_eq!(source.content(), "hello\nworld");
}

// ── Default content() implementation ───────────────────────────────────

#[test]
fn default_content_empty() {
    let buffer = Buffer::new();
    let source = BufferLineSource(&buffer);
    // Use the trait's default impl by checking consistency
    let line_count = source.line_count();
    assert_eq!(line_count, 0);
}
