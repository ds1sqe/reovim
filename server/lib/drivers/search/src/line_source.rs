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
    reovim_provider_text::{Buffer, BufferOps, VirtualBuffer},
    reovim_types_text::{Position, TextGeometry},
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

pub struct TextGeometryLineSource(Vec<String>);

impl TextGeometryLineSource {
    #[must_use]
    pub fn new(text: &dyn TextGeometry) -> Self {
        Self(
            (0..text.line_count())
                .filter_map(|idx| text.line(idx).map(Cow::into_owned))
                .collect(),
        )
    }
}

impl LineSource for TextGeometryLineSource {
    fn line_count(&self) -> usize {
        self.0.len()
    }

    fn line(&self, idx: usize) -> Option<Cow<'_, str>> {
        self.0.get(idx).map(|line| Cow::Borrowed(line.as_str()))
    }

    fn line_len(&self, idx: usize) -> Option<usize> {
        self.0.get(idx).map(|line| line.chars().count())
    }

    fn position_to_byte(&self, pos: Position) -> usize {
        let line_count = self.0.len();
        if line_count == 0 {
            return 0;
        }

        let line = pos.line.min(line_count - 1);
        let mut offset = 0;
        for idx in 0..line {
            offset += self.0[idx].len() + 1;
        }

        offset + column_to_byte_offset(&self.0[line], pos.column)
    }

    fn byte_to_position(&self, byte_offset: usize) -> Position {
        let line_count = self.0.len();
        if line_count == 0 {
            return Position::origin();
        }

        let mut offset = 0;
        for idx in 0..line_count {
            let text = &self.0[idx];
            let line_end = offset + text.len();
            if byte_offset <= line_end {
                return Position::new(idx, byte_to_column(text, byte_offset - offset));
            }
            offset = line_end + 1;
        }

        let last_line = line_count - 1;
        Position::new(last_line, self.0[last_line].chars().count())
    }
}

fn column_to_byte_offset(text: &str, column: usize) -> usize {
    text.char_indices()
        .nth(column)
        .map_or(text.len(), |(idx, _)| idx)
}

fn byte_to_column(text: &str, byte_offset: usize) -> usize {
    text.get(..byte_offset)
        .map_or_else(|| text.chars().count(), |prefix| prefix.chars().count())
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
