//! Pure-function file-open helper for the content-codec subsystem.
//!
//! Drives raw bytes through the classifier and factory stores to a
//! decoded [`String`] plus an optional [`ContentType`] tag. The helper
//! is the shared seam used by `:e`, the file picker, and the explorer
//! so every file-open entry point applies the same classification and
//! UTF-8 fallback policy. The legacy `decode_file_content` in
//! `ext/server/modules/commands` is the historical home of this logic;
//! Phase 4 of the codec hot-attach plan refactors it to call this
//! helper.

use reovim_content_codec::{ContentClassifierStore, ContentCodecFactoryStore, ContentType};

/// Maximum byte length accepted by [`decode_file_bytes`].
///
/// Files larger than this are routed to the streaming `:e` path. The
/// in-memory pipeline is not the right tool for very large files —
/// callers must size-check upstream and redirect to the streaming path
/// before invoking the helper.
pub const MAX_FILE_SIZE: usize = 64 * 1024 * 1024;

/// Errors returned by [`decode_file_bytes`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileOpenError {
    /// Bytes exceeded [`MAX_FILE_SIZE`]. Callers should redirect to the
    /// streaming `:e` path.
    TooLarge { len: usize, limit: usize },

    /// No codec produced valid output and the bytes are not valid
    /// UTF-8. The offset is the first invalid byte position so callers
    /// can highlight the failure point.
    NotUtf8 { offset: usize },
}

impl std::fmt::Display for FileOpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge { len, limit } => {
                write!(f, "file is {len} bytes, exceeds limit of {limit}")
            }
            Self::NotUtf8 { offset } => {
                write!(f, "file is not valid UTF-8 (invalid byte at offset {offset})")
            }
        }
    }
}

impl std::error::Error for FileOpenError {}

/// Decode raw bytes through the codec pipeline.
///
/// The helper consults `classifier_store` first; on a hit it asks
/// `factory_store` for a codec and runs `decode()`. Any failure along
/// that path — classifier returning `None`, factory not knowing the
/// type, or `decode()` erroring — drops to a UTF-8 fallback. Callers
/// only see [`FileOpenError`] when the file is too large or when the
/// fallback itself fails because the bytes are not valid UTF-8.
///
/// `filename` is forwarded to `classify()` so extension-aware
/// classifiers (rlib, csv, binary) can use it. Pass `""` when no path
/// is available.
///
/// # Returns
///
/// - `Ok((text, Some(ct)))` — codec produced `text` from bytes
///   recognised as `ct`.
/// - `Ok((text, None))` — fallback path: bytes were valid UTF-8 and
///   `text` is the literal byte contents (either no classifier
///   matched, no factory was registered for the matched type, or the
///   codec's `decode()` returned `Err`).
///
/// # Errors
///
/// - [`FileOpenError::TooLarge`] when `bytes.len() > MAX_FILE_SIZE`.
/// - [`FileOpenError::NotUtf8`] when the fallback UTF-8 conversion
///   fails.
pub fn decode_file_bytes(
    bytes: &[u8],
    filename: &str,
    classifier_store: &ContentClassifierStore,
    factory_store: &ContentCodecFactoryStore,
) -> Result<(String, Option<ContentType>), FileOpenError> {
    if bytes.len() > MAX_FILE_SIZE {
        return Err(FileOpenError::TooLarge {
            len: bytes.len(),
            limit: MAX_FILE_SIZE,
        });
    }

    if let Some(content_type) = classifier_store.classify(bytes, filename)
        && let Some(codec) = factory_store.find(&content_type)
        && let Ok(decoded) = codec.decode(bytes)
    {
        return Ok((decoded.content, Some(content_type)));
    }

    String::from_utf8(bytes.to_vec())
        .map(|s| (s, None))
        .map_err(|err| FileOpenError::NotUtf8 {
            offset: err.utf8_error().valid_up_to(),
        })
}
