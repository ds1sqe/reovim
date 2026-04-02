//! Virtual buffer backed by mmap + piece table for large files.
//!
//! The original file stays on disk, memory-mapped read-only via
//! [`FileMapping`].  Edits create new pieces referencing an append-only
//! add buffer.  The [`PieceTree`] is a B-tree with `Arc` structural sharing
//! (O(1) clone) for efficient snapshots.
//!
//! # `FileMapping` Trait
//!
//! Defined here in the kernel to keep it free of driver types.
//! `MappedFile` (VFS driver, real mmap) and [`HeapMapping`] (test helper)
//! both implement this trait.

use std::{
    borrow::Cow,
    fmt,
    hash::{Hash, Hasher},
    ops::Range,
    sync::Arc,
};

use super::{
    BufferId, Position,
    line_index::LineIndex,
    piece_table::{Piece, PieceMetrics, PieceSource, PieceTree},
};

// ─── FileMapping Trait ──────────────────────────────────────────────────────

/// Trait for zero-copy access to original file bytes.
///
/// Implemented by `MappedFile` (VFS driver, real mmap) and by
/// `HeapMapping` (test helper, `Vec<u8>` wrapper).
pub trait FileMapping: Send + Sync + 'static {
    /// Raw bytes of the original file.
    fn as_bytes(&self) -> &[u8];

    /// File size in bytes.
    fn len(&self) -> u64;

    /// Whether the file is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Check if the underlying file has been modified since mapping.
    /// Returns `false` for heap-backed mappings (tests).
    fn is_stale(&self) -> bool {
        false
    }
}

// ─── VirtualSnapshot ────────────────────────────────────────────────────────

/// Opaque snapshot of `VirtualBuffer` state.
///
/// Produced by [`VirtualBuffer::capture_snapshot`], consumed by
/// [`VirtualBuffer::restore_snapshot`].  The `block/snapshot.rs` module
/// stores this type without knowing `VirtualBuffer` internals.
#[derive(Clone)]
pub struct VirtualSnapshot {
    pieces: PieceTree,
    add_buffer_len: usize,
    original: Arc<dyn FileMapping>,
    line_index: LineIndex,
    crlf: bool,
}

impl VirtualSnapshot {
    /// Length of the add buffer at snapshot time.
    #[must_use]
    pub const fn add_buffer_len(&self) -> usize {
        self.add_buffer_len
    }
}

impl fmt::Debug for VirtualSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VirtualSnapshot")
            .field("piece_count", &self.pieces.piece_count())
            .field("add_buffer_len", &self.add_buffer_len)
            .field("original_len", &self.original.len())
            .field("line_count", &self.line_index.line_count())
            .field("crlf", &self.crlf)
            .finish()
    }
}

// ─── VirtualBuffer ──────────────────────────────────────────────────────────

/// A buffer backed by mmap + piece table for large files.
///
/// The original file content is accessed via `Arc<dyn FileMapping>` (zero-copy).
/// Edits append to `add_buffer` and update the `PieceTree`.
///
/// Clone is cheap: `PieceTree` is O(1) via Arc, `original` is an Arc bump.
/// `line_index` and `add_buffer` are cloned (Vec/String).
pub struct VirtualBuffer {
    id: BufferId,
    pieces: PieceTree,
    add_buffer: String,
    line_index: LineIndex,
    /// Original file content (zero-copy mmap or heap-copied for tests).
    original: Arc<dyn FileMapping>,
    file_size: u64,
    file_path: Option<String>,
    modified: bool,
    /// Whether the original file uses CRLF line endings.
    crlf: bool,
}

impl Clone for VirtualBuffer {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            pieces: self.pieces.clone(),
            add_buffer: self.add_buffer.clone(),
            line_index: self.line_index.clone(),
            original: Arc::clone(&self.original),
            file_size: self.file_size,
            file_path: self.file_path.clone(),
            modified: self.modified,
            crlf: self.crlf,
        }
    }
}

impl fmt::Debug for VirtualBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VirtualBuffer")
            .field("id", &self.id)
            .field("piece_count", &self.pieces.piece_count())
            .field("add_buffer_len", &self.add_buffer.len())
            .field("line_count", &self.line_index.line_count())
            .field("original_len", &self.original.len())
            .field("file_size", &self.file_size)
            .field("file_path", &self.file_path)
            .field("modified", &self.modified)
            .field("crlf", &self.crlf)
            .finish()
    }
}

