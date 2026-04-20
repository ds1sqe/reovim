//! Content codec trait.
//!
//! Defines the [`ContentCodec`] trait for decoding raw bytes into text
//! and encoding text back into raw bytes. Codecs handle the translation
//! between on-disk byte representation and the internal UTF-8 text
//! representation used by the kernel's buffer.

use reovim_driver_annotation::Annotation;

use crate::{CodecError, CodecMetadata, DecodedEdit, TranslateEditError};

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
    ///
    /// SAFETY (`#740` Phase 3, B2): this flag is currently discarded by
    /// every consumer (`DecodeResult.readonly` is set but never honoured).
    /// Phase 3 deletes the field entirely once `ContentCodec::translate_edit`
    /// becomes the writability seam — codecs that cannot translate decoded
    /// edits back to byte edits return `None` and are read-only by
    /// construction. No Phase 0 runtime patch.
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

/// A named view that a codec can produce.
///
/// Codecs that support multiple views (e.g., structured metadata + hex dump)
/// return multiple `CodecView` entries from [`ContentCodec::views()`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodecView {
    /// Machine-readable view name (e.g., `"default"`, `"hex"`, `"meta"`).
    pub name: &'static str,
    /// Human-readable display label (e.g., `"Default"`, `"Hex Dump"`, `"Metadata"`).
    pub display: &'static str,
}

impl CodecView {
    /// The default view — every codec has at least this one.
    pub const DEFAULT: Self = Self {
        name: "default",
        display: "Default",
    };
}

/// Trait for encoding and decoding file content.
///
/// Implementations handle the translation between on-disk byte
/// representation and the internal UTF-8 representation. Each codec
/// handles a specific content type (e.g., UTF-8, hex dump, EUC-KR).
///
/// # Faithful vs Transforming
///
/// - **Faithful** codecs (e.g. UTF-8, hex, CSV, CJK, legacy encodings)
///   override [`translate_edit`](Self::translate_edit) to return
///   `Some(ByteEdit)`. Decoded edits flow back into `inode.bytes`
///   atomically so `:w` can persist the current inode bytes without
///   ever re-encoding.
/// - **Transforming** codecs (e.g. ELF, rlib, zip, PDF) inherit the
///   default `translate_edit -> None` and are read-only by
///   construction. `InodeTable::apply_edit` surfaces
///   `EditError::ReadOnly` when a user tries to edit through a
///   transforming mount.
///
/// # Encode path removed (#740 Plan 06 Phase 5 sub-commit 5d)
///
/// The previous `encode(&str, &CodecMetadata) -> Option<Result<Vec<u8>, _>>`
/// method has been deleted. `:w` now flushes canonical
/// `inode.bytes` via [`InodeTable::flush`](crate::InodeTable::flush),
/// which writes through `ByteSource::write_to`. Bytes are the source
/// of truth; there is no re-encode on save.
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

    /// Translate a decoded edit into a byte-level edit.
    ///
    /// This method is the canonical seam for edit capability checks.
    /// Plan 07 Phase 1 expands the return shape from the Plan 06
    /// `Option<ByteEdit>` to distinguish structural-codec rejection
    /// classes:
    ///
    /// - `Ok(Some(byte_edit))` — edit accepted, here is the byte diff.
    ///   `InodeTable::apply_edit` mutates `inode.bytes`, appends undo,
    ///   and marks peer mounts stale.
    /// - `Ok(None)` — edit accepted, no observable byte change (clean
    ///   no-op). `InodeTable::apply_edit` MUST early-return without
    ///   mutation, undo append, or peer stale-marking. See the Plan 07
    ///   `Ok(None)` semantic pin.
    /// - `Err(TranslateEditError::ReadOnly)` — codec cannot translate
    ///   any edit. Surfaces as [`EditError::ReadOnly`](crate::EditError::ReadOnly).
    /// - `Err(TranslateEditError::UnsupportedEdit { reason })` —
    ///   codec can edit some variants but not this one. Surfaces as
    ///   [`EditError::Unsupported`](crate::EditError::Unsupported).
    /// - `Err(TranslateEditError::ConstraintViolation { reason })` —
    ///   edit violates codec-domain constraints. Surfaces as
    ///   [`EditError::InvalidEdit`](crate::EditError::InvalidEdit).
    /// - `Err(TranslateEditError::MalformedPath { reason })` — tree
    ///   path did not resolve. Surfaces as
    ///   [`EditError::InvalidEdit`](crate::EditError::InvalidEdit).
    /// - `Err(TranslateEditError::Internal { reason })` — I/O, parse,
    ///   or infrastructure failure. Surfaces as
    ///   [`EditError::ApplyFailed`](crate::EditError::ApplyFailed).
    ///
    /// Default implementation returns `Err(TranslateEditError::ReadOnly)`
    /// so codecs that are not writable through this seam inherit
    /// read-only behavior by construction.
    ///
    /// # Errors
    ///
    /// See the `Err` variants above.
    fn translate_edit(
        &self,
        _bytes: &dyn reovim_subsys_vfs::ByteSource,
        _edit: &DecodedEdit,
    ) -> Result<Option<reovim_kernel::api::v1::ByteEdit>, TranslateEditError> {
        Err(TranslateEditError::ReadOnly)
    }

    /// Available views for this codec.
    ///
    /// Returns the list of named views this codec can produce.
    /// The default implementation returns a single "default" view.
    /// Codecs with multiple views (e.g., structured metadata + hex dump)
    /// override this to list all available views.
    fn views(&self) -> &[CodecView] {
        &[CodecView::DEFAULT]
    }

    /// Decode raw bytes using a specific named view.
    ///
    /// The `view` parameter must match one of the names returned by
    /// [`views()`](Self::views). The default implementation ignores
    /// the view name and delegates to [`decode()`](Self::decode).
    ///
    /// # Errors
    ///
    /// Returns [`CodecError`] if the bytes cannot be decoded or the
    /// view name is not recognized.
    fn decode_view(&self, raw: &[u8], _view: &str) -> Result<DecodeResult, CodecError> {
        self.decode(raw)
    }

    /// Decode a large file via streaming (reading only what's needed).
    ///
    /// For large binary files, this avoids loading the entire file into memory.
    /// Instead, the codec reads headers and metadata from the file handle.
    ///
    /// Returns `None` if this codec does not support streaming decode
    /// (the default). Callers fall back to full `decode()` in that case.
    ///
    /// # Arguments
    ///
    /// * `handle` - An open file handle for streaming reads
    /// * `file_size` - Total file size in bytes
    fn decode_streaming(
        &self,
        _handle: &mut dyn reovim_subsys_vfs::FileHandle,
        _file_size: u64,
    ) -> Option<Result<DecodeResult, CodecError>> {
        None
    }
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
