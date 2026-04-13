//! Byte-level source abstractions for inode-style storage.
//!
//! This module provides a small abstraction used by upcoming inode-layer code to
//! read file bytes from either owned heap storage or mmap-backed storage.

use std::{borrow::Cow, ops::Range, sync::Arc};

use bitflags::bitflags;

use crate::{MappedFile, file_mapping::FileMapping};

bitflags! {
    /// Describes which operations a byte source supports.
    ///
    /// The bit layout intentionally mirrors the phase-2 contract requirements
    /// for migration-safe capability checks.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ByteSourceCapabilities: u32 {
        /// Supports random-access reads.
        const RANDOM_ACCESS = 1 << 0;
        /// Backing storage is mutable in place.
        const WRITABLE = 1 << 1;
        /// Bytes may be appended/arrive over time.
        const STREAMING = 1 << 2;
        /// Backed by a `memmap2::Mmap` owned by `MappedFile`.
        ///
        /// This includes both real file-backed mappings and the anonymous mmap
        /// fallback used by the default `VfsDriver::mmap_read()` implementation.
        const MMAP_BACKED = 1 << 3;
    }
}

/// Byte source contract used by inode-aware codecs and buffers.
pub trait ByteSource: Send + Sync + std::any::Any {
    /// Total number of bytes in the source.
    fn len(&self) -> u64;

    /// Whether the source is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Type-erased upcast for test/runtime downcasts.
    fn as_any(&self) -> &dyn std::any::Any;

    /// Zero-copy contiguous access if available.
    fn as_slice(&self) -> Option<&[u8]>;

    /// Read the requested half-open byte range.
    ///
    /// Ranges outside bounds return an empty slice. Reversed ranges also return
    /// empty. Implementations must never panic on bad ranges.
    fn read(&self, range: Range<u64>) -> Cow<'_, [u8]>;

    /// Stream all bytes to `writer`.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if this source cannot expose contiguous bytes and does
    /// not provide a streaming implementation.
    fn write_to(&self, writer: &mut dyn std::io::Write) -> std::io::Result<u64> {
        match self.as_slice() {
            Some(slice) => {
                writer.write_all(slice)?;
                Ok(slice.len() as u64)
            }
            None => Err(std::io::Error::other(
                "ByteSource is not sliceable and has no streaming write_to impl",
            )),
        }
    }

    /// Advertise byte-level capabilities.
    fn capabilities(&self) -> ByteSourceCapabilities;
}

/// Heap-backed byte source.
pub struct HeapByteSource(Arc<Vec<u8>>);

impl HeapByteSource {
    /// Construct a heap-backed source from owned byte data.
    #[must_use]
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self(Arc::new(bytes.into()))
    }
}

impl ByteSource for HeapByteSource {
    fn len(&self) -> u64 {
        self.0.len() as u64
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_slice(&self) -> Option<&[u8]> {
        Some(self.0.as_slice())
    }

    fn read(&self, range: Range<u64>) -> Cow<'_, [u8]> {
        read_slice(&self.0, range)
    }

    fn capabilities(&self) -> ByteSourceCapabilities {
        ByteSourceCapabilities::RANDOM_ACCESS
    }
}

/// Mmap-backed byte source wrapped around `MappedFile`.
///
/// The constructor intentionally takes a concrete `MappedFile` to guarantee
/// `MMAP_BACKED` semantics by construction.
pub struct MappedByteSource(Arc<MappedFile>);

impl MappedByteSource {
    /// Construct a mmap-backed source from a mapped file.
    #[must_use]
    pub fn new(mapping: MappedFile) -> Self {
        Self(Arc::new(mapping))
    }
}

impl ByteSource for MappedByteSource {
    fn len(&self) -> u64 {
        self.0.len()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_slice(&self) -> Option<&[u8]> {
        Some(self.0.as_bytes())
    }

    fn read(&self, range: Range<u64>) -> Cow<'_, [u8]> {
        read_slice(self.0.as_bytes(), range)
    }

    fn capabilities(&self) -> ByteSourceCapabilities {
        ByteSourceCapabilities::RANDOM_ACCESS | ByteSourceCapabilities::MMAP_BACKED
    }
}

fn read_slice(data: &[u8], range: Range<u64>) -> Cow<'_, [u8]> {
    if range.start > range.end {
        return Cow::Borrowed(&[]);
    }

    let data_len = data.len();
    let data_len_u64 = data_len as u64;

    if range.start >= data_len_u64 {
        return Cow::Borrowed(&[]);
    }

    let start = usize::try_from(range.start).expect("range.start already bounded by slice len");

    let end = usize::try_from(range.end.min(data_len_u64))
        .expect("range.end already clamped to slice len");

    Cow::Borrowed(&data[start..end])
}