// ── Construction ────────────────────────────────────────────────────────────

impl VirtualBuffer {
    /// Create a new `VirtualBuffer` from a file mapping.
    ///
    /// The `line_index` is built from the mapped bytes.  The `crlf` flag
    /// is detected during the line index scan.
    ///
    /// # Panics
    ///
    /// Panics if the original bytes are not valid UTF-8 (caller must
    /// validate via `LineIndex::from_bytes` first).
    #[must_use]
    pub fn new(original: Arc<dyn FileMapping>, line_index: LineIndex) -> Self {
        let file_size = original.len();
        let crlf = line_index.has_crlf();

        // Single piece covering the entire original file
        let piece = if file_size > 0 {
            let metrics = PieceMetrics::compute(
                // SAFETY: LineIndex::from_bytes already validated UTF-8
                std::str::from_utf8(original.as_bytes()).expect("LineIndex validated UTF-8"),
            );
            Piece {
                source: PieceSource::Original {
                    byte_start: 0,
                    byte_len: file_size,
                },
                metrics,
            }
        } else {
            Piece {
                source: PieceSource::Original {
                    byte_start: 0,
                    byte_len: 0,
                },
                metrics: PieceMetrics::default(),
            }
        };

        let pieces = if file_size > 0 {
            PieceTree::from_piece(piece)
        } else {
            PieceTree::new()
        };

        Self {
            id: BufferId::new(),
            pieces,
            add_buffer: String::new(),
            line_index,
            original,
            file_size,
            file_path: None,
            modified: false,
            crlf,
        }
    }

    /// Create a `VirtualBuffer` with a specific ID (for testing).
    #[must_use]
    pub fn with_id(id: BufferId, original: Arc<dyn FileMapping>, line_index: LineIndex) -> Self {
        let mut vbuf = Self::new(original, line_index);
        vbuf.id = id;
        vbuf
    }
}

// ── Accessors ───────────────────────────────────────────────────────────────

impl VirtualBuffer {
    /// Get the buffer ID.
    #[must_use]
    pub const fn id(&self) -> BufferId {
        self.id
    }

    /// Get the file path.
    #[must_use]
    pub fn file_path(&self) -> Option<&str> {
        self.file_path.as_deref()
    }

    /// Set the file path.
    pub fn set_file_path(&mut self, path: Option<String>) {
        self.file_path = path;
    }

    /// Check if the buffer has been modified.
    #[must_use]
    pub const fn is_modified(&self) -> bool {
        self.modified
    }

    /// Set the modified flag.
    pub const fn set_modified(&mut self, modified: bool) {
        self.modified = modified;
    }

    /// Number of lines.
    #[must_use]
    pub const fn line_count(&self) -> usize {
        self.line_index.line_count()
    }

    /// Whether the buffer is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.pieces.is_empty()
    }

    /// Whether the file uses CRLF line endings.
    #[must_use]
    pub const fn has_crlf(&self) -> bool {
        self.crlf
    }

    /// Original file size in bytes.
    #[must_use]
    pub const fn file_size(&self) -> u64 {
        self.file_size
    }

    /// Number of pieces in the piece tree.
    #[must_use]
    pub const fn piece_count(&self) -> usize {
        self.pieces.piece_count()
    }

    /// Access to the underlying `PieceTree` (for snapshot).
    #[allow(dead_code)]
    pub(crate) const fn pieces(&self) -> &PieceTree {
        &self.pieces
    }
}

// ── Line Access ─────────────────────────────────────────────────────────────

impl VirtualBuffer {
    /// Get a line by index (0-based).
    ///
    /// Returns `None` if `idx >= line_count()`.
    /// If `crlf` is set, strips trailing `\r` from the returned line.
    #[must_use]
    pub fn line(&self, idx: usize) -> Option<String> {
        let range = self.line_byte_range(idx)?;
        let content = self.materialize_byte_range(range.start, range.end);
        let line = if self.crlf {
            content.strip_suffix('\r').unwrap_or(&content).to_string()
        } else {
            content
        };
        Some(line)
    }

    /// Get the length of a line in characters.
    #[must_use]
    pub fn line_len(&self, idx: usize) -> Option<usize> {
        self.line(idx).map(|l| l.chars().count())
    }

