//! Per-session codec state stored in `ExtensionMap`.
//!
//! Stores per-buffer codec metadata using the same pattern as
//! [`SyntaxSessionState`]. When a file is opened, its `CodecMetadata`
//! is stored here keyed by buffer ID. When the file is saved, the
//! metadata is retrieved to encode back in the correct format.
//!
//! # Architecture
//!
//! ```text
//! Session
//!   └─ ExtensionMap
//!        └─ CodecSessionState
//!             └─ HashMap<BufferId, CodecMetadata>
//! ```
//!
//! # Usage from Commands
//!
//! ```ignore
//! // During :e (open)
//! let codec_state = runtime.ext::<CodecSessionState>();
//! codec_state.insert(buffer_id, result.metadata);
//!
//! // During :w (save)
//! let codec_state = runtime.ext::<CodecSessionState>();
//! if let Some(metadata) = codec_state.get(buffer_id) {
//!     let bytes = codec.encode(&content, metadata)?;
//!     vfs.write(path, &bytes);
//! }
//! ```

use std::collections::HashMap;

use {reovim_driver_session::SessionExtension, reovim_kernel::api::v1::BufferId};

use crate::CodecMetadata;

/// Per-session codec metadata storage.
///
/// Maps buffer IDs to their codec metadata. Each buffer can have
/// at most one codec metadata entry (the result of its most recent
/// decode operation).
#[derive(Default)]
pub struct CodecSessionState {
    /// Metadata per buffer (`BufferId.as_usize()` -> `CodecMetadata`).
    metadata: HashMap<usize, CodecMetadata>,
}

impl SessionExtension for CodecSessionState {
    fn create() -> Self {
        Self::default()
    }
}

impl CodecSessionState {
    /// Create a new empty state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Store metadata for a buffer.
    ///
    /// Replaces any existing metadata for this buffer.
    pub fn insert(&mut self, buffer_id: BufferId, metadata: CodecMetadata) {
        self.metadata.insert(buffer_id.as_usize(), metadata);
    }

    /// Get metadata for a buffer.
    #[must_use]
    pub fn get(&self, buffer_id: BufferId) -> Option<&CodecMetadata> {
        self.metadata.get(&buffer_id.as_usize())
    }

    /// Remove metadata for a buffer.
    ///
    /// Call this when a buffer is closed.
    pub fn remove(&mut self, buffer_id: BufferId) -> Option<CodecMetadata> {
        self.metadata.remove(&buffer_id.as_usize())
    }

    /// Check if a buffer has codec metadata.
    #[must_use]
    pub fn contains(&self, buffer_id: BufferId) -> bool {
        self.metadata.contains_key(&buffer_id.as_usize())
    }

    /// Get the number of buffers with metadata.
    #[must_use]
    pub fn len(&self) -> usize {
        self.metadata.len()
    }

    /// Check if no buffers have metadata.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.metadata.is_empty()
    }

    /// Clear all metadata.
    pub fn clear(&mut self) {
        self.metadata.clear();
    }
}

impl std::fmt::Debug for CodecSessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodecSessionState")
            .field("buffer_count", &self.metadata.len())
            .finish()
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
