//! Buffer identity and metadata traits.
//!
//! Separate from [`StorageOps`](super::StorageOps) to allow independent
//! evolution. `BufferMeta` covers identity (id, path) and state (modified),
//! while `StorageOps` covers byte I/O operations.

use crate::mm::BufferId;

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

/// Combined byte-level buffer trait for the kernel fd table.
///
/// `KernelBuffer` unifies [`StorageOps`](super::StorageOps) (byte I/O) and
/// [`BufferMeta`] (identity) into a single object-safe trait. The kernel's
/// `BufferManager` stores buffers as `dyn KernelBuffer` — the kernel sees
/// only bytes and metadata, never text.
///
/// Text-specific operations (`line()`, `position_to_byte()`, etc.) are provided
/// by `BufferOps` in the provider layer, not in the kernel.
///
/// A blanket implementation is provided: any type implementing both `StorageOps`
/// and `BufferMeta` automatically implements `KernelBuffer`.
pub trait KernelBuffer: super::StorageOps + BufferMeta {}

impl<T: super::StorageOps + BufferMeta> KernelBuffer for T {}