    /// Byte range for a line (from line index).
    fn line_byte_range(&self, idx: usize) -> Option<Range<u64>> {
        self.line_index.line_byte_range(idx)
    }

    /// Compute hash of a line for cache validation.
    #[must_use]
    pub fn line_hash(&self, line_idx: usize) -> Option<u64> {
        use std::collections::hash_map::DefaultHasher;

        self.line(line_idx).map(|line| {
            let mut hasher = DefaultHasher::new();
            line.hash(&mut hasher);
            hasher.finish()
        })
    }

    /// Get all line hashes.
    #[must_use]
    pub fn line_hashes(&self) -> Vec<u64> {
        (0..self.line_count())
            .filter_map(|idx| self.line_hash(idx))
            .collect()
    }
}

// ── Content Materialization ─────────────────────────────────────────────────

impl VirtualBuffer {
    /// Materialize the full content as a `String`.
    ///
    /// O(n) — walks all pieces and copies bytes.
    /// For large files, prefer line-based access.
    #[must_use]
    pub fn content(&self) -> String {
        let mut result = String::with_capacity(self.pieces.byte_len() as usize);
        for piece in self.pieces.iter_pieces() {
            result.push_str(&self.piece_text(piece));
        }
        if self.crlf {
            result.replace("\r\n", "\n")
        } else {
            result
        }
    }

    /// Materialize a byte range from pieces as a `String`.
    fn materialize_byte_range(&self, byte_start: u64, byte_end: u64) -> String {
        if byte_start >= byte_end {
            return String::new();
        }

        let mut result = String::new();
        let mut cumulative = 0u64;

        for piece in self.pieces.iter_pieces() {
            let piece_start = cumulative;
            let piece_end = cumulative + piece.metrics.byte_len;
            cumulative = piece_end;

            if piece_end <= byte_start || piece_start >= byte_end {
                continue;
            }

            let text = self.piece_text(piece);
            let slice_start = byte_start.saturating_sub(piece_start) as usize;
            let slice_end = (byte_end - piece_start).min(piece.metrics.byte_len) as usize;

            // Ensure we're on char boundaries
            let text_bytes = text.as_bytes();
            let safe_start = find_char_boundary(text_bytes, slice_start);
            let safe_end = find_char_boundary(text_bytes, slice_end);

            if safe_start < safe_end && safe_end <= text_bytes.len() {
                result.push_str(&text[safe_start..safe_end]);
            }
        }

        result
    }

    /// Get the text for a piece.
    fn piece_text(&self, piece: &Piece) -> String {
        match piece.source {
            PieceSource::Original {
                byte_start,
                byte_len,
            } => {
                let bytes = self.original.as_bytes();
                let start = byte_start as usize;
                let end = start + byte_len as usize;
                // SAFETY: LineIndex validated UTF-8 at construction
                String::from_utf8_lossy(&bytes[start..end]).into_owned()
            }
            PieceSource::Add { offset, len } => self.add_buffer[offset..offset + len].to_string(),
        }
    }
}

/// Find the nearest char boundary at or before `idx` in UTF-8 bytes.
fn find_char_boundary(bytes: &[u8], idx: usize) -> usize {
    if idx >= bytes.len() {
        return bytes.len();
    }
    let mut i = idx;
    while i > 0 && (bytes[i] & 0xC0) == 0x80 {
        i -= 1;
    }
    i
}

// ── Position Conversion ─────────────────────────────────────────────────────

impl VirtualBuffer {
    /// Convert a (line, column) position to a byte offset.
    ///
    /// `col` is measured in Unicode scalar values (chars), not bytes.
    /// Clamps to valid ranges.
    #[must_use]
    pub fn position_to_byte(&self, pos: Position) -> usize {
        if self.is_empty() {
            return 0;
        }
        let line = pos.line.min(self.line_count().saturating_sub(1));
        let line_start = self.line_index.line_to_byte(line);
        let line_text = self.line(line).unwrap_or_default();
        let col = pos.column.min(line_text.chars().count());

        let byte_col: usize = line_text
            .char_indices()
            .nth(col)
            .map_or(line_text.len(), |(i, _)| i);

        (line_start as usize) + byte_col
    }

