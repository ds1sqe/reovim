//! Memory-mapped file for zero-copy large file access.
//!
//! [`MappedFile`] wraps a `memmap2::Mmap` handle behind an `Arc` so it can
//! be shared across `VirtualBuffer` clones and snapshots.  It implements the
//! VFS-layer [`FileMapping`] trait, keeping the mmap dependency confined to
//! this driver.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

use crate::FileMapping;

// ─── MappedFile ─────────────────────────────────────────────────────────────

/// Inner state for a memory-mapped file.
struct MappedFileInner {
    /// The memory-mapped region.
    mmap: memmap2::Mmap,
    /// Path of the mapped file.
    path: PathBuf,
    /// File modification time at mapping time.
    mtime: SystemTime,
    /// File size at mapping time.
    size: u64,
}

/// A memory-mapped file that provides zero-copy access to its bytes.
///
/// The `Mmap` handle is kept alive as long as any `MappedFile` clone exists.
/// `as_bytes()` provides `&[u8]` access without copying.
///
/// # Safety
///
/// The file is opened read-only.  SIGBUS can occur if the file is truncated
/// externally while mapped.  Callers must check `is_stale()` before accessing
/// bytes on any write path.
#[derive(Clone)]
pub struct MappedFile {
    inner: Arc<MappedFileInner>,
}

impl MappedFile {
    /// Create a `MappedFile` from a `memmap2::Mmap`.
    #[must_use]
    pub fn new(mmap: memmap2::Mmap, path: &Path, mtime: SystemTime, size: u64) -> Self {
        Self {
            inner: Arc::new(MappedFileInner {
                mmap,
                path: path.to_path_buf(),
                mtime,
                size,
            }),
        }
    }

    /// Create a `MappedFile` from a byte slice (heap-copy fallback).
    ///
    /// Used by VFS implementations that don't support mmap (e.g., tests).
    ///
    /// # Panics
    ///
    /// Panics if the anonymous mmap allocation fails (out of memory).
    #[must_use]
    pub fn from_vec(bytes: &[u8], path: &Path) -> Self {
        let size = bytes.len() as u64;
        // Create an anonymous mmap-like mapping from bytes.
        // We use memmap2::MmapMut to create a writable mapping, copy bytes
        // into it, then freeze it to get a read-only Mmap.
        let mut mmap_mut = memmap2::MmapMut::map_anon(bytes.len()).expect("anonymous mmap failed");
        mmap_mut.copy_from_slice(bytes);
        let mmap = mmap_mut.make_read_only().expect("mmap freeze failed");

        Self {
            inner: Arc::new(MappedFileInner {
                mmap,
                path: path.to_path_buf(),
                mtime: SystemTime::now(),
                size,
            }),
        }
    }

    /// File path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.inner.path
    }

    /// File size at mapping time.
    #[must_use]
    pub fn size(&self) -> u64 {
        self.inner.size
    }
}

impl FileMapping for MappedFile {
    fn as_bytes(&self) -> &[u8] {
        &self.inner.mmap
    }

    fn len(&self) -> u64 {
        self.inner.size
    }

    fn is_stale(&self) -> bool {
        // Check if the file has been modified since mapping.
        let Ok(meta) = std::fs::metadata(&self.inner.path) else {
            return true; // File deleted or inaccessible
        };
        // If modified() is unsupported, treat mtime as unchanged and still
        // check size.  `is_ok_and` avoids an untestable Err branch on
        // platforms where modified() always succeeds (Linux, macOS, Windows).
        let mtime_changed = meta.modified().is_ok_and(|t| t != self.inner.mtime);
        mtime_changed || meta.len() != self.inner.size
    }
}

#[cfg(test)]
#[path = "mapped_file_tests.rs"]
mod tests;

impl std::fmt::Debug for MappedFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MappedFile")
            .field("path", &self.inner.path)
            .field("size", &self.inner.size)
            .finish()
    }
}
