//! File mapping trait for zero-copy access to original file bytes.
//!
//! `FileMapping` abstracts over mmap'd files and heap-backed test data.
//! The VFS layer defines the trait because it owns byte-level mapping and
//! memory-mapped file access.
//!
//! # Implementations
//!
//! - `MappedFile` (VFS driver) — real mmap, production use
//! - `HeapMapping` (providers/text) — `Vec<u8>` wrapper, test use

/// Trait for zero-copy access to original file bytes.
///
/// Implemented by `MappedFile` (VFS driver, real mmap) and by
/// `HeapMapping` (test helper, `Vec<u8>` wrapper).
pub trait FileMapping: Send + Sync + 'static {
    /// Raw bytes of the original file.
    fn as_bytes(&self) -> &[u8];

    /// File size in bytes.
    fn len(&self) -> u64;

    /// Whether the file is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Check if the underlying file has been modified since mapping.
    /// Returns `false` for heap-backed mappings (tests).
    fn is_stale(&self) -> bool {
        false
    }
}