    /// Convert a byte offset to a (line, column) position.
    ///
    /// Column is measured in Unicode scalar values (chars).
    #[must_use]
    pub fn byte_to_position(&self, byte_offset: usize) -> Position {
        if self.is_empty() {
            return Position::origin();
        }
        let line = self.line_index.byte_to_line(byte_offset as u64);
        let line_start = self.line_index.line_to_byte(line) as usize;
        let offset_in_line = byte_offset.saturating_sub(line_start);

        let line_text = self.line(line).unwrap_or_default();
        let col = line_text
            .as_bytes()
            .iter()
            .take(offset_in_line)
            .filter(|&&b| (b & 0xC0) != 0x80) // count non-continuation bytes
            .count();

        Position::new(line, col)
    }
}

// ── Edit Operations ─────────────────────────────────────────────────────────

impl VirtualBuffer {
    /// Insert text at a position.
    pub fn insert_at(&mut self, pos: Position, text: &str) {
        if text.is_empty() {
            return;
        }

        let byte_offset = if self.is_empty() {
            0
        } else {
            self.position_to_byte(pos) as u64
        };

        // Append to add_buffer
        let add_offset = self.add_buffer.len();
        self.add_buffer.push_str(text);
        let add_len = text.len();

        let metrics = PieceMetrics::compute(text);
        let piece = Piece {
            source: PieceSource::Add {
                offset: add_offset,
                len: add_len,
            },
            metrics,
        };

        self.pieces = self.pieces.insert(byte_offset, piece);
        self.modified = true;

        // Rebuild line index from materialized content
        self.rebuild_line_index();
    }

    /// Delete text at a position.
    ///
    /// Returns the deleted text.
    pub fn delete_at(&mut self, pos: Position, count: usize) -> String {
        if count == 0 || self.is_empty() {
            return String::new();
        }

        let byte_start = self.position_to_byte(pos) as u64;
        let content = self.content();
        let char_start = content[..byte_start as usize].chars().count();
        let char_end = (char_start + count).min(content.chars().count());

        // Find byte range for the chars to delete
        let byte_end = content
            .char_indices()
            .nth(char_end)
            .map_or(content.len(), |(i, _)| i) as u64;

        if byte_start >= byte_end {
            return String::new();
        }

        let deleted = self.materialize_byte_range(byte_start, byte_end);
        let delete_len = byte_end - byte_start;

        self.pieces = self.pieces.delete(byte_start, delete_len);
        self.modified = true;

        self.rebuild_line_index();

        deleted
    }

    /// Delete a range of text.
    ///
    /// Returns the deleted text.
    pub fn delete_range(&mut self, start: Position, end: Position) -> String {
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };

        if self.is_empty() {
            return String::new();
        }

        let byte_start = self.position_to_byte(start) as u64;
        let byte_end = self.position_to_byte(end) as u64;

        if byte_start >= byte_end {
            return String::new();
        }

        let deleted = self.materialize_byte_range(byte_start, byte_end);
        let delete_len = byte_end - byte_start;

        self.pieces = self.pieces.delete(byte_start, delete_len);
        self.modified = true;

        self.rebuild_line_index();

        deleted
    }

    /// Set the full content from a string.
    pub fn set_content(&mut self, content: &str) {
        self.add_buffer.clear();
        self.add_buffer.push_str(content);

        let metrics = PieceMetrics::compute(content);
        let piece = Piece {
            source: PieceSource::Add {
                offset: 0,
                len: content.len(),
            },
            metrics,
        };

        self.pieces = if content.is_empty() {
            PieceTree::new()
        } else {
            PieceTree::from_piece(piece)
        };

        self.modified = true;
        self.rebuild_line_index();
    }

    /// Rebuild the line index from current content.
    fn rebuild_line_index(&mut self) {
        let content = self.content_bytes();
        self.line_index.rebuild(&content);
    }

    /// Write buffer content to a writer without full materialization.
    ///
    /// Iterates pieces in order, writing each directly from the mmap
    /// or add buffer.  This avoids allocating a full copy in memory.
    ///
    /// # Errors
    ///
    /// Returns `std::io::Error` if writing fails.
    pub fn write_to(&self, writer: &mut dyn std::io::Write) -> Result<(), std::io::Error> {
        for piece in self.pieces.iter_pieces() {
            match piece.source {
                PieceSource::Original {
                    byte_start,
                    byte_len,
                } => {
                    let bytes = self.original.as_bytes();
                    let start = byte_start as usize;
                    let end = start + byte_len as usize;
                    writer.write_all(&bytes[start..end])?;
                }
                PieceSource::Add { offset, len } => {
                    writer.write_all(
                        self.add_buffer
                            .as_bytes()
                            .get(offset..offset + len)
                            .unwrap_or_default(),
                    )?;
                }
            }
        }
        Ok(())
    }

    /// Materialize content as bytes (for line index rebuild).
    fn content_bytes(&self) -> Vec<u8> {
        let mut result = Vec::with_capacity(self.pieces.byte_len() as usize);
        for piece in self.pieces.iter_pieces() {
            match piece.source {
                PieceSource::Original {
                    byte_start,
                    byte_len,
                } => {
                    let bytes = self.original.as_bytes();
                    let start = byte_start as usize;
                    let end = start + byte_len as usize;
                    result.extend_from_slice(&bytes[start..end]);
                }
                PieceSource::Add { offset, len } => {
                    result.extend_from_slice(
                        self.add_buffer
                            .as_bytes()
                            .get(offset..offset + len)
                            .unwrap_or_default(),
                    );
                }
            }
        }
        result
    }
}

