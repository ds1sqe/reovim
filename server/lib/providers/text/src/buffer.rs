//! Buffer data structure for text storage.
//!
//! The buffer is the core abstraction for text editing. It stores
//! text as a rope data structure providing O(log n) insert/delete,
//! O(1) clone via structural sharing, and O(log n) position conversion.
//!
//! # Cursor Isolation (#471)
//!
//! **Buffer does NOT track cursor position.** Cursor is per-client UI state
//! that lives in `Window`, not in the shared kernel Buffer. This follows the
//! mechanism vs policy principle:
//! - **Mechanism**: Buffer provides text storage operations
//! - **Policy**: Window/Session decides cursor position (per-client)
//!
//! Use `SessionRuntime::cursor_position(buffer_id)` to get cursor from Window.

use std::{
    borrow::Cow,
    hash::{Hash, Hasher},
};

use {
    crate::{BufferCapabilities, BufferOps},
    reovim_domain_text::{Position, TextGeometry},
    reovim_kernel::api::v1::{BufferId, BufferMeta, StorageCapabilities, StorageError, StorageOps},
};

use super::{BufferSnapshot, rope::Rope};

/// A text buffer with rope-based storage.
///
/// The buffer stores text as a rope — a balanced B-tree of text chunks.
/// This provides O(log n) insert/delete, O(1) clone via structural sharing
/// (`Arc<Node>`), and O(log n) position conversion.
///
/// # Invariants
///
/// - An empty buffer has zero lines (not one empty line)
/// - Positions are clamped to valid ranges on access
///
/// # Cursor Isolation (#471)
///
/// Buffer does NOT have a cursor field. Cursor is per-client state in Window.
/// All edit operations take explicit positions instead of using an internal cursor.
///
/// # Example
///
/// ```
/// use reovim_provider_text::Buffer;
/// # use reovim_domain_text::Position;
///
/// let mut buf = Buffer::from_string("Hello\nWorld");
/// assert_eq!(buf.line_count(), 2);
/// assert_eq!(buf.line(0), Some("Hello"));
///
/// buf.insert_at(Position::new(0, 5), "!");
/// assert_eq!(buf.line(0), Some("Hello!"));
/// ```
#[derive(Debug, Clone)]
pub struct Buffer {
    /// Unique identifier for this buffer.
    id: BufferId,
    /// Text content stored as a rope.
    text: Rope,
    /// Whether the buffer has unsaved modifications.
    modified: bool,
    /// File path associated with this buffer.
    file_path: Option<String>,
    // NOTE: Cursor removed in #471 (per-client cursor isolation).
    // Cursor now lives in Window (per-client state), not Buffer.
    // See: ext/server/drivers/text-session/src/types.rs - Window.cursor
    //
    // NOTE: Selection removed in Phase 8 (#465).
    // Selection now lives in Window (per-window state), not Buffer.
    // See: ext/server/drivers/text-session/src/types.rs - Window.selection
}

