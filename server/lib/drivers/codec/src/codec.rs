//! Content codec trait.
//!
//! Defines the [`ContentCodec`] trait for decoding raw bytes into text
//! and encoding text back into raw bytes. Codecs handle the translation
//! between on-disk byte representation and the internal UTF-8 text
//! representation used by the kernel's buffer.

use reovim_driver_annotation::Annotation;

use crate::{CodecError, CodecMetadata};

/// Result of decoding raw bytes into text.
///
/// Contains the decoded text content, any annotations (e.g., hex dump
/// address/byte/ASCII annotations), metadata for round-trip encoding,
/// and flags indicating whether the decode was lossy or should be readonly.
///
/// # Invariant
///
/// If `lossy` is `true`, then `readonly` MUST also be `true`. A lossy
/// decode means the original bytes cannot be faithfully reconstructed,
/// so writing would corrupt data.
#[derive(Debug, Clone)]
pub struct DecodeResult {
    /// Decoded text content (always valid UTF-8).
    pub content: String,

    /// Annotations for the decoded content.
    ///
    /// Used by codecs like hex dump to emit address, byte, and ASCII
    /// annotations for syntax highlighting.
    pub annotations: Vec<Annotation>,

    /// Metadata for round-trip encoding.
    pub metadata: CodecMetadata,

    /// Whether the decode lost information.
    ///
    /// When `true`, the decoded text is a lossy representation of the
    /// original bytes (e.g., hex dump, or encoding with replacement
    /// characters). The buffer MUST be readonly.
    pub lossy: bool,

    /// Whether the buffer should be read-only.
    ///
    /// Set to `true` for lossy decodes or one-way codecs (like hex dump).
    pub readonly: bool,

    /// Whether the output was truncated due to input size limits.
    ///
    /// When `true`, the codec processed only a prefix of the input bytes.
    /// The `metadata` field contains `"truncated_at"` with the byte offset
    /// where truncation occurred, and `"total_size"` with the original
    /// input size.
    pub truncated: bool,
}

impl DecodeResult {
    /// Validate the lossy/readonly invariant.
    ///
    /// Returns `true` if the invariant holds: lossy implies readonly.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        // lossy => readonly
        !self.lossy || self.readonly
    }
}

/// Trait for encoding and decoding file content.
///
/// Implementations handle the translation between on-disk byte
/// representation and the internal UTF-8 representation. Each codec
/// handles a specific content type (e.g., UTF-8, hex dump, EUC-KR).
///
/// # Bidirectional vs One-Way
///
/// - **Bidirectional** codecs (e.g., UTF-8, EUC-KR) support both
///   `decode()` and `encode()`. Files can be edited and saved.
/// - **One-way** codecs (e.g., hex dump) only support `decode()`.
///   `encode()` returns `None`, and the buffer is marked readonly.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` for use across async tasks.
pub trait ContentCodec: Send + Sync {
    /// Decode raw bytes into text.
    ///
    /// # Errors
    ///
    /// Returns [`CodecError`] if the bytes cannot be decoded.
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, CodecError>;

    /// Encode text back into raw bytes.
    ///
    /// Returns:
    /// - `Some(Ok(bytes))` — successful encode
    /// - `Some(Err(error))` — encode failed (e.g., unmappable character)
    /// - `None` — codec is one-way (decode only)
    ///
    /// # Arguments
    ///
    /// * `content` — The text content to encode
    /// * `metadata` — Metadata from the original decode (for round-trip fidelity)
    fn encode(
        &self,
        content: &str,
        metadata: &CodecMetadata,
    ) -> Option<Result<Vec<u8>, CodecError>>;
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
