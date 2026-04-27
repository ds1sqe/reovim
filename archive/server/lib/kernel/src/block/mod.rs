//! Block I/O subsystem.
//!
//! Linux equivalent: `block/`
//!
//! This subsystem defines the byte-level I/O contract for all buffer types.
//! It provides the mutation atom ([`ByteEdit`]), the universal undo log
//! ([`ByteUndoLog`]), and the byte-level storage trait ([`StorageOps`]).
//!
//! Domain-specific indexing (line boundaries, character counts, sample
//! offsets) is handled by codecs and providers above this layer.
//!
//! # Components
//!
//! - [`ByteEdit`] — atomic byte-level mutation (insert/delete/replace)
//! - [`ByteUndoLog`] — universal append-only undo log over `ByteEdit`s
//! - [`StorageOps`] — byte I/O trait for all buffer types
//! - [`StorageCapabilities`] — bitflags for what a storage backend supports
//! - [`BufferMeta`] — buffer identity and metadata
//! - [`KernelBuffer`] — combined `StorageOps` + `BufferMeta`

mod buffer_meta;
mod byte_edit;
mod byte_undo_log;
mod storage_ops;

pub use {
    buffer_meta::{BufferMeta, KernelBuffer},
    byte_edit::ByteEdit,
    byte_undo_log::{ByteUndoEntry, ByteUndoLog},
    storage_ops::{StorageCapabilities, StorageError, StorageOps},
};
