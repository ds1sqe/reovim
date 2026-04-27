//! Host-side byte-container trait and `CodecAttachmentId` newtype.

use {
    crate::error::BufferError,
    reovim_content_codec::ByteNotifiable,
    reovim_kernel::api::v1::{BufferId, ByteEdit},
    std::{num::NonZeroU32, ops::Range},
};

/// Identifies a codec slot in a buffer's attachment table.
///
/// Allocated monotonically by [`Buffer::attach_codec`]; the first slot
/// in any buffer is `1`. The monotonic order determines the fan-out
/// sequence inside `apply_edit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CodecAttachmentId(pub NonZeroU32);

/// Byte-container with synchronous edit fan-out and codec attachments.
///
/// Every method takes `&self`. Interior mutability (e.g. `Mutex`,
/// `RwLock`) lives inside the driver implementation; `Buffer` is
/// `Send + Sync`.
///
/// The async edit-subscription surface lives on the companion trait
/// [`crate::BufferSubscribable`] and is intentionally separated to
/// avoid coupling the core `Buffer` trait object to tokio.
pub trait Buffer: Send + Sync + 'static {
    /// The stable identifier for this buffer instance.
    fn id(&self) -> BufferId;

    /// The file path associated with this buffer, if any.
    fn file_path(&self) -> Option<String>;

    /// Replace the file path associated with this buffer.
    ///
    /// Passing `None` dissociates any existing path.
    ///
    /// # Errors
    ///
    /// Returns [`BufferError::Io`] if the driver-side path validation
    /// or storage fails (e.g. an unrepresentable path on the target
    /// platform).
    fn set_file_path(&self, path: Option<String>) -> Result<(), BufferError>;

    /// Returns `true` if the buffer has unsaved modifications.
    fn is_modified(&self) -> bool;

    /// Total number of bytes in the buffer.
    fn size(&self) -> usize;

    /// Read a contiguous slice of the buffer's byte content.
    ///
    /// Returns `Vec<u8>` (not `&[u8]`) because the rope-based storage
    /// is non-contiguous; no contiguous borrowed slice of the full byte
    /// stream is available. The allocation cost is equivalent to the
    /// existing `BufferContentProvider::content_bytes` call.
    ///
    /// # Errors
    ///
    /// Returns [`BufferError::InvalidEdit`] if `range` is out of bounds
    /// or inverted.
    fn read_bytes(&self, range: Range<usize>) -> Result<Vec<u8>, BufferError>;

    /// Apply a single byte-level edit and fan out to all attached codecs.
    ///
    /// Fan-out order: monotonically increasing `CodecAttachmentId`.
    /// Each codec's `notify` runs synchronously inside a `catch_unwind`
    /// guard; a panicking slot is marked poisoned but not detached.
    ///
    /// # Errors
    ///
    /// Returns [`BufferError::InvalidEdit`] if `edit` references a
    /// byte range outside the current buffer extent.
    fn apply_edit(&self, edit: ByteEdit) -> Result<(), BufferError>;

    /// Write the full buffer contents to `writer`.
    ///
    /// # Errors
    ///
    /// Propagates any [`std::io::Error`] surfaced by `writer`.
    fn write_to(&self, writer: &mut dyn std::io::Write) -> std::io::Result<()>;

    // ── Multi-attachment: codec slot table ──────────────────────────────

    /// Attach a codec to this buffer and return the new slot id.
    ///
    /// `codec.build(current_bytes)` runs synchronously BEFORE the slot
    /// becomes visible to subsequent `apply_edit` fan-out, and BEFORE
    /// any internal slot lock is taken — so a long-running build does
    /// not block other buffer operations. After build returns, the slot
    /// is inserted under the slot-table critical section so the
    /// duplicate-name check + insert is atomic. Returns
    /// `Err(BufferError::DuplicateCodecName)` if a live slot already
    /// holds the same name.
    ///
    /// # Errors
    ///
    /// Returns [`BufferError::DuplicateCodecName`] if `name` is
    /// already in use by a live slot on this buffer.
    fn attach_codec(
        &self,
        name: &str,
        codec: Box<dyn ByteNotifiable>,
    ) -> Result<CodecAttachmentId, BufferError>;

    /// Detach the codec at `id` and return the original allocation.
    ///
    /// The returned `Box<dyn ByteNotifiable>` is the same allocation
    /// the host passed to `attach_codec`; the driver must not move it
    /// across allocators.
    ///
    /// # Errors
    ///
    /// Returns [`BufferError::NotFound`] if no slot exists for `id`.
    fn detach_codec(&self, id: CodecAttachmentId) -> Result<Box<dyn ByteNotifiable>, BufferError>;

    /// List all live codec slots as `(id, name)` pairs.
    ///
    /// Ordered by ascending `CodecAttachmentId` to match fan-out order.
    fn list_codecs(&self) -> Vec<(CodecAttachmentId, String)>;
}

#[cfg(test)]
#[path = "buffer_tests.rs"]
mod tests;
