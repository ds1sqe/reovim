//! Buffer manager trait for kernel-driver communication.
//!
//! Defines the interface for buffer storage and retrieval. The kernel provides
//! pure storage mechanisms; drivers handle I/O operations (loading, saving).

use std::{fmt, sync::Arc};

use reovim_arch::sync::RwLock;

use crate::mm::{Buffer, BufferId};

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

/// Buffer manager interface for kernel-driver communication.
///
/// This trait defines how buffers are stored and retrieved. The kernel provides
/// pure storage mechanisms, while drivers implement I/O operations.
///
/// # Design Philosophy
///
/// - **No I/O operations**: No `open()/save()` methods - VFS driver handles these
/// - **No focus tracking**: Active buffer tracking is a runtime/window concern
/// - **Thread-safe**: Uses `Arc<RwLock<Buffer>>` for concurrent access
/// - **Kernel purity**: Pure mechanisms only, no external dependencies
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::api::v1::{BufferManager, Buffer, BufferId};
///
/// struct SimpleBufferManager { /* ... */ }
///
/// impl BufferManager for SimpleBufferManager {
///     fn get(&self, id: BufferId) -> Option<Arc<RwLock<Buffer>>> {
///         // Implementation
///     }
///     // ... other methods
/// }
/// ```
pub trait BufferManager: Send + Sync {
    /// Get buffer by ID.
    ///
    /// Returns `None` if the buffer does not exist.
    fn get(&self, id: BufferId) -> Option<Arc<RwLock<Buffer>>>;

    /// Create a new empty buffer.
    ///
    /// Returns the ID of the newly created buffer.
    fn create(&self) -> BufferId;

    /// Register an existing buffer (used by drivers after loading).
    ///
    /// Returns the ID assigned to the registered buffer.
    fn register(&self, buffer: Buffer) -> BufferId;

    /// Unregister buffer, returning ownership.
    ///
    /// # Errors
    ///
    /// Returns `Err(BufferError::NotFound)` if the buffer does not exist.
    fn unregister(&self, id: BufferId) -> Result<Buffer, BufferError>;

    /// List all buffer IDs.
    fn list(&self) -> Vec<BufferId>;

    /// Get count of buffers.
    fn count(&self) -> usize;
}
