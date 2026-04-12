//! Tests for `LineSource` adapters.

use std::sync::Arc;

use {
    reovim_domain_text::{LineIndex, Position, SimpleText},
    reovim_provider_text::{Buffer, HeapMapping, VirtualBuffer},
};

use super::{
    BufferLineSource, BufferOpsLineSource, LineSource, TextGeometryLineSource,
    VirtualBufferLineSource,
};

fn make_vbuf(content: &str) -> VirtualBuffer {
    let original = Arc::new(HeapMapping(content.as_bytes().to_vec()));
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

#[test]
fn text_geometry_line_count() {
    let text = SimpleText::new("a\nb\nc");
    let source = TextGeometryLineSource::new(&text);
    assert_eq!(source.line_count(), 3);
}

#[test]
fn text_geometry_position_conversion() {
    let text = SimpleText::new("héllo\nworld");
    let source = TextGeometryLineSource::new(&text);
    assert_eq!(source.position_to_byte(Position::new(0, 0)), 0);
    assert_eq!(source.position_to_byte(Position::new(0, 1)), 1);
    assert_eq!(source.position_to_byte(Position::new(0, 5)), 6);
    assert_eq!(source.position_to_byte(Position::new(1, 0)), 7);
    assert_eq!(source.byte_to_position(6), Position::new(0, 5));
    assert_eq!(source.byte_to_position(7), Position::new(1, 0));
    assert_eq!(source.byte_to_position(12), Position::new(1, 5));
}

#[test]
fn text_geometry_content() {
    let text = SimpleText::new("hello\nworld");
    let source = TextGeometryLineSource::new(&text);
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

// ── MC/DC 47:1 — default content() with a LineSource that returns None ─

struct PartialLineSource {
    count: usize,
}

impl LineSource for PartialLineSource {
    fn line_count(&self) -> usize {
        self.count
    }

    fn line(&self, idx: usize) -> Option<std::borrow::Cow<'_, str>> {
        // Return None for odd indices to exercise the else path of
        // `if let Some(line) = self.line(i)` in content().
        if idx.is_multiple_of(2) {
            Some(std::borrow::Cow::Borrowed("line"))
        } else {
            None
        }
    }

    fn line_len(&self, _idx: usize) -> Option<usize> {
        None
    }

    fn position_to_byte(&self, _pos: reovim_domain_text::Position) -> usize {
        0
    }

    fn byte_to_position(&self, _byte_offset: usize) -> reovim_domain_text::Position {
        reovim_domain_text::Position::new(0, 0)
    }
}

#[test]
fn default_content_skips_none_lines() {
    // MC/DC 47:1: line() returns None for odd indices; the default content()
    // implementation skips them (the None arm of the if-let).
    let source = PartialLineSource { count: 4 };
    // Lines: 0=Some("line"), 1=None, 2=Some("line"), 3=None
    let content = source.content();
    // The default impl pushes '\n' before each non-first iteration and skips
    // the push_str when line() returns None.
    assert_eq!(content, "line\n\nline\n");
}

// ── MC/DC 141:0 and 156:0 — TextGeometryLineSource with empty content ──

#[test]
fn text_geometry_position_to_byte_empty_source() {
    // MC/DC 141:0: line_count == 0 in TextGeometryLineSource::position_to_byte.
    let text = reovim_domain_text::SimpleText::new("");
    let source = TextGeometryLineSource::new(&text);
    // Empty source: position_to_byte should return 0 immediately.
    assert_eq!(source.position_to_byte(reovim_domain_text::Position::new(0, 0)), 0);
}

#[test]
fn text_geometry_byte_to_position_empty_source() {
    // MC/DC 156:0: line_count == 0 in TextGeometryLineSource::byte_to_position.
    let text = reovim_domain_text::SimpleText::new("");
    let source = TextGeometryLineSource::new(&text);
    // Empty source: byte_to_position should return Position::origin().
    assert_eq!(source.byte_to_position(0), reovim_domain_text::Position::new(0, 0));
}

// ── TextGeometryLineSource::line_len ────────────────────────────────────────

#[test]
fn text_geometry_line_len() {
    let text = SimpleText::new("abc\nde");
    let source = TextGeometryLineSource::new(&text);
    assert_eq!(source.line_len(0), Some(3));
    assert_eq!(source.line_len(1), Some(2));
    assert_eq!(source.line_len(99), None);
}

// ── TextGeometryLineSource::byte_to_position past end ───────────────────────

#[test]
fn text_geometry_byte_to_position_past_end() {
    let text = SimpleText::new("abc\ndef");
    let source = TextGeometryLineSource::new(&text);
    // byte_offset 100 is past all content: falls through to last-line fallback
    let pos = source.byte_to_position(100);
    assert_eq!(pos, Position::new(1, 3));
}

// ── BufferOpsLineSource ──────────────────────────────────────────────────────

#[test]
fn buffer_ops_line_source_line_count() {
    let buffer = Buffer::from_string("a\nb\nc");
    let source = BufferOpsLineSource(&buffer as &dyn reovim_provider_text::BufferOps);
    assert_eq!(source.line_count(), 3);
}

#[test]
fn buffer_ops_line_source_line() {
    let buffer = Buffer::from_string("hello\nworld");
    let source = BufferOpsLineSource(&buffer as &dyn reovim_provider_text::BufferOps);
    assert_eq!(source.line(0).as_deref(), Some("hello"));
    assert_eq!(source.line(1).as_deref(), Some("world"));
    assert!(source.line(2).is_none());
}

#[test]
fn buffer_ops_line_source_line_len() {
    let buffer = Buffer::from_string("abc\nde");
    let source = BufferOpsLineSource(&buffer as &dyn reovim_provider_text::BufferOps);
    assert_eq!(source.line_len(0), Some(3));
    assert_eq!(source.line_len(1), Some(2));
    assert!(source.line_len(99).is_none());
}

#[test]
fn buffer_ops_line_source_position_conversion() {
    let buffer = Buffer::from_string("abc\ndef");
    let source = BufferOpsLineSource(&buffer as &dyn reovim_provider_text::BufferOps);
    let byte = source.position_to_byte(Position::new(1, 1));
    let pos = source.byte_to_position(byte);
    assert_eq!(pos, Position::new(1, 1));
}

#[test]
fn buffer_ops_line_source_content() {
    let buffer = Buffer::from_string("hello\nworld");
    let source = BufferOpsLineSource(&buffer as &dyn reovim_provider_text::BufferOps);
    assert_eq!(source.content(), "hello\nworld");
}
