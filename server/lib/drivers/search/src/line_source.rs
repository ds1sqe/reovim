//! Line-oriented read access abstraction for search.
//!
//! [`LineSource`] generalizes over `Buffer` (Rope) and `VirtualBuffer`
//! (mmap + piece table) so that [`SearchProvider`](super::SearchProvider)
//! can operate on both without knowing which implementation it has.
//!
//! Adapters are provided in this module rather than in the kernel,
//! keeping the kernel free of driver dependencies.

use std::borrow::Cow;

use {
    reovim_kernel::api::v1::BufferOps,
    reovim_provider_text::{Buffer, VirtualBuffer},
    reovim_types_text::Position,
};

/// Line-oriented read access to buffer content.
///
/// Both `Buffer` (Rope) and `VirtualBuffer` (mmap + piece table) can be
/// wrapped in adapters implementing this trait.  `SearchProvider` methods
/// accept `&dyn LineSource` for buffer-type-agnostic search.
pub trait LineSource: Send + Sync {
    /// Number of lines in the content.
    fn line_count(&self) -> usize;

    /// Get a line by index.
    fn line(&self, idx: usize) -> Option<Cow<'_, str>>;

    /// Length of a line in characters.
    fn line_len(&self, idx: usize) -> Option<usize>;

    /// Convert a (line, column) position to a byte offset.
    fn position_to_byte(&self, pos: Position) -> usize;

    /// Convert a byte offset to a (line, column) position.
    fn byte_to_position(&self, byte_offset: usize) -> Position;

    /// Materialize full content as a string.
    ///
    /// Default implementation joins all lines with newlines.
    fn content(&self) -> String {
        let mut result = String::new();
        for i in 0..self.line_count() {
            if i > 0 {
                result.push('\n');
            }
            if let Some(line) = self.line(i) {
                result.push_str(&line);
            }
        }
        result
    }
}

/// Adapter wrapping a `Buffer` (Rope) as a [`LineSource`].
pub struct BufferLineSource<'a>(pub &'a Buffer);

impl LineSource for BufferLineSource<'_> {
    fn line_count(&self) -> usize {
        self.0.line_count()
    }

    fn line(&self, idx: usize) -> Option<Cow<'_, str>> {
        self.0.line(idx).map(|s| Cow::Owned(s.to_string()))
    }

    fn line_len(&self, idx: usize) -> Option<usize> {
        self.0.line_len(idx)
    }

    fn position_to_byte(&self, pos: Position) -> usize {
        self.0.position_to_byte(pos)
    }

    fn byte_to_position(&self, byte_offset: usize) -> Position {
        self.0.byte_to_position(byte_offset)
    }

    fn content(&self) -> String {
        self.0.content()
    }
}

/// Adapter wrapping a `VirtualBuffer` (mmap + piece table) as a [`LineSource`].
pub struct VirtualBufferLineSource<'a>(pub &'a VirtualBuffer);

impl LineSource for VirtualBufferLineSource<'_> {
    fn line_count(&self) -> usize {
        self.0.line_count()
    }

    fn line(&self, idx: usize) -> Option<Cow<'_, str>> {
        self.0.line(idx).map(Cow::Owned)
    }

    fn line_len(&self, idx: usize) -> Option<usize> {
        self.0.line(idx).map(|s| s.chars().count())
    }

    fn position_to_byte(&self, pos: Position) -> usize {
        self.0.position_to_byte(pos)
    }

    fn byte_to_position(&self, byte_offset: usize) -> Position {
        self.0.byte_to_position(byte_offset)
    }

    fn content(&self) -> String {
        self.0.content()
    }
}

/// Adapter wrapping a `&dyn BufferOps` as a [`LineSource`].
///
/// This allows search operations to work with the unified `dyn BufferOps`
/// from the buffer manager without knowing the concrete buffer type.
pub struct BufferOpsLineSource<'a>(pub &'a dyn BufferOps);

impl LineSource for BufferOpsLineSource<'_> {
    fn line_count(&self) -> usize {
        self.0.line_count()
    }

    fn line(&self, idx: usize) -> Option<Cow<'_, str>> {
        self.0.line(idx)
    }

    fn line_len(&self, idx: usize) -> Option<usize> {
        self.0.line_len(idx)
    }

    fn position_to_byte(&self, pos: Position) -> usize {
        self.0.position_to_byte(pos)
    }

    fn byte_to_position(&self, byte_offset: usize) -> Position {
        self.0.byte_to_position(byte_offset)
    }

    fn content(&self) -> String {
        self.0.content()
    }
}

#[cfg(test)]
#[path = "line_source_tests.rs"]
mod tests;
