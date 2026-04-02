//! Text content provider for reovim.
//!
//! This crate provides text-specific buffer types, algorithms, and navigation
//! that operate on decoded text from any codec (UTF-8, Shift-JIS, xxd, etc.).
//!
//! # Architecture
//!
//! ```text
//! Provider (here):  VirtualBuffer, MotionEngine, TextGeometry
//! Codec:            ContentCodec (bytes <-> text, with domain index)
//! VFS:              ByteBuffer, PieceTable (byte-only), StreamBuffer
//! Kernel:           BufferId, StorageOps, BufferMeta, EventBus
//! ```
//!
//! # Migration Status
//!
//! This crate is being populated incrementally as part of #740
//! (kernel buffer extraction). Types move here from `reovim-kernel`
//! with backward compatibility maintained via import updates.

// Large file offsets are u64 but Rust indexing uses usize.
// Truncation is lossless on our 64-bit-only target.
#[allow(clippy::cast_possible_truncation)]
mod virtual_buffer;

pub use virtual_buffer::{HeapMapping, VirtualBuffer};