// ── Snapshot ────────────────────────────────────────────────────────────────

impl VirtualBuffer {
    /// Capture an opaque snapshot of this buffer's state.
    #[must_use]
    pub fn capture_snapshot(&self) -> VirtualSnapshot {
        VirtualSnapshot {
            pieces: self.pieces.clone(),
            add_buffer_len: self.add_buffer.len(),
            original: Arc::clone(&self.original),
            line_index: self.line_index.clone(),
            crlf: self.crlf,
        }
    }

    /// Restore state from a snapshot.
    pub fn restore_snapshot(&mut self, snap: VirtualSnapshot) {
        self.pieces = snap.pieces;
        self.add_buffer.truncate(snap.add_buffer_len);
        self.original = snap.original;
        self.line_index = snap.line_index;
        self.crlf = snap.crlf;
        self.modified = true;
    }
}

// ─── HeapMapping (test helper) ──────────────────────────────────────────────

/// Heap-backed `FileMapping` for unit tests.
///
/// Wraps a `Vec<u8>` to simulate file content without actual mmap.
#[derive(Debug, Clone)]
pub struct HeapMapping(pub Vec<u8>);

impl FileMapping for HeapMapping {
    fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    fn len(&self) -> u64 {
        self.0.len() as u64
    }
}

// ── TextGeometry ────────────────────────────────────────────────────────────

use crate::{
    api::{
        BufferCapabilities, BufferOps, BufferOpsError,
        storage_ops::{BufferMeta, StorageCapabilities, StorageError, StorageOps},
    },
    core::TextGeometry,
};

impl TextGeometry for VirtualBuffer {
    fn line_count(&self) -> usize {
        self.line_index.line_count()
    }

    fn line(&self, idx: usize) -> Option<Cow<'_, str>> {
        Self::line(self, idx).map(Cow::Owned)
    }

    fn line_len(&self, idx: usize) -> Option<usize> {
        Self::line(self, idx).map(|l| l.chars().count())
    }

    fn is_empty(&self) -> bool {
        self.pieces.is_empty()
    }
}

// ── StorageOps + BufferMeta (#740) ────────────────────────────────────────

impl StorageOps for VirtualBuffer {
    fn byte_len(&self) -> usize {
        self.pieces.byte_len() as usize
    }

    fn read_bytes(&self, offset: usize, buf: &mut [u8]) -> usize {
        BufferOps::read_bytes(self, offset, buf)
    }

    fn capabilities(&self) -> StorageCapabilities {
        StorageCapabilities::MMAP
    }

    fn insert_bytes(&mut self, offset: usize, data: &[u8]) -> Result<(), StorageError> {
        let text = std::str::from_utf8(data)
            .map_err(|_| StorageError::NotSupported("non-UTF-8 insert into text buffer"))?;
        if text.is_empty() {
            return Ok(());
        }
        let total = self.pieces.byte_len() as usize;
        if offset > total {
            return Err(StorageError::OffsetOutOfRange { offset, len: total });
        }
        let pos = BufferOps::byte_to_position(self, offset);
        Self::insert_at(self, pos, text);
        Ok(())
    }

