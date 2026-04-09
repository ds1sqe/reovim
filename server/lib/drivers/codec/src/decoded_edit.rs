//! Decoded-edit abstraction for codec-local mutation pipelines.
//!
//! `DecodedEdit` represents a user-level edit in the decoded domain.
//!
//! The `Text` variant is expressed with text positions so callers can keep
//! semantic intent in terms of document coordinates. A codec translates these
//! coordinates to byte offsets when mutating canonical bytes.

use reovim_types_text::Position;

/// Domain-agnostic decoded edit representation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DecodedEdit {
    /// A text-shaped edit from a decoded text buffer.
    Text {
        /// Start position in decoded text coordinates.
        start: Position,
        /// End position in decoded text coordinates (exclusive).
        end: Position,
        /// Replacement text.
        replacement: String,
    },

    /// A direct byte-shaped edit in decoded output space.
    Bytes {
        /// Start offset in bytes.
        offset: usize,
        /// Number of old bytes to replace.
        old_len: usize,
        /// Replacement bytes.
        new_bytes: Vec<u8>,
    },

    /// Reserved for future structural editing variants.
    _Reserved,
}

impl DecodedEdit {
    /// Convenience: whether this edit replaces no old bytes.
    #[must_use]
    pub fn is_insertion(&self) -> bool {
        match self {
            Self::Text { start, end, .. } => start == end,
            Self::Bytes { old_len, .. } => *old_len == 0,
            Self::_Reserved => false,
        }
    }

    /// Convenience: whether this edit deletes old bytes and inserts no new bytes.
    #[must_use]
    pub fn is_deletion(&self) -> bool {
        match self {
            Self::Text {
                start,
                end,
                replacement,
                ..
            } => *start != *end && replacement.is_empty(),
            Self::Bytes {
                old_len, new_bytes, ..
            } => *old_len > 0 && new_bytes.is_empty(),
            Self::_Reserved => false,
        }
    }
}

#[cfg(test)]
#[path = "decoded_edit_tests.rs"]
mod tests;
