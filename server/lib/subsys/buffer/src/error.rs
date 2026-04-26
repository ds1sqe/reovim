//! Structured error types for the buffer subsys.

use {crate::buffer::CodecAttachmentId, reovim_kernel::api::v1::BufferId};

/// Errors raised by buffer operations and by drivers that implement
/// the [`crate::Buffer`] and [`crate::BufferDriver`] contracts.
#[derive(Debug, Clone, thiserror::Error)]
pub enum BufferError {
    /// No buffer exists with the given id.
    #[error("buffer not found: {0:?}")]
    NotFound(BufferId),

    /// The supplied edit is structurally invalid (out-of-bounds offset,
    /// mismatched old-bytes, etc.).
    #[error("invalid edit: {0}")]
    InvalidEdit(String),

    /// The codec at the given slot panicked during a previous notify;
    /// the slot is poisoned and must be detached before use.
    #[error("codec attachment {0:?} poisoned: {1}")]
    CodecPoisoned(CodecAttachmentId, String),

    /// A second `attach_codec` call supplied a name already held by a
    /// live slot.
    #[error("attached codec name conflict: {0}")]
    DuplicateCodecName(String),

    /// An I/O error occurred (e.g. inside `write_to` or `open_buffer`).
    #[error("io: {0}")]
    Io(String),

    /// The driver encountered a lifecycle error (construction, open,
    /// close).
    #[error("driver lifecycle: {0}")]
    Driver(String),
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