    fn delete_bytes(&mut self, offset: usize, len: usize) -> Result<Vec<u8>, StorageError> {
        let total = self.pieces.byte_len() as usize;
        if offset + len > total {
            return Err(StorageError::OffsetOutOfRange { offset, len: total });
        }
        if len == 0 {
            return Ok(Vec::new());
        }
        let start = BufferOps::byte_to_position(self, offset);
        let end = BufferOps::byte_to_position(self, offset + len);
        Ok(Self::delete_range(self, start, end).into_bytes())
    }

    fn append_bytes(&mut self, data: &[u8]) -> Result<(), StorageError> {
        let offset = self.pieces.byte_len() as usize;
        StorageOps::insert_bytes(self, offset, data)
    }

    fn read_chunk(&self, offset: usize, max_len: usize) -> Vec<u8> {
        let total = self.pieces.byte_len() as usize;
        if offset >= total {
            return Vec::new();
        }
        let end = (offset + max_len).min(total);
        let content = self.content_bytes();
        content[offset..end].to_vec()
    }
}

impl BufferMeta for VirtualBuffer {
    fn id(&self) -> BufferId {
        self.id
    }

    fn file_path(&self) -> Option<&str> {
        Self::file_path(self)
    }

    fn set_file_path(&mut self, path: Option<String>) {
        Self::set_file_path(self, path);
    }

    fn is_modified(&self) -> bool {
        self.modified
    }

    fn set_modified(&mut self, modified: bool) {
        Self::set_modified(self, modified);
    }
}

// ── BufferOps ──────────────────────────────────────────────────────────────

impl BufferOps for VirtualBuffer {
    fn id(&self) -> BufferId {
        self.id
    }

    fn byte_len(&self) -> usize {
        self.pieces.byte_len() as usize
    }

    fn read_bytes(&self, offset: usize, buf: &mut [u8]) -> usize {
        let content = self.content_bytes();
        if offset >= content.len() {
            return 0;
        }
        let available = &content[offset..];
        let count = buf.len().min(available.len());
        buf[..count].copy_from_slice(&available[..count]);
        count
    }

    fn insert_bytes(&mut self, offset: usize, data: &[u8]) -> Result<(), BufferOpsError> {
        let text = std::str::from_utf8(data).map_err(|_| BufferOpsError::InvalidUtf8)?;
        if text.is_empty() {
            return Ok(());
        }
        let pos = BufferOps::byte_to_position(self, offset);
        Self::insert_at(self, pos, text);
        Ok(())
    }

    fn delete_bytes(&mut self, offset: usize, len: usize) -> Vec<u8> {
        if len == 0 {
            return Vec::new();
        }
        let start = BufferOps::byte_to_position(self, offset);
        let end = BufferOps::byte_to_position(self, offset + len);
        Self::delete_range(self, start, end).into_bytes()
    }

    fn content_bytes(&self) -> Vec<u8> {
        Self::content_bytes(self)
    }

    fn is_modified(&self) -> bool {
        self.modified
    }

    fn set_modified(&mut self, modified: bool) {
        Self::set_modified(self, modified);
    }

    fn file_path(&self) -> Option<&str> {
        Self::file_path(self)
    }

    fn set_file_path(&mut self, path: Option<String>) {
        Self::set_file_path(self, path);
    }

    fn capabilities(&self) -> BufferCapabilities {
        BufferCapabilities::VIRTUAL
    }

    fn line_count(&self) -> usize {
        Self::line_count(self)
    }

    fn line(&self, idx: usize) -> Option<Cow<'_, str>> {
        Self::line(self, idx).map(Cow::Owned)
    }

    fn line_len(&self, idx: usize) -> Option<usize> {
        Self::line(self, idx).map(|l| l.chars().count())
    }

    fn position_to_byte(&self, pos: Position) -> usize {
        Self::position_to_byte(self, pos)
    }

    fn byte_to_position(&self, byte_offset: usize) -> Position {
        Self::byte_to_position(self, byte_offset)
    }

    fn insert_at(&mut self, pos: Position, text: &str) {
        Self::insert_at(self, pos, text);
    }

    fn delete_range(&mut self, start: Position, end: Position) -> String {
        Self::delete_range(self, start, end)
    }

    fn set_content(&mut self, content: &str) {
        Self::set_content(self, content);
    }

    fn content(&self) -> String {
        Self::content(self)
    }

    fn as_text_geometry(&self) -> &dyn TextGeometry {
        self
    }
}
