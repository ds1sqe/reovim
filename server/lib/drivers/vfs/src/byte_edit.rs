//! Byte-level edit type for VFS operations.
//!
//! [`ByteEdit`] represents a single byte-level modification to storage.
//! Codecs translate domain edits (e.g., text `Edit`) into `ByteEdit`s,
//! and the VFS byte undo log records them for universal undo.

use std::ops::Range;

/// A single byte-level edit operation.
///
/// Represents an atomic modification at a byte offset in storage.
/// Used by the VFS byte undo log and as the output of codec
/// `encode_edit` translation.
///
/// # Undo
///
/// `ByteEdit` is self-contained and invertible: swap `old_bytes` and
/// `new_bytes` to get the inverse operation.
///
/// # Example
///
/// ```
/// use reovim_driver_vfs::ByteEdit;
///
/// // Insert "hello" at byte offset 10
/// let edit = ByteEdit::insert(10, b"hello");
/// assert_eq!(edit.offset, 10);
/// assert!(edit.old_bytes.is_empty());
/// assert_eq!(edit.new_bytes, b"hello");
///
/// // Delete 3 bytes at offset 5
/// let edit = ByteEdit::delete(5, b"abc");
/// assert_eq!(edit.offset, 5);
/// assert_eq!(edit.old_bytes, b"abc");
/// assert!(edit.new_bytes.is_empty());
///
/// // Replace "foo" with "bar" at offset 0
/// let edit = ByteEdit::replace(0, b"foo", b"bar");
/// assert_eq!(edit.offset, 0);
/// assert_eq!(edit.old_bytes, b"foo");
/// assert_eq!(edit.new_bytes, b"bar");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ByteEdit {
    /// Byte offset where the edit occurs.
    pub offset: usize,
    /// Bytes that were replaced (empty for pure inserts).
    pub old_bytes: Vec<u8>,
    /// Bytes that replaced the old bytes (empty for pure deletes).
    pub new_bytes: Vec<u8>,
}

impl ByteEdit {
    /// Create a byte insertion (no bytes removed).
    #[must_use]
    pub fn insert(offset: usize, data: &[u8]) -> Self {
        Self {
            offset,
            old_bytes: Vec::new(),
            new_bytes: data.to_vec(),
        }
    }

    /// Create a byte deletion (no bytes added).
    #[must_use]
    pub fn delete(offset: usize, old: &[u8]) -> Self {
        Self {
            offset,
            old_bytes: old.to_vec(),
            new_bytes: Vec::new(),
        }
    }

    /// Create a byte replacement (old bytes → new bytes).
    #[must_use]
    pub fn replace(offset: usize, old: &[u8], new: &[u8]) -> Self {
        Self {
            offset,
            old_bytes: old.to_vec(),
            new_bytes: new.to_vec(),
        }
    }

    /// The byte range affected by this edit in the pre-edit buffer.
    #[must_use]
    pub const fn affected_range(&self) -> Range<usize> {
        self.offset..self.offset + self.old_bytes.len()
    }

    /// The inverse of this edit (for undo).
    #[must_use]
    pub fn inverse(&self) -> Self {
        Self {
            offset: self.offset,
            old_bytes: self.new_bytes.clone(),
            new_bytes: self.old_bytes.clone(),
        }
    }

    /// Whether this edit is a pure insert (no bytes removed).
    #[must_use]
    pub const fn is_insert(&self) -> bool {
        self.old_bytes.is_empty() && !self.new_bytes.is_empty()
    }

    /// Whether this edit is a pure delete (no bytes added).
    #[must_use]
    pub const fn is_delete(&self) -> bool {
        !self.old_bytes.is_empty() && self.new_bytes.is_empty()
    }

    /// Whether this edit has no effect.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.old_bytes.is_empty() && self.new_bytes.is_empty()
    }
}

#[cfg(test)]
#[path = "byte_edit_tests.rs"]
mod tests;
