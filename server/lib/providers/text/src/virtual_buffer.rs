//! Virtual buffer backed by mmap + piece table for large files.
//!
//! The original file stays on disk, memory-mapped read-only via
//! [`FileMapping`].  Edits create new pieces referencing an append-only
//! add buffer.  The [`PieceTree`] is a B-tree with `Arc` structural sharing
//! (O(1) clone) for efficient snapshots.
//!
//! # Migration
//!
//! This type was moved from `reovim-kernel` as part of #740 (kernel buffer
//! extraction).  It is text-specific (uses `LineIndex` for line-based access,
//! `Position` for text coordinates) and belongs in the text provider layer.

use std::{
    borrow::Cow,
    fmt,
    hash::{Hash, Hasher},
    ops::Range,
    sync::Arc,
};

use crate::virtual_snapshot::VirtualSnapshot;

use {
    crate::{BufferCapabilities, BufferOps},
    reovim_domain_text::{LineIndex, Position, TextGeometry},
    reovim_driver_vfs::{FileMapping, Piece, PieceMetrics, PieceSource, PieceTree},
    reovim_kernel::api::v1::{BufferId, BufferMeta, StorageCapabilities, StorageError, StorageOps},
};

/// Create byte-only metrics from a string's byte length.
const fn byte_metrics(s: &str) -> PieceMetrics {
    PieceMetrics::from_byte_len(s.len() as u64)
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
    /// Create a `VirtualBuffer` from a file mapping, validating UTF-8.
    ///
    /// Builds a [`LineIndex`] internally from the mapped bytes.  This is
    /// the preferred constructor — callers do not need to construct or
    /// import `LineIndex` themselves.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidUtf8`] if the mapped bytes are not valid UTF-8.
    pub fn from_mapping(
        mapping: Arc<dyn FileMapping>,
    ) -> Result<Self, reovim_domain_text::InvalidUtf8> {
        let line_index = LineIndex::from_bytes(mapping.as_bytes())?;
        Ok(Self::new(mapping, line_index))
    }

    /// Create a new `VirtualBuffer` from a file mapping and a pre-built
    /// line index.
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
        let piece = Piece {
            source: PieceSource::Original {
                byte_start: 0,
                byte_len: file_size,
            },
            metrics: PieceMetrics::from_byte_len(file_size),
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
    ///
    /// # Panics
    ///
    /// Panics if the backing bytes are not valid UTF-8.  This should never
    /// happen: original content is validated at construction, and the add
    /// buffer is always a `String`.
    #[must_use]
    pub fn content(&self) -> String {
        let total = self.pieces.byte_len() as usize;
        let mut buf = Vec::with_capacity(total);
        for piece in self.pieces.iter_pieces() {
            buf.extend_from_slice(self.resolve_piece_bytes(piece));
        }
        // All backing bytes are valid UTF-8: original validated at construction,
        // add_buffer is always a String.
        let s = String::from_utf8(buf).expect("backing bytes are valid UTF-8");
        if self.crlf {
            s.replace("\r\n", "\n")
        } else {
            s
        }
    }

    /// Materialize a byte range from pieces as a `String`.
    ///
    /// Walks only the pieces that overlap the requested range and copies
    /// the relevant sub-slices directly from the backing stores (mmap /
    /// add buffer) without allocating intermediate per-piece Strings.
    fn materialize_byte_range(&self, byte_start: u64, byte_end: u64) -> String {
        if byte_start >= byte_end {
            return String::new();
        }

        let range_len = (byte_end - byte_start) as usize;
        let mut buf = Vec::with_capacity(range_len);

        self.collect_bytes_in_range(byte_start, byte_end, &mut buf);

        // All backing bytes are valid UTF-8: original validated at
        // construction, add_buffer is always a String.
        String::from_utf8(buf).expect("backing bytes are valid UTF-8")
    }

    /// Copy bytes from pieces in `[byte_start, byte_end)` into `buf`.
    fn collect_bytes_in_range(&self, byte_start: u64, byte_end: u64, buf: &mut Vec<u8>) {
        let mut cumulative = 0u64;

        for piece in self.pieces.iter_pieces() {
            let piece_start = cumulative;
            let piece_end = cumulative + piece.metrics.byte_len;
            cumulative = piece_end;

            if piece_end <= byte_start {
                continue;
            }
            if piece_start >= byte_end {
                break;
            }

            let local_start = byte_start.saturating_sub(piece_start) as usize;
            let local_end = (byte_end - piece_start).min(piece.metrics.byte_len) as usize;

            let bytes = self.resolve_piece_bytes(piece);
            buf.extend_from_slice(&bytes[local_start..local_end]);
        }
    }

    /// Resolve a piece to its backing byte slice without allocation.
    ///
    /// For `Original` pieces this returns a sub-slice of the mmap.
    /// For `Add` pieces this returns a sub-slice of the append buffer.
    fn resolve_piece_bytes<'a>(&'a self, piece: &Piece) -> &'a [u8] {
        match piece.source {
            PieceSource::Original {
                byte_start,
                byte_len,
            } => {
                let bytes = self.original.as_bytes();
                let start = byte_start as usize;
                let end = start + byte_len as usize;
                &bytes[start..end]
            }
            PieceSource::Add { offset, len } => &self.add_buffer.as_bytes()[offset..offset + len],
        }
    }

    /// Invoke a closure with each contiguous byte chunk of the buffer.
    ///
    /// This yields raw `&[u8]` slices directly from the backing stores
    /// without any String allocation — suitable for byte-level regex
    /// search or streaming output.
    pub fn for_each_chunk(&self, mut f: impl FnMut(&[u8])) {
        for piece in self.pieces.iter_pieces() {
            f(self.resolve_piece_bytes(piece));
        }
    }
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

        let piece = Piece {
            source: PieceSource::Add {
                offset: add_offset,
                len: add_len,
            },
            metrics: byte_metrics(text),
        };

        self.pieces = self.pieces.insert(byte_offset, piece);
        self.modified = true;

        self.line_index.apply_insert(byte_offset, text.as_bytes());
    }

    /// Delete text at a position.
    ///
    /// Returns the deleted text.
    pub fn delete_at(&mut self, pos: Position, count: usize) -> String {
        if count == 0 || self.is_empty() {
            return String::new();
        }

        let byte_start = self.position_to_byte(pos) as u64;
        let total_bytes = self.pieces.byte_len();

        // Find byte_end by counting `count` chars from byte_start without
        // materializing the entire buffer.  We read a chunk large enough
        // to cover the requested char count (worst case: 4 bytes per char
        // for UTF-8) and scan it.
        let estimate = (byte_start + count as u64 * 4).min(total_bytes);
        let chunk = self.materialize_byte_range(byte_start, estimate);

        let byte_end = if let Some((i, _)) = chunk.char_indices().nth(count) {
            byte_start + i as u64
        } else {
            // Fewer chars than requested — delete to end of chunk.
            // If the estimate didn't cover enough, extend to total.
            if estimate < total_bytes {
                let rest = self.materialize_byte_range(estimate, total_bytes);
                let remaining = count - chunk.chars().count();
                let extra = rest
                    .char_indices()
                    .nth(remaining)
                    .map_or(rest.len(), |(i, _)| i);
                estimate + extra as u64
            } else {
                byte_start + chunk.len() as u64
            }
        };

        if byte_start >= byte_end {
            return String::new();
        }

        let deleted = self.materialize_byte_range(byte_start, byte_end);
        let delete_len = byte_end - byte_start;

        self.pieces = self.pieces.delete(byte_start, delete_len);
        self.modified = true;

        self.line_index.apply_delete(byte_start, byte_end);

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

        self.line_index.apply_delete(byte_start, byte_end);

        deleted
    }

    /// Set the full content from a string.
    pub fn set_content(&mut self, content: &str) {
        self.add_buffer.clear();
        self.add_buffer.push_str(content);

        let piece = Piece {
            source: PieceSource::Add {
                offset: 0,
                len: content.len(),
            },
            metrics: byte_metrics(content),
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
        let content = self.content_bytes_vec();
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
                    // Pieces are created to cover exactly the bytes appended to
                    // add_buffer, so `offset..offset+len` is always in-bounds.
                    // The `get()` + `unwrap_or_default()` is a defensive
                    // fallback that cannot be reached through the public API.
                    let slice = add_buffer_slice_safe(self.add_buffer.as_bytes(), offset, len);
                    writer.write_all(slice)?;
                }
            }
        }
        Ok(())
    }

    /// Materialize content as bytes (for line index rebuild and `StorageOps`).
    fn content_bytes_vec(&self) -> Vec<u8> {
        let mut result = Vec::with_capacity(self.pieces.byte_len() as usize);
        for piece in self.pieces.iter_pieces() {
            result.extend_from_slice(self.resolve_piece_bytes(piece));
        }
        result
    }
}

