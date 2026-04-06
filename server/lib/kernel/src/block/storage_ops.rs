//! Byte-level storage traits for the block I/O subsystem.
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
    pub const HEAP: Self = Self::SEEKABLE.union(Self::EDITABLE).union(Self::FINITE);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::buffer_meta::BufferMeta;
    use crate::mm::BufferId;

    /// Mock storage backed by `Vec<u8>` for testing.
    struct MockStorage {
        id: BufferId,
        data: Vec<u8>,
        file_path: Option<String>,
        modified: bool,
    }

    impl MockStorage {
        fn new(id: BufferId, data: &[u8]) -> Self {
            Self {
                id,
                data: data.to_vec(),
                file_path: None,
                modified: false,
            }
        }
    }

    impl StorageOps for MockStorage {
        fn byte_len(&self) -> usize {
            self.data.len()
        }

        fn read_bytes(&self, offset: usize, buf: &mut [u8]) -> usize {
            if offset >= self.data.len() {
                return 0;
            }
            let available = self.data.len() - offset;
            let to_read = buf.len().min(available);
            buf[..to_read].copy_from_slice(&self.data[offset..offset + to_read]);
            to_read
        }

        fn capabilities(&self) -> StorageCapabilities {
            StorageCapabilities::HEAP
        }

        fn insert_bytes(&mut self, offset: usize, data: &[u8]) -> Result<(), StorageError> {
            if offset > self.data.len() {
                return Err(StorageError::OffsetOutOfRange {
                    offset,
                    len: self.data.len(),
                });
            }
            self.data.splice(offset..offset, data.iter().copied());
            Ok(())
        }

        fn delete_bytes(&mut self, offset: usize, len: usize) -> Result<Vec<u8>, StorageError> {
            if offset + len > self.data.len() {
                return Err(StorageError::OffsetOutOfRange {
                    offset,
                    len: self.data.len(),
                });
            }
            let deleted: Vec<u8> = self.data[offset..offset + len].to_vec();
            self.data.drain(offset..offset + len);
            Ok(deleted)
        }

        fn append_bytes(&mut self, data: &[u8]) -> Result<(), StorageError> {
            self.data.extend_from_slice(data);
            Ok(())
        }

        fn read_chunk(&self, offset: usize, max_len: usize) -> Vec<u8> {
            if offset >= self.data.len() {
                return Vec::new();
            }
            let end = (offset + max_len).min(self.data.len());
            self.data[offset..end].to_vec()
        }
    }

    impl BufferMeta for MockStorage {
        fn id(&self) -> BufferId {
            self.id
        }

        fn file_path(&self) -> Option<&str> {
            self.file_path.as_deref()
        }

        fn set_file_path(&mut self, path: Option<String>) {
            self.file_path = path;
        }

        fn is_modified(&self) -> bool {
            self.modified
        }

        fn set_modified(&mut self, modified: bool) {
            self.modified = modified;
        }
    }

    // === StorageOps tests ===

    #[test]
    fn byte_len_empty() {
        let storage = MockStorage::new(BufferId::new(), &[]);
        assert_eq!(storage.byte_len(), 0);
        assert!(storage.is_empty());
    }

    #[test]
    fn byte_len_nonempty() {
        let storage = MockStorage::new(BufferId::new(), b"hello");
        assert_eq!(storage.byte_len(), 5);
        assert!(!storage.is_empty());
    }

    #[test]
    fn read_bytes_full() {
        let storage = MockStorage::new(BufferId::new(), b"hello world");
        let mut buf = [0u8; 5];
        let n = storage.read_bytes(0, &mut buf);
        assert_eq!(n, 5);
        assert_eq!(&buf, b"hello");
    }

    #[test]
    fn read_bytes_partial() {
        let storage = MockStorage::new(BufferId::new(), b"hi");
        let mut buf = [0u8; 10];
        let n = storage.read_bytes(0, &mut buf);
        assert_eq!(n, 2);
        assert_eq!(&buf[..2], b"hi");
    }

    #[test]
    fn read_bytes_offset() {
        let storage = MockStorage::new(BufferId::new(), b"hello");
        let mut buf = [0u8; 3];
        let n = storage.read_bytes(2, &mut buf);
        assert_eq!(n, 3);
        assert_eq!(&buf, b"llo");
    }

    #[test]
    fn read_bytes_past_end() {
        let storage = MockStorage::new(BufferId::new(), b"hi");
        let mut buf = [0u8; 5];
        let n = storage.read_bytes(10, &mut buf);
        assert_eq!(n, 0);
    }

    #[test]
    fn insert_bytes_at_start() {
        let mut storage = MockStorage::new(BufferId::new(), b"world");
        storage.insert_bytes(0, b"hello ").unwrap();
        assert_eq!(&storage.data, b"hello world");
    }

    #[test]
    fn insert_bytes_at_end() {
        let mut storage = MockStorage::new(BufferId::new(), b"hello");
        storage.insert_bytes(5, b" world").unwrap();
        assert_eq!(&storage.data, b"hello world");
    }

    #[test]
    fn insert_bytes_in_middle() {
        let mut storage = MockStorage::new(BufferId::new(), b"helo");
        storage.insert_bytes(2, b"ll").unwrap();
        assert_eq!(&storage.data, b"helllo");
    }

    #[test]
    fn insert_bytes_out_of_range() {
        let mut storage = MockStorage::new(BufferId::new(), b"hi");
        let result = storage.insert_bytes(10, b"x");
        assert!(result.is_err());
    }

    #[test]
    fn delete_bytes_from_start() {
        let mut storage = MockStorage::new(BufferId::new(), b"hello world");
        let deleted = storage.delete_bytes(0, 6).unwrap();
        assert_eq!(&deleted, b"hello ");
        assert_eq!(&storage.data, b"world");
    }

    #[test]
    fn delete_bytes_from_end() {
        let mut storage = MockStorage::new(BufferId::new(), b"hello world");
        let deleted = storage.delete_bytes(5, 6).unwrap();
        assert_eq!(&deleted, b" world");
        assert_eq!(&storage.data, b"hello");
    }

    #[test]
    fn delete_bytes_out_of_range() {
        let mut storage = MockStorage::new(BufferId::new(), b"hi");
        let result = storage.delete_bytes(0, 10);
        assert!(result.is_err());
    }

    #[test]
    fn append_bytes() {
        let mut storage = MockStorage::new(BufferId::new(), b"hello");
        storage.append_bytes(b" world").unwrap();
        assert_eq!(&storage.data, b"hello world");
    }

    #[test]
    fn read_chunk_full() {
        let storage = MockStorage::new(BufferId::new(), b"hello world");
        let chunk = storage.read_chunk(0, 100);
        assert_eq!(&chunk, b"hello world");
    }

    #[test]
    fn read_chunk_partial() {
        let storage = MockStorage::new(BufferId::new(), b"hello world");
        let chunk = storage.read_chunk(6, 5);
        assert_eq!(&chunk, b"world");
    }

    #[test]
    fn read_chunk_past_end() {
        let storage = MockStorage::new(BufferId::new(), b"hi");
        let chunk = storage.read_chunk(10, 5);
        assert!(chunk.is_empty());
    }

    // === StorageCapabilities tests ===

    #[test]
    fn heap_capabilities() {
        let caps = StorageCapabilities::HEAP;
        assert!(caps.contains(StorageCapabilities::SEEKABLE));
        assert!(caps.contains(StorageCapabilities::EDITABLE));
        assert!(caps.contains(StorageCapabilities::FINITE));
        assert!(!caps.contains(StorageCapabilities::PERSISTENT));
        assert!(!caps.contains(StorageCapabilities::APPENDABLE));
    }

    #[test]
    fn mmap_capabilities() {
        let caps = StorageCapabilities::MMAP;
        assert!(caps.contains(StorageCapabilities::SEEKABLE));
        assert!(caps.contains(StorageCapabilities::EDITABLE));
        assert!(caps.contains(StorageCapabilities::FINITE));
        assert!(caps.contains(StorageCapabilities::PERSISTENT));
    }

    #[test]
    fn stream_capabilities() {
        let caps = StorageCapabilities::STREAM;
        assert!(caps.contains(StorageCapabilities::APPENDABLE));
        assert!(!caps.contains(StorageCapabilities::SEEKABLE));
        assert!(!caps.contains(StorageCapabilities::EDITABLE));
    }

    // === StorageError tests ===

    #[test]
    fn storage_error_display() {
        let err = StorageError::OffsetOutOfRange { offset: 10, len: 5 };
        assert_eq!(err.to_string(), "offset 10 out of range (len 5)");

        let err = StorageError::NotSupported("insert");
        assert_eq!(err.to_string(), "operation not supported: insert");
    }

    // === BufferMeta tests ===

    #[test]
    fn buffer_meta_id() {
        let id = BufferId::new();
        let storage = MockStorage::new(id, b"");
        assert_eq!(storage.id(), id);
    }

    #[test]
    fn buffer_meta_file_path() {
        let mut storage = MockStorage::new(BufferId::new(), b"");
        assert!(storage.file_path().is_none());

        storage.set_file_path(Some("foo.rs".to_string()));
        assert_eq!(storage.file_path(), Some("foo.rs"));

        storage.set_file_path(None);
        assert!(storage.file_path().is_none());
    }

    #[test]
    fn buffer_meta_modified() {
        let mut storage = MockStorage::new(BufferId::new(), b"");
        assert!(!storage.is_modified());

        storage.set_modified(true);
        assert!(storage.is_modified());

        storage.set_modified(false);
        assert!(!storage.is_modified());
    }

    // === Trait object safety ===

    #[test]
    fn storage_ops_is_object_safe() {
        let storage: Box<dyn StorageOps> = Box::new(MockStorage::new(BufferId::new(), b"test"));
        assert_eq!(storage.byte_len(), 4);
    }

    #[test]
    fn buffer_meta_is_object_safe() {
        let meta: Box<dyn BufferMeta> = Box::new(MockStorage::new(BufferId::new(), b""));
        assert!(!meta.is_modified());
    }
}
