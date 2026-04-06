//! Text buffer operations trait.
//!
//! `BufferOps` extends `StorageOps + BufferMeta` with text-specific methods.
//! Both Rope-backed `Buffer` and mmap-backed `VirtualBuffer` implement this
//! trait. The session-layer `TextBufferRegistry` stores `Arc<RwLock<dyn BufferOps>>`.
//!
//! # Architecture
//!
//! Byte-level I/O and identity come from supertraits (`StorageOps`, `BufferMeta`)
//! which are defined in `reovim-kernel`. `BufferOps` adds only text-specific
//! operations (line access, position conversion, text mutations).
//!
//! ```text
//! StorageOps (byte I/O)  ─┐
//!                          ├─ BufferOps (text-specific extension)
//! BufferMeta (identity)  ─┘
//! ```

use std::borrow::Cow;

use {
    crate::BufferCapabilities,
    reovim_kernel::api::v1::{BufferMeta, StorageOps},
    reovim_types_text::{Position, TextGeometry},
};

/// Text buffer operations extending byte storage with text-specific methods.
///
/// All text buffer types (Rope, mmap-backed) implement this trait. The
/// session-layer `TextBufferRegistry` stores `Arc<RwLock<dyn BufferOps>>`,
/// enabling polymorphic text buffer access.
///
/// Byte-level I/O and metadata methods are inherited from `StorageOps` and
/// `BufferMeta` supertraits. This trait adds only text-specific operations.
///
/// # Object Safety
///
/// This trait is object-safe: all methods use `&self`/`&mut self`, return
/// owned types, and have no generic parameters.
pub trait BufferOps: StorageOps + BufferMeta {
    // === Text-Specific Capabilities ===

    /// Text-level capability flags for this buffer type.
    ///
    /// For byte-level capabilities, use `StorageOps::capabilities()`.
    fn buffer_capabilities(&self) -> BufferCapabilities;

    // === Line-Level Access ===

    /// Number of lines.
    fn line_count(&self) -> usize;

    /// Get a line by 0-based index.
    fn line(&self, idx: usize) -> Option<Cow<'_, str>>;

    /// Length of a line in characters.
    fn line_len(&self, idx: usize) -> Option<usize>;

    /// Full content as bytes.
    fn content_bytes(&self) -> Vec<u8>;

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