impl Buffer {
    /// Create a new empty buffer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: BufferId::new(),
            text: Rope::new(),
            modified: false,
            file_path: None,
        }
    }

    /// Create a buffer with a specific ID.
    ///
    /// This is primarily useful for testing.
    #[must_use]
    pub fn with_id(id: BufferId) -> Self {
        Self {
            id,
            text: Rope::new(),
            modified: false,
            file_path: None,
        }
    }

    /// Create a buffer from a string.
    ///
    /// The string is split by newlines into lines.
    /// An empty string results in a buffer with zero lines.
    #[must_use]
    pub fn from_string(content: &str) -> Self {
        Self {
            id: BufferId::new(),
            text: normalize_to_rope(content),
            modified: false,
            file_path: None,
        }
    }

    // === Accessors ===

    /// Get the buffer ID.
    #[must_use]
    pub const fn id(&self) -> BufferId {
        self.id
    }

    // NOTE: cursor(), cursor_mut(), position(), set_position() removed in #471.
    // Cursor is per-client state in Window, not Buffer.
    // Use SessionRuntime::cursor_position(buffer_id) via BufferApi trait.

    /// Check if the buffer has unsaved modifications.
    #[must_use]
    pub const fn is_modified(&self) -> bool {
        self.modified
    }

    /// Mark the buffer as modified or unmodified.
    pub const fn set_modified(&mut self, modified: bool) {
        self.modified = modified;
    }

    // NOTE: Selection methods removed in Phase 8 (#465).
    // Selection now lives in Window (per-window state), not Buffer.
    // Use SessionRuntime::selection(buffer_id) via BufferApi trait.

    // === File Path ===

    /// Get the file path associated with this buffer.
    #[must_use]
    pub fn file_path(&self) -> Option<&str> {
        self.file_path.as_deref()
    }

    /// Set the file path for this buffer.
    pub fn set_file_path(&mut self, path: Option<String>) {
        self.file_path = path;
    }

    // === Line Hashing ===

    /// Compute hash of a line for cache validation.
    ///
    /// Uses `DefaultHasher` for speed over cryptographic strength.
    /// Returns `None` if line index is out of bounds.
    #[must_use]
    pub fn line_hash(&self, line_idx: usize) -> Option<u64> {
        use std::collections::hash_map::DefaultHasher;

        self.line(line_idx).map(|line| {
            let mut hasher = DefaultHasher::new();
            line.hash(&mut hasher);
            hasher.finish()
        })
    }

    /// Get all line hashes (for saturator requests).
    #[must_use]
    pub fn line_hashes(&self) -> Vec<u64> {
        (0..self.line_count())
            .filter_map(|idx| self.line_hash(idx))
            .collect()
    }

    // === Line Access ===

    /// Get the number of lines in the buffer.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.text.line_count()
    }

    /// Check if the buffer is empty (has no lines).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Get a specific line by index.
    ///
    /// Returns `None` if the index is out of bounds.
    #[must_use]
    pub fn line(&self, index: usize) -> Option<&str> {
        self.text.line(index)
    }

    /// Get the length of a specific line in characters.
    ///
    /// Returns `None` if the index is out of bounds.
    #[must_use]
    pub fn line_len(&self, index: usize) -> Option<usize> {
        self.text.line_len(index)
    }

    // NOTE: lines() -> &[String] removed in #711.
    // With rope storage, there is no contiguous &[String] to return.
    // Use line(idx) for individual access or content() for full text.

    /// Clone the internal rope (O(1) via `Arc` sharing).
    ///
    /// Used by snapshot types for efficient state capture.
    #[must_use]
    pub fn clone_rope(&self) -> Rope {
        self.text.clone()
    }

    /// Get the full content as a string (lines joined with newlines).
    #[must_use]
    pub fn content(&self) -> String {
        self.text.content()
    }

    /// Set the full content from a string.
    ///
    /// This replaces all existing content.
    pub fn set_content(&mut self, content: &str) {
        self.text = normalize_to_rope(content);
        self.modified = true;
    }

    // === Edit Operations ===
    //
    // NOTE: insert() and delete() convenience methods removed in #471.
    // These methods relied on internal cursor which is now per-client in Window.
    // Use insert_at(pos, text) and delete_at(pos, count) with explicit positions.

    /// Insert text at a specific position.
    ///
    /// This is a pure text operation - cursor management is the caller's responsibility.
    pub fn insert_at(&mut self, pos: Position, text: &str) {
        if text.is_empty() {
            return;
        }

        if self.text.is_empty() {
            self.text = Rope::from_str(text);
            self.modified = true;
            return;
        }

        let pos = self.clamp_position(pos);
        let byte_offset = self.text.position_to_byte(pos.line, pos.column);
        self.text = self.text.insert(byte_offset, text);
        self.modified = true;
    }

    /// Delete text at a specific position.
    ///
    /// Returns the deleted text.
    /// This is a pure text operation - cursor management is the caller's responsibility.
    pub fn delete_at(&mut self, pos: Position, count: usize) -> String {
        if count == 0 || self.text.is_empty() {
            return String::new();
        }

        let pos = self.clamp_position(pos);
        let byte_start = self.text.position_to_byte(pos.line, pos.column);
        let char_start = self.text.byte_to_char(byte_start);
        let char_end = (char_start + count).min(self.text.char_len());
        let byte_end = self.text.char_to_byte(char_end);

        if byte_start >= byte_end {
            return String::new();
        }

        let deleted = extract_byte_range(&self.text, byte_start, byte_end);
        self.text = self.text.remove(byte_start..byte_end);
        // byte_start < byte_end (guard above), so deleted is always non-empty.
        self.modified = true;

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

        let start = self.clamp_position(start);
        let end = self.clamp_position(end);

        let byte_start = self.text.position_to_byte(start.line, start.column);
        let byte_end = self.text.position_to_byte(end.line, end.column);

        if byte_start >= byte_end {
            return String::new();
        }

        let deleted = extract_byte_range(&self.text, byte_start, byte_end);
        self.text = self.text.remove(byte_start..byte_end);
        // byte_start < byte_end (guard above), so deleted is always non-empty.
        self.modified = true;

        deleted
    }

    // === Position Conversion ===

    /// Convert a position to a byte offset in the full content.
    ///
    /// This is useful for tree-sitter and other byte-based APIs.
    #[must_use]
    pub fn position_to_byte(&self, pos: Position) -> usize {
        if self.text.is_empty() {
            return 0;
        }
        let pos = self.clamp_position(pos);
        self.text.position_to_byte(pos.line, pos.column)
    }

    /// Convert a byte offset to a position.
    #[must_use]
    pub fn byte_to_position(&self, byte_offset: usize) -> Position {
        if self.text.is_empty() {
            return Position::new(0, 0);
        }
        let (line, col) = self.text.byte_to_position(byte_offset);
        Position::new(line, col)
    }

    // === Helper Methods ===

    /// Clamp a position to valid buffer coordinates.
    #[must_use]
    fn clamp_position(&self, pos: Position) -> Position {
        if self.text.is_empty() {
            return Position::origin();
        }

        let line = pos.line.min(self.text.line_count() - 1);
        let max_col = self.text.line_len(line).unwrap_or(0);
        let column = pos.column.min(max_col);

        Position::new(line, column)
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

// === Helper Functions ===

/// Normalize content string into a rope.
///
/// Uses `str::lines()` to split and rejoin, which strips the optional
/// final newline — matching the old `Vec<String>` buffer behavior where
/// `content()` was `lines.join("\n")`.
fn normalize_to_rope(content: &str) -> Rope {
    if content.is_empty() {
        return Rope::new();
    }
    let joined: String = content.lines().collect::<Vec<_>>().join("\n");
    if joined.is_empty() {
        Rope::new()
    } else {
        Rope::from_str(&joined)
    }
}

/// Extract text in a byte range from a rope by iterating over chunks.
fn extract_byte_range(text: &Rope, start: usize, end: usize) -> String {
    let mut result = String::with_capacity(end - start);
    let mut pos = 0;
    for chunk in text.chunks() {
        let chunk_end = pos + chunk.len();
        if chunk_end <= start {
            pos = chunk_end;
            continue;
        }
        if pos >= end {
            break;
        }
        let s = start.saturating_sub(pos);
        let e = (end - pos).min(chunk.len());
        result.push_str(&chunk[s..e]);
        pos = chunk_end;
    }
    result
}

// ── TextGeometry ────────────────────────────────────────────────────────────

impl TextGeometry for Buffer {
    fn line_count(&self) -> usize {
        self.text.line_count()
    }

    fn line(&self, idx: usize) -> Option<Cow<'_, str>> {
        self.text.line(idx).map(Cow::Borrowed)
    }

    fn line_len(&self, idx: usize) -> Option<usize> {
        self.text.line_len(idx)
    }

    fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

// ── BufferOps ──────────────────────────────────────────────────────────────

// StorageOps, BufferMeta, BufferOps, etc. imported at the top of the file.

// ── StorageOps + BufferMeta (#740) ────────────────────────────────────────

impl StorageOps for Buffer {
    fn byte_len(&self) -> usize {
        self.text.byte_len()
    }

    fn read_bytes(&self, offset: usize, buf: &mut [u8]) -> usize {
        let total = self.text.byte_len();
        if offset >= total {
            return 0;
        }
        let available = total - offset;
        let count = buf.len().min(available);
        let mut written = 0;
        let mut pos = 0;
        for chunk in self.text.chunks() {
            let chunk_end = pos + chunk.len();
            if chunk_end <= offset {
                pos = chunk_end;
                continue;
            }
            if written >= count {
                break;
            }
            let start = offset.saturating_sub(pos);
            let end = (count - written).min(chunk.len() - start);
            buf[written..written + end].copy_from_slice(&chunk.as_bytes()[start..start + end]);
            written += end;
            pos = chunk_end;
        }
        written
    }

    fn capabilities(&self) -> StorageCapabilities {
        StorageCapabilities::HEAP
    }

    fn insert_bytes(&mut self, offset: usize, data: &[u8]) -> Result<(), StorageError> {
        let text = std::str::from_utf8(data)
            .map_err(|_| StorageError::NotSupported("non-UTF-8 insert into text buffer"))?;
        if text.is_empty() {
            return Ok(());
        }
        let total = self.text.byte_len();
        if offset > total {
            return Err(StorageError::OffsetOutOfRange { offset, len: total });
        }
        if self.text.is_empty() {
            self.text = Rope::from_str(text);
        } else {
            self.text = self.text.insert(offset, text);
        }
        self.modified = true;
        Ok(())
    }

    fn delete_bytes(&mut self, offset: usize, len: usize) -> Result<Vec<u8>, StorageError> {
        let total = self.text.byte_len();
        if offset + len > total {
            return Err(StorageError::OffsetOutOfRange { offset, len: total });
        }
        if len == 0 {
            return Ok(Vec::new());
        }
        let end = offset + len;
        let deleted = extract_byte_range(&self.text, offset, end).into_bytes();
        self.text = self.text.remove(offset..end);
        self.modified = true;
        Ok(deleted)
    }

    fn append_bytes(&mut self, data: &[u8]) -> Result<(), StorageError> {
        let offset = self.text.byte_len();
        StorageOps::insert_bytes(self, offset, data)
    }

    fn read_chunk(&self, offset: usize, max_len: usize) -> Vec<u8> {
        let total = self.text.byte_len();
        if offset >= total {
            return Vec::new();
        }
        let end = (offset + max_len).min(total);
        extract_byte_range(&self.text, offset, end).into_bytes()
    }
}

impl BufferMeta for Buffer {
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

impl BufferOps for Buffer {
    fn buffer_capabilities(&self) -> BufferCapabilities {
        BufferCapabilities::ROPE
    }

    fn content_bytes(&self) -> Vec<u8> {
        self.text.content().into_bytes()
    }

    fn line_count(&self) -> usize {
        self.text.line_count()
    }

    fn line(&self, idx: usize) -> Option<Cow<'_, str>> {
        self.text.line(idx).map(Cow::Borrowed)
    }

    fn line_len(&self, idx: usize) -> Option<usize> {
        self.text.line_len(idx)
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

// ── BufferSnapshot helper ──────────────────────────────────────────────────

use reovim_domain_text::Cursor;

impl Buffer {
    /// Create a `BufferSnapshot` from this buffer.
    ///
    /// Cursor must be passed explicitly - get it from Window.
    /// This is O(1) via `Arc` rope sharing.
    #[must_use]
    pub fn snapshot(&self, cursor: Cursor) -> BufferSnapshot {
        BufferSnapshot::from_parts(
            self.id(),
            self.clone_rope(),
            cursor,
            self.file_path().map(String::from),
            self.is_modified(),
        )
    }
}

#[cfg(test)]
#[path = "tests/buffer.rs"]
mod tests;
