//! Domain-generic codec traits.
//!
//! These traits enable content codecs that work with any domain (text, audio,
//! video) without this crate knowing about specific domain types. The domain's
//! associated types ([`Domain::Position`], [`Domain::Edit`], [`Domain::Content`])
//! flow through as generics.
//!
//! # Trait Decomposition
//!
//! Each capability is a separate trait, allowing codecs to implement only
//! what they support:
//!
//! | Codec | [`Decode`] | [`Encode`] | [`Index`] |
//! |-------|:----------:|:----------:|:---------:|
//! | UTF-8 | yes | yes | yes |
//! | Hex dump | yes | — | — |
//! | PDF | yes | — | — |
//! | PCM (future) | yes | yes | yes |
//!
//! # Relationship to `ContentCodec`
//!
//! The existing [`ContentCodec`](crate::ContentCodec) trait handles raw byte ↔ text
//! transformations. These new traits add domain-aware operations (position mapping,
//! edit translation) that [`ContentCodec`](crate::ContentCodec) cannot express.
//! Both trait systems coexist — existing codec modules are unaffected.

use reovim_domain::Domain;
use reovim_driver_vfs::ByteEdit;

use crate::{CodecError, CodecMetadata};

/// Decode raw bytes into domain content.
///
/// Pure stateless transformation from bytes to the domain's content type.
/// Every codec must implement at least `Decode` for its domain.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` for use across async tasks.
pub trait Decode<D: Domain>: Send + Sync {
    /// Decode raw bytes into domain content.
    ///
    /// # Errors
    ///
    /// Returns [`CodecError`] if the bytes cannot be decoded.
    fn decode(&self, raw: &[u8]) -> Result<DecodeOutput<D>, CodecError>;
}

/// Encode domain content back into raw bytes.
///
/// Inverse of [`Decode`]. Codecs that support round-trip editing implement
/// this trait. Read-only codecs (hex dump, PDF viewer) omit it.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` for use across async tasks.
pub trait Encode<D: Domain>: Send + Sync {
    /// Encode domain content into raw bytes.
    ///
    /// The `metadata` parameter carries information from the original decode
    /// (e.g., BOM presence, line ending style) to ensure round-trip fidelity.
    ///
    /// # Errors
    ///
    /// Returns [`CodecError`] if the content cannot be encoded.
    fn encode(&self, content: &D::Content, metadata: &CodecMetadata)
        -> Result<Vec<u8>, CodecError>;
}

/// Stateful domain index for position mapping and edit translation.
///
/// Maintains a mapping between byte offsets (VFS layer) and domain positions
/// (provider layer). The index is built once from raw bytes, then updated
/// incrementally as byte-level edits occur.
///
/// # Lifecycle
///
/// 1. [`build`](Self::build) — Scan raw bytes, build initial index
/// 2. [`notify`](Self::notify) — Incrementally update on each byte edit
/// 3. [`to_bytes`](Self::to_bytes) / [`from_bytes`](Self::from_bytes) — Position mapping
/// 4. [`translate_edit`](Self::translate_edit) — Domain edit → byte edit
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync`. The `&mut self` on `build` and
/// `notify` means the caller must hold exclusive access during mutations.
pub trait Index<D: Domain>: Send + Sync {
    /// Build the index from raw bytes.
    ///
    /// Called once when a buffer is first opened or when the codec is switched.
    /// Replaces any existing index state.
    fn build(&mut self, raw: &[u8]);

    /// Incrementally update the index after a byte-level edit.
    ///
    /// Called after each `ByteEdit` is applied to the VFS storage.
    /// The implementation should update its internal state to reflect
    /// the change without a full rebuild.
    fn notify(&mut self, edit: &ByteEdit);

    /// Map a domain position to a byte offset.
    ///
    /// Returns `None` if the position is out of bounds.
    fn to_bytes(&self, pos: &D::Position) -> Option<usize>;

    /// Map a byte offset to a domain position.
    ///
    /// Returns `None` if the offset is out of bounds or falls in the
    /// middle of a multi-byte sequence.
    fn offset_to_position(&self, offset: usize) -> Option<D::Position>;

    /// Translate a domain-level edit into a byte-level edit.
    ///
    /// Converts a semantic edit (e.g., "insert text at line 5, column 3")
    /// into a byte edit (e.g., "insert these bytes at offset 142").
    ///
    /// Returns `None` if the edit cannot be translated (e.g., position
    /// out of bounds).
    fn translate_edit(&self, edit: &D::Edit) -> Option<ByteEdit>;
}

/// Result of decoding raw bytes into domain content.
///
/// Generic over [`Domain`], carrying the domain's content type along with
/// metadata for round-trip encoding and flags indicating decode quality.
#[derive(Debug, Clone)]
pub struct DecodeOutput<D: Domain> {
    /// Decoded domain content.
    pub content: D::Content,

    /// Metadata for round-trip encoding.
    pub metadata: CodecMetadata,

    /// Whether the decode lost information.
    ///
    /// When `true`, the decoded content is a lossy representation
    /// (e.g., replacement characters for unmappable bytes). The buffer
    /// should be treated as read-only.
    pub lossy: bool,

    /// Whether the buffer should be read-only.
    ///
    /// Set to `true` for lossy decodes or one-way codecs.
    pub readonly: bool,

    /// Whether the output was truncated due to input size limits.
    pub truncated: bool,
}

#[cfg(test)]
#[path = "domain_codec_tests.rs"]
mod tests;