// ── write_to helper ─────────────────────────────────────────────────────────

/// Return a slice of `buf` at `offset..offset+len`, or an empty slice if the
/// range is out of bounds.
///
/// In practice the add-buffer pieces always reference valid ranges that were
/// produced by `insert_at` / `set_content`.  This function exists solely as a
/// defensive fallback that satisfies the borrow-checker without a panic path;
/// it is excluded from MC/DC coverage because the `unwrap_or_default` branch
/// cannot be reached through the public API.
#[cfg_attr(coverage_nightly, coverage(off))]
fn add_buffer_slice_safe(buf: &[u8], offset: usize, len: usize) -> &[u8] {
    buf.get(offset..offset + len).unwrap_or_default()
}

// ── Snapshot ────────────────────────────────────────────────────────────────

impl VirtualBuffer {
    /// Capture an opaque snapshot of this buffer's state.
    #[must_use]
    pub fn capture_snapshot(&self) -> VirtualSnapshot {
        VirtualSnapshot::new(
            self.pieces.clone(),
            self.add_buffer.len(),
            Arc::clone(&self.original),
            self.line_index.clone(),
            self.crlf,
        )
    }

    /// Restore state from a snapshot.
    pub fn restore_snapshot(&mut self, snap: VirtualSnapshot) {
        let (pieces, add_buffer_len, original, line_index, crlf) = snap.into_parts();
        self.pieces = pieces;
        self.add_buffer.truncate(add_buffer_len);
        self.original = original;
        self.line_index = line_index;
        self.crlf = crlf;
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
        let total = self.pieces.byte_len() as usize;
        if offset >= total {
            return 0;
        }
        let end = (offset + buf.len()).min(total);
        let mut tmp = Vec::with_capacity(end - offset);
        self.collect_bytes_in_range(offset as u64, end as u64, &mut tmp);
        let count = tmp.len().min(buf.len());
        buf[..count].copy_from_slice(&tmp[..count]);
        count
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
        let mut buf = Vec::with_capacity(end - offset);
        self.collect_bytes_in_range(offset as u64, end as u64, &mut buf);
        buf
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
    fn buffer_capabilities(&self) -> BufferCapabilities {
        BufferCapabilities::VIRTUAL
    }

    fn content_bytes(&self) -> Vec<u8> {
        self.content_bytes_vec()
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

    fn write_to(&self, writer: &mut dyn std::io::Write) -> Result<(), std::io::Error> {
        Self::write_to(self, writer)
    }

    fn as_text_geometry(&self) -> &dyn TextGeometry {
        self
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
