//! Input driver traits.
//!
//! Linux equivalent: `include/linux/input.h` (input subsystem)
//!
//! # Architecture
//!
//! ```text
//! +-----------------+
//! | ClipboardProvider |  <-- System clipboard access
//! +-----------------+
//! ```

#![allow(clippy::missing_errors_doc)]

use crate::error::ClipboardError;

/// System clipboard provider.
///
/// Provides read/write access to the system clipboard.
/// Implementations handle platform-specific clipboard APIs.
pub trait ClipboardProvider: Send + Sync {
    /// Read text from the system clipboard.
    fn read(&self) -> Result<String, ClipboardError>;

    /// Write text to the system clipboard.
    fn write(&mut self, text: &str) -> Result<(), ClipboardError>;

    /// Check if clipboard is available.
    fn is_available(&self) -> bool;

    /// Get the name of this provider (for debugging).
    fn name(&self) -> &str;
}
