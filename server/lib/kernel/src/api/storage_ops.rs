//! Byte-level storage traits for the kernel fd table.
//!
//! These traits define the kernel's contract for buffer storage. The kernel
//! treats all buffers as byte containers — content interpretation (text, audio,
//! video) happens in codecs and providers above the kernel layer.
//!
//! # Architecture
//!
//! ```text
//! Kernel:   StorageOps + BufferMeta (byte I/O + identity)
//! VFS:      ByteBuffer, StreamBuffer (concrete storage)
//! Codec:    ContentCodec (bytes ↔ domain)
//! Provider: TextProvider, AudioProvider (domain navigation)
//! ```
//!
//! # Static vs Stream
//!
//! Both static (file-backed) and stream (pipe/network) buffers implement
//! `StorageOps`. Capabilities advertise what's supported:
//! - Static: SEEKABLE + EDITABLE + FINITE
//! - Stream: APPENDABLE (possibly without SEEKABLE or EDITABLE)

use std::fmt;

use bitflags::bitflags;

use crate::mm::BufferId;

/// Error type for storage operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageError {
    /// Byte offset out of range.
    OffsetOutOfRange { offset: usize, len: usize },
    /// Operation not supported by this storage type.
    NotSupported(&'static str),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OffsetOutOfRange { offset, len } => {
                write!(f, "offset {offset} out of range (len {len})")
            }
            Self::NotSupported(op) => write!(f, "operation not supported: {op}"),
        }
    }
}

impl std::error::Error for StorageError {}

bitflags! {
    /// Describes what operations a storage backend supports.
    ///
    /// The kernel fd table uses these flags to determine valid operations
    /// without knowing the concrete storage type.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct StorageCapabilities: u32 {
        /// Random-access reads at any byte offset.
        const SEEKABLE    = 1 << 0;
        /// Insert/delete bytes at arbitrary offsets.
        const EDITABLE    = 1 << 1;
        /// Append bytes to the end (streams).
        const APPENDABLE  = 1 << 2;
        /// Known, fixed total size.
        const FINITE      = 1 << 3;
        /// Backed by a persistent file on disk.
        const PERSISTENT  = 1 << 4;
    }
}

impl StorageCapabilities {
    /// Standard capabilities for a heap-backed buffer (small files).
    pub const HEAP: Self = Self::SEEKABLE
        .union(Self::EDITABLE)
        .union(Self::FINITE);

    /// Standard capabilities for a file-backed mmap buffer.
    pub const MMAP: Self = Self::SEEKABLE
        .union(Self::EDITABLE)
        .union(Self::FINITE)
        .union(Self::PERSISTENT);

    /// Standard capabilities for a stream buffer.
    pub const STREAM: Self = Self::APPENDABLE;
}

/// Byte-level storage operations.
///
/// All buffer types implement this trait. The kernel `BufferManager` stores
/// `dyn StorageOps` in the fd table. Codecs and providers layer on top.
///
/// # Object Safety
///
/// This trait is object-safe: all methods use `&self`/`&mut self`, return
/// owned types, and have no generic parameters.
pub trait StorageOps: Send + Sync + 'static {
    /// Total byte length of stored content.
    fn byte_len(&self) -> usize;

    /// Whether the storage has zero bytes.
    fn is_empty(&self) -> bool {
        self.byte_len() == 0
    }

    /// Read bytes starting at `offset` into `buf`.
    ///
    /// Returns the number of bytes actually read (may be less than `buf.len()`
    /// if offset + `buf.len()` exceeds storage length).
    fn read_bytes(&self, offset: usize, buf: &mut [u8]) -> usize;

    /// Capability flags for this storage type.
    fn capabilities(&self) -> StorageCapabilities;

    // === Static mode (SEEKABLE + EDITABLE) ===

    /// Insert raw bytes at `offset`.
    ///
    /// # Errors
    ///
    /// Returns `NotSupported` if storage lacks EDITABLE capability.
    /// Returns `OffsetOutOfRange` if offset > `byte_len`.
    fn insert_bytes(&mut self, offset: usize, data: &[u8]) -> Result<(), StorageError>;

    /// Delete `len` bytes starting at `offset`, returning deleted bytes.
    ///
    /// # Errors
    ///
    /// Returns `NotSupported` if storage lacks EDITABLE capability.
    fn delete_bytes(&mut self, offset: usize, len: usize) -> Result<Vec<u8>, StorageError>;

    // === Stream mode (APPENDABLE) ===

    /// Append bytes to the end of storage.
    ///
    /// # Errors
    ///
    /// Returns `NotSupported` if storage lacks APPENDABLE capability.
    fn append_bytes(&mut self, data: &[u8]) -> Result<(), StorageError>;

    // === Bulk access ===

    /// Read a chunk of bytes starting at `offset`, up to `max_len` bytes.
    ///
    /// Returns owned bytes to avoid lifetime issues with ring-buffer-backed
    /// stream storage where underlying data can rotate.
    fn read_chunk(&self, offset: usize, max_len: usize) -> Vec<u8>;
}

/// Buffer identity and metadata.
///
/// All buffers in the fd table have identity (id, path) and state (modified).
/// This is separate from `StorageOps` to allow independent evolution.
pub trait BufferMeta: Send + Sync + 'static {
    /// Unique buffer identifier (inode).
    fn id(&self) -> BufferId;

    /// File path associated with this buffer.
    fn file_path(&self) -> Option<&str>;

    /// Set the file path for this buffer.
    fn set_file_path(&mut self, path: Option<String>);

    /// Whether the buffer has unsaved modifications.
    fn is_modified(&self) -> bool;

    /// Mark the buffer as modified or unmodified.
    fn set_modified(&mut self, modified: bool);
}

#[cfg(test)]
#[path = "tests/storage_ops.rs"]
mod tests;
