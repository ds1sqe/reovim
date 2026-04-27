//! Codec error types.

use std::fmt;

/// Errors that can occur during codec operations.
///
/// Codec operations can fail for various reasons: invalid byte sequences
/// during decoding, unmappable characters during encoding, or general
/// I/O issues.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    /// Invalid byte sequence at the given offset.
    ///
    /// Returned when the decoder encounters bytes that don't form a valid
    /// character in the expected encoding.
    InvalidSequence {
        /// Byte offset where the invalid sequence starts.
        offset: usize,
        /// Human-readable description of the error.
        detail: String,
    },

    /// Character cannot be represented in the target encoding.
    ///
    /// Returned when the encoder encounters a Unicode character that has
    /// no mapping in the target encoding (e.g., a CJK character in Latin-1).
    UnmappableCharacter {
        /// Byte offset in the source string.
        offset: usize,
        /// Human-readable description of the error.
        detail: String,
    },

    /// General codec error.
    Other(String),
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSequence { offset, detail } => {
                write!(f, "invalid byte sequence at offset {offset}: {detail}")
            }
            Self::UnmappableCharacter { offset, detail } => {
                write!(f, "unmappable character at offset {offset}: {detail}")
            }
            Self::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for CodecError {}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
