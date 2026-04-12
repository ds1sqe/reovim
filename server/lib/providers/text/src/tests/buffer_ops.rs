use std::borrow::Cow;

use {
    crate::{BufferCapabilities, BufferOps},
    reovim_domain_text::{Position, TextGeometry},
    reovim_kernel::api::v1::{BufferId, BufferMeta, StorageCapabilities, StorageError, StorageOps},
};

/// Minimal mock implementing `StorageOps + BufferMeta + BufferOps`.
struct MockBuffer {
    id: BufferId,
    content: String,
    modified: bool,
    file_path: Option<String>,
}

impl MockBuffer {
    fn new(content: &str) -> Self {
        Self {
            id: BufferId::new(),
            content: content.to_owned(),
            modified: false,
            file_path: None,
        }
    }
}

// === StorageOps (byte-level I/O) ===

impl StorageOps for MockBuffer {
    fn byte_len(&self) -> usize {
        self.content.len()
    }

    fn read_bytes(&self, offset: usize, buf: &mut [u8]) -> usize {
        let bytes = self.content.as_bytes();
        if offset >= bytes.len() {
            return 0;
        }
        let available = &bytes[offset..];
        let count = buf.len().min(available.len());
        buf[..count].copy_from_slice(&available[..count]);
        count
    }

    fn capabilities(&self) -> StorageCapabilities {
        StorageCapabilities::HEAP
    }

    fn insert_bytes(&mut self, offset: usize, data: &[u8]) -> Result<(), StorageError> {
        let text = std::str::from_utf8(data)
            .map_err(|_| StorageError::NotSupported("non-UTF-8 insert into text buffer"))?;
        self.content.insert_str(offset, text);
        self.modified = true;
        Ok(())
    }

    fn delete_bytes(&mut self, offset: usize, len: usize) -> Result<Vec<u8>, StorageError> {
        let total = self.content.len();
        if offset + len > total {
            return Err(StorageError::OffsetOutOfRange { offset, len: total });
        }
        let end = offset + len;
        let deleted = self.content.as_bytes()[offset..end].to_vec();
        self.content.replace_range(offset..end, "");
        self.modified = true;
        Ok(deleted)
    }

    fn append_bytes(&mut self, data: &[u8]) -> Result<(), StorageError> {
        let offset = self.content.len();
        StorageOps::insert_bytes(self, offset, data)
    }

    fn read_chunk(&self, offset: usize, max_len: usize) -> Vec<u8> {
        let bytes = self.content.as_bytes();
        if offset >= bytes.len() {
            return Vec::new();
        }
        let end = (offset + max_len).min(bytes.len());
        bytes[offset..end].to_vec()
    }
}

// === BufferMeta (identity + state) ===

impl BufferMeta for MockBuffer {
    fn id(&self) -> BufferId {
        self.id
    }

    fn file_path(&self) -> Option<&str> {
        self.file_path.as_deref()
    }

    fn set_file_path(&mut self, path: Option<String>) {
        self.file_path = path;
    }

    fn is_modified(&self) -> bool {
        self.modified
    }

    fn set_modified(&mut self, modified: bool) {
        self.modified = modified;
    }
}

// === TextGeometry (read-only text access) ===

impl TextGeometry for MockBuffer {
    fn line_count(&self) -> usize {
        if self.content.is_empty() {
            return 0;
        }
        self.content.lines().count()
    }

    fn line(&self, idx: usize) -> Option<Cow<'_, str>> {
        self.content.lines().nth(idx).map(Cow::Borrowed)
    }

    fn line_len(&self, idx: usize) -> Option<usize> {
        self.content.lines().nth(idx).map(str::len)
    }

    fn is_empty(&self) -> bool {
        self.content.is_empty()
    }
}

// === BufferOps (text-specific extension) ===

impl BufferOps for MockBuffer {
    fn buffer_capabilities(&self) -> BufferCapabilities {
        BufferCapabilities::ROPE
    }

    fn line_count(&self) -> usize {
        TextGeometry::line_count(self)
    }

