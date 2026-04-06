//! Buffer manager trait for kernel-driver communication.
//!
//! Defines the interface for buffer storage and retrieval. The kernel provides
//! pure storage mechanisms; drivers handle I/O operations (loading, saving).
//!
//! The manager stores `Arc<RwLock<dyn KernelBuffer>>`. `KernelBuffer` combines
//! `StorageOps + BufferMeta` — the kernel sees only bytes and identity.
//! Text-specific operations are provided by `BufferOps` in the session layer's
//! `TextBufferRegistry`, not through the kernel.

use std::{fmt, sync::Arc};

use reovim_arch::sync::RwLock;

use crate::{api::storage_ops::KernelBuffer, mm::BufferId};

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
/// Stores all buffer types as `Arc<RwLock<dyn KernelBuffer>>`. `KernelBuffer`
/// combines `StorageOps + BufferMeta`, providing byte I/O and identity through
/// a single trait object. Text-specific operations are provided by
/// `TextBufferRegistry` in the session layer, not through this manager.
///
/// # Design Philosophy
///
/// - **No I/O operations**: No `open()/save()` methods — VFS driver handles these
/// - **No focus tracking**: Active buffer tracking is a runtime/window concern
/// - **Thread-safe**: Uses `Arc<RwLock<dyn KernelBuffer>>` for concurrent access
/// - **Kernel purity**: Pure mechanisms only, no text dependencies
/// - **Byte-only**: The kernel sees bytes and metadata, never text
///
/// # Register Pattern
///
/// Callers construct the buffer and wrap it before registering:
/// ```ignore
/// let buf = Buffer::from_string("hello");
/// let arc: Arc<RwLock<dyn KernelBuffer>> = Arc::new(RwLock::new(buf));
/// let id = manager.register(arc);
/// ```
pub trait BufferManager: Send + Sync {
    /// Get buffer by ID.
    ///
    /// Returns `None` if the buffer does not exist.
    fn get(&self, id: BufferId) -> Option<Arc<RwLock<dyn KernelBuffer>>>;

    /// Register a buffer.
    ///
    /// The buffer's own ID (from `BufferMeta::id()`) is used as the key.
    /// Returns the buffer's ID for convenience.
    fn register(&self, buffer: Arc<RwLock<dyn KernelBuffer>>) -> BufferId;

    /// Unregister buffer, returning the arc if it existed.
    fn unregister(&self, id: BufferId) -> Option<Arc<RwLock<dyn KernelBuffer>>>;

    /// List all buffer IDs. Order is not guaranteed; callers that need
    /// deterministic ordering must sort the result.
    fn list(&self) -> Vec<BufferId>;

    /// Get count of buffers.
    fn count(&self) -> usize;
}
