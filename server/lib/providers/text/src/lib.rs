//! Text content provider for reovim.
//!
//! This crate provides text-specific buffer types, algorithms, and navigation
//! that operate on decoded text from any codec (UTF-8, Shift-JIS, xxd, etc.).
//!
//! # Architecture
//!
//! ```text
//! Provider (here):  Rope, VirtualBuffer, MotionEngine, TextGeometry
//! Codec:            ContentCodec (bytes ↔ text, with domain index)
//! VFS:              ByteBuffer, PieceTable (byte-only), StreamBuffer
//! Kernel:           BufferId, StorageOps, BufferMeta, EventBus
//! ```
//!
//! # Migration Status
//!
//! This crate is being populated incrementally as part of #740
//! (kernel buffer extraction). Types move here from `reovim-kernel`
//! with re-exports maintaining backward compatibility.
