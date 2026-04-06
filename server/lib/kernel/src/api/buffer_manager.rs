//! Buffer manager trait for kernel-driver communication.
//!
//! Defines the interface for buffer storage and retrieval. The kernel provides
//! pure storage mechanisms; drivers handle I/O operations (loading, saving).
//!
//! The manager stores `Arc<RwLock<dyn BufferOps>>`. `BufferOps` extends
//! `StorageOps + BufferMeta`, so byte-level I/O, identity, and text operations
//! are all accessible through the same trait object.
//!
//! # Migration Plan (#740)
//!
//! The target architecture stores `dyn KernelBuffer` (byte-only) instead of
//! `dyn BufferOps` (text-specific). The migration path:
//! 1. Add `TextBufferRegistry` service at session layer
//! 2. Migrate callers to use text registry for text access
//! 3. Change `BufferManager` to `dyn KernelBuffer`
//! 4. Move `BufferOps` definition out of kernel

use std::{fmt, sync::Arc};

use reovim_arch::sync::RwLock;

use crate::{api::BufferOps, mm::BufferId};

// ============================================================================
// Error Types
// ============================================================================

/// Error type for buffer manager operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BufferError {
    /// Buffer with the given ID was not found.
    NotFound(BufferId),
    /// Buffer with the given ID already exists.
    AlreadyExists(BufferId),
    /// Invalid operation attempted.
    InvalidOperation(&'static str),
}

impl fmt::Display for BufferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "buffer not found: {id:?}"),
            Self::AlreadyExists(id) => write!(f, "buffer already exists: {id:?}"),
            Self::InvalidOperation(msg) => write!(f, "invalid operation: {msg}"),
        }
    }
}

impl std::error::Error for BufferError {}

// ============================================================================
// BufferManager Trait
// ============================================================================

/// Unified buffer manager interface.
///
/// Stores all buffer types as `Arc<RwLock<dyn BufferOps>>`. `BufferOps`
/// extends `StorageOps + BufferMeta`, providing byte I/O, identity, and
/// text operations through a single trait object.
///
/// # Design Philosophy
///
/// - **No I/O operations**: No `open()/save()` methods — VFS driver handles these
/// - **No focus tracking**: Active buffer tracking is a runtime/window concern
/// - **Thread-safe**: Uses `Arc<RwLock<dyn BufferOps>>` for concurrent access
/// - **Kernel purity**: Pure mechanisms only, no external dependencies
/// - **Type-agnostic**: Callers use `BufferOps` trait, not concrete types
///
/// # Register Pattern
///
/// Callers construct the buffer and wrap it before registering:
/// ```ignore
/// let buf = Buffer::from_string("hello");
/// let arc: Arc<RwLock<dyn BufferOps>> = Arc::new(RwLock::new(buf));
/// let id = manager.register(arc);
/// ```
pub trait BufferManager: Send + Sync {
    /// Get buffer by ID.
    ///
    /// Returns `None` if the buffer does not exist.
    fn get(&self, id: BufferId) -> Option<Arc<RwLock<dyn BufferOps>>>;

    /// Register a buffer.
    ///
    /// The buffer's own ID (from `BufferMeta::id()`) is used as the key.
    /// Returns the buffer's ID for convenience.
    fn register(&self, buffer: Arc<RwLock<dyn BufferOps>>) -> BufferId;

    /// Unregister buffer, returning the arc if it existed.
    fn unregister(&self, id: BufferId) -> Option<Arc<RwLock<dyn BufferOps>>>;

    /// List all buffer IDs. Order is not guaranteed; callers that need
    /// deterministic ordering must sort the result.
    fn list(&self) -> Vec<BufferId>;

    /// Get count of buffers.
    fn count(&self) -> usize;
}