    fn line(&self, idx: usize) -> Option<Cow<'_, str>> {
        TextGeometry::line(self, idx)
    }

    fn line_len(&self, idx: usize) -> Option<usize> {
        TextGeometry::line_len(self, idx)
    }

    fn content_bytes(&self) -> Vec<u8> {
        self.content.as_bytes().to_vec()
    }

    fn position_to_byte(&self, pos: Position) -> usize {
        let mut offset = 0;
        for (i, line) in self.content.lines().enumerate() {
            if i == pos.line {
                return offset + pos.column.min(line.len());
            }
            offset += line.len() + 1;
        }
        self.content.len()
    }

    fn byte_to_position(&self, byte_offset: usize) -> Position {
        let mut offset = 0;
        for (i, line) in self.content.lines().enumerate() {
            let line_end = offset + line.len();
            if byte_offset <= line_end {
                return Position::new(i, byte_offset - offset);
            }
            offset = line_end + 1;
        }
        Position::new(0, 0)
    }

    fn insert_at(&mut self, pos: Position, text: &str) {
        let offset = self.position_to_byte(pos);
        self.content.insert_str(offset, text);
        self.modified = true;
    }

    fn delete_range(&mut self, start: Position, end: Position) -> String {
        let s = self.position_to_byte(start);
        let e = self.position_to_byte(end);
        let deleted = self.content[s..e].to_owned();
        self.content.replace_range(s..e, "");
        self.modified = true;
        deleted
    }

    fn set_content(&mut self, content: &str) {
        self.content = content.to_owned();
        self.modified = true;
    }

    fn content(&self) -> String {
        self.content.clone()
    }

    fn as_text_geometry(&self) -> &dyn TextGeometry {
        self
    }
}

// === Object Safety ===

#[test]
fn trait_is_object_safe() {
    let buf = MockBuffer::new("hello");
    let _boxed: Box<dyn BufferOps> = Box::new(buf);
}

#[test]
fn dyn_buffer_ops_works() {
    let buf = MockBuffer::new("hello\nworld");
    let dyn_buf: &dyn BufferOps = &buf;
    assert_eq!(dyn_buf.line_count(), 2);
    assert_eq!(dyn_buf.line(0).as_deref(), Some("hello"));
    // byte_len comes from StorageOps supertrait
    assert_eq!(dyn_buf.byte_len(), 11);
}

// === Supertrait access through dyn BufferOps ===

#[test]
fn supertrait_storage_ops_via_dyn_buffer_ops() {
    let buf = MockBuffer::new("hello");
    let dyn_buf: &dyn BufferOps = &buf;

    // StorageOps methods accessible through dyn BufferOps
    assert_eq!(dyn_buf.byte_len(), 5);
    assert!(!dyn_buf.is_empty());

    let mut read_buf = [0u8; 3];
    let n = dyn_buf.read_bytes(0, &mut read_buf);
    assert_eq!(n, 3);
    assert_eq!(&read_buf, b"hel");
}

#[test]
fn supertrait_buffer_meta_via_dyn_buffer_ops() {
    let buf = MockBuffer::new("hello");
    let dyn_buf: &dyn BufferOps = &buf;

    // BufferMeta methods accessible through dyn BufferOps
    let _ = dyn_buf.id();
    assert!(!dyn_buf.is_modified());
    assert!(dyn_buf.file_path().is_none());
}

// === as_text_geometry bridge ===

#[test]
fn as_text_geometry_line_count() {
    let buf = MockBuffer::new("a\nb\nc");
    let dyn_buf: &dyn BufferOps = &buf;
    let geom = dyn_buf.as_text_geometry();
    assert_eq!(geom.line_count(), 3);
}

#[test]
fn as_text_geometry_line() {
    let buf = MockBuffer::new("hello\nworld");
    let dyn_buf: &dyn BufferOps = &buf;
    let geom = dyn_buf.as_text_geometry();
    assert_eq!(geom.line(0).as_deref(), Some("hello"));
    assert_eq!(geom.line(1).as_deref(), Some("world"));
    assert_eq!(geom.line(2), None);
}

#[test]
fn as_text_geometry_is_empty() {
    let empty = MockBuffer::new("");
    let dyn_empty: &dyn BufferOps = &empty;
    assert!(dyn_empty.as_text_geometry().is_empty());

    let non_empty = MockBuffer::new("x");
    let dyn_ne: &dyn BufferOps = &non_empty;
    assert!(!dyn_ne.as_text_geometry().is_empty());
}

// === StorageOps error types (replace BufferOpsError) ===

#[test]
fn insert_bytes_rejects_invalid_utf8() {
    let mut buf = MockBuffer::new("hello");
    let result = StorageOps::insert_bytes(&mut buf, 0, &[0xFF, 0xFE]);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not supported"));
}

#[test]
fn storage_error_display() {
    let err = StorageError::NotSupported("test op");
    assert_eq!(err.to_string(), "operation not supported: test op");

    let err2 = StorageError::OffsetOutOfRange {
        offset: 100,
        len: 50,
    };
    assert_eq!(err2.to_string(), "offset 100 out of range (len 50)");
}

// === buffer_capabilities ===

#[test]
fn buffer_capabilities_returns_text_flags() {
    let buf = MockBuffer::new("hello");
    let dyn_buf: &dyn BufferOps = &buf;
    assert_eq!(dyn_buf.buffer_capabilities(), BufferCapabilities::ROPE);
}
