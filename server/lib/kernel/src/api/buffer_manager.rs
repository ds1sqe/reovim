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

    /// List all buffer IDs. Order is not guaranteed; callers that need
    /// deterministic ordering must sort the result.
    fn list(&self) -> Vec<BufferId>;

    /// Get count of buffers.
    fn count(&self) -> usize;
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== BufferError tests ==========

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_buffer_error_not_found_display() {
        let id = BufferId::from_raw(42);
        let err = BufferError::NotFound(id);
        let display = format!("{err}");
        assert!(display.contains("buffer not found"));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_buffer_error_already_exists_display() {
        let id = BufferId::from_raw(7);
        let err = BufferError::AlreadyExists(id);
        let display = format!("{err}");
        assert!(display.contains("buffer already exists"));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_buffer_error_invalid_operation_display() {
        let err = BufferError::InvalidOperation("cannot delete last buffer");
        let display = format!("{err}");
        assert_eq!(display, "invalid operation: cannot delete last buffer");
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_buffer_error_debug() {
        let id = BufferId::from_raw(1);
        let err = BufferError::NotFound(id);
        let debug = format!("{err:?}");
        assert!(debug.contains("NotFound"));
    }

    #[test]
    fn test_buffer_error_clone() {
        let id = BufferId::from_raw(5);
        let err = BufferError::NotFound(id);
        let cloned = err.clone();
        assert_eq!(err, cloned);
    }

    #[test]
    fn test_buffer_error_eq() {
        let id = BufferId::from_raw(1);
        assert_eq!(BufferError::NotFound(id), BufferError::NotFound(id));
        assert_ne!(BufferError::NotFound(id), BufferError::InvalidOperation("test"));
    }

    #[test]
    fn test_buffer_error_is_std_error() {
        let id = BufferId::from_raw(1);
        let err: Box<dyn std::error::Error> = Box::new(BufferError::NotFound(id));
        // Verify it implements std::error::Error
        let _ = format!("{err}");
    }
}
