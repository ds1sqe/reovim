//! Unified buffer operations trait.
//!
//! `BufferOps` is the kernel-level contract for all buffer types. Both Rope-backed
//! `Buffer` and mmap-backed `VirtualBuffer` implement this trait. The unified
//! `BufferManager` stores `Arc<RwLock<dyn BufferOps>>`.
//!
//! # Design
//!
//! The trait includes both byte-level I/O and line-level convenience methods.
//! Line-level methods are included because all buffer types already implement them,
//! avoiding a separate `TextDriver` crate. A blanket `TextGeometry` impl covers
//! the read-only subset automatically.
//!
//! # Layer Model
//!
//! ```text
//! Kernel:  BufferOps (byte + line storage)
//! Driver:  SearchProvider, BufferApi (consume dyn BufferOps)
//! Module:  Operators, commands (use BufferApi or dyn BufferOps)
//! ```

use std::{borrow::Cow, fmt};

use crate::{api::BufferCapabilities, mm::{BufferId, Position}};
use reovim_types_text::TextGeometry;

/// Error type for `BufferOps` operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BufferOpsError {
    /// Attempted to insert non-UTF-8 bytes into a UTF-8 buffer.
    InvalidUtf8,
    /// Byte offset out of range.
    OffsetOutOfRange { offset: usize, len: usize },
}

impl fmt::Display for BufferOpsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUtf8 => write!(f, "invalid UTF-8 data"),
            Self::OffsetOutOfRange { offset, len } => {
                write!(f, "offset {offset} out of range (len {len})")
            }
        }
    }
}

impl std::error::Error for BufferOpsError {}

/// Unified buffer storage interface.
///
/// All buffer types (Rope, mmap-backed, future types) implement this trait.
/// The kernel `BufferManager` stores `Arc<RwLock<dyn BufferOps>>`, enabling
/// polymorphic buffer access without type-specific dispatch.
///
/// # Object Safety
///
/// This trait is object-safe: all methods use `&self`/`&mut self`, return
/// owned types, and have no generic parameters.
pub trait BufferOps: Send + Sync + 'static {
    // === Identity ===

    /// Unique buffer identifier.
    fn id(&self) -> BufferId;

    // === Byte-Level I/O ===

    /// Total byte length of buffer content.
    fn byte_len(&self) -> usize;

    /// Whether the buffer has zero bytes.
    fn is_byte_empty(&self) -> bool {
        self.byte_len() == 0
    }

    /// Read bytes starting at `offset` into `buf`.
    ///
    /// Returns the number of bytes actually read (may be less than `buf.len()`
    /// if offset + `buf.len()` exceeds buffer length).
    fn read_bytes(&self, offset: usize, buf: &mut [u8]) -> usize;

    /// Insert raw bytes at `offset`.
    ///
    /// # Errors
    ///
    /// Returns `InvalidUtf8` if data is not valid UTF-8 (for text buffers).
    fn insert_bytes(&mut self, offset: usize, data: &[u8]) -> Result<(), BufferOpsError>;

    /// Delete `len` bytes starting at `offset`, returning deleted bytes.
    fn delete_bytes(&mut self, offset: usize, len: usize) -> Vec<u8>;

    /// Full content as bytes.
    fn content_bytes(&self) -> Vec<u8>;

    // === Metadata ===

    /// Whether the buffer has unsaved modifications.
    fn is_modified(&self) -> bool;

    /// Mark the buffer as modified or unmodified.
    fn set_modified(&mut self, modified: bool);

    /// File path associated with this buffer.
    fn file_path(&self) -> Option<&str>;

    /// Set the file path for this buffer.
    fn set_file_path(&mut self, path: Option<String>);

    /// Capability flags for this buffer type.
    fn capabilities(&self) -> BufferCapabilities;

    // === Line-Level Access ===

    /// Number of lines.
    fn line_count(&self) -> usize;

    /// Get a line by 0-based index.
    fn line(&self, idx: usize) -> Option<Cow<'_, str>>;

    /// Length of a line in characters.
    fn line_len(&self, idx: usize) -> Option<usize>;

    // === Position Conversion ===

    /// Convert a (line, column) position to a byte offset.
    fn position_to_byte(&self, pos: Position) -> usize;

    /// Convert a byte offset to a (line, column) position.
    fn byte_to_position(&self, byte_offset: usize) -> Position;

    // === Text-Level Mutations ===

    /// Insert text at a position.
    fn insert_at(&mut self, pos: Position, text: &str);

    /// Delete text in a range, returning deleted text.
    fn delete_range(&mut self, start: Position, end: Position) -> String;

    /// Replace all content.
    fn set_content(&mut self, content: &str);

    /// Full content as a string.
    fn content(&self) -> String;

    // === TextGeometry Bridge ===

    /// Upcast to `TextGeometry` for motion and text object calculations.
    ///
    /// Concrete types implement this as `self` since they already implement
    /// `TextGeometry`. This method bridges `dyn BufferOps` → `&dyn TextGeometry`
    /// which Rust can't do via automatic vtable coercion.
    fn as_text_geometry(&self) -> &dyn TextGeometry;
}

#[cfg(test)]
#[path = "tests/buffer_ops.rs"]
mod tests;
