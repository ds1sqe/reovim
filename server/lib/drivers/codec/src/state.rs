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

use crate::{ByteNotifiable, CodecMetadata};

/// Per-session codec metadata storage.
///
/// Maps buffer IDs to their codec metadata. Each buffer can have
/// at most one codec metadata entry (the result of its most recent
/// decode operation).
///
/// Also caches raw bytes per-buffer for view switching: when a codec
/// supports multiple views (e.g., structured summary + hex dump),
/// switching views requires re-decoding the same raw bytes with a
/// different view name. The raw bytes cache avoids re-reading from disk.
#[derive(Default)]
pub struct CodecSessionState {
    /// Metadata per buffer (`BufferId.as_usize()` -> `CodecMetadata`).
    metadata: HashMap<usize, CodecMetadata>,
    /// Cached raw bytes per buffer for view switching.
    raw_bytes: HashMap<usize, Vec<u8>>,
    /// Active view name per buffer (e.g., `"default"`, `"hex"`).
    active_view: HashMap<usize, String>,
    /// Active codec index per buffer for incremental byte-edit notification.
    ///
    /// When present, the session runtime routes [`ByteEdit`] notifications
    /// through [`ByteNotifiable::notify_byte_edit`] after every mutation.
    /// Per-buffer (shared across clients), not per-client — the byte-level
    /// index tracks shared buffer state.
    indices: HashMap<usize, Box<dyn ByteNotifiable>>,
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

    /// Remove metadata, raw bytes, active view, and index for a buffer.
    ///
    /// Call this when a buffer is closed.
    pub fn remove(&mut self, buffer_id: BufferId) -> Option<CodecMetadata> {
        let key = buffer_id.as_usize();
        self.raw_bytes.remove(&key);
        self.active_view.remove(&key);
        self.indices.remove(&key);
        self.metadata.remove(&key)
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

    /// Clear all metadata, raw bytes, active views, and indices.
    pub fn clear(&mut self) {
        self.metadata.clear();
        self.raw_bytes.clear();
        self.active_view.clear();
        self.indices.clear();
    }

    /// Store raw bytes for a buffer (for view switching).
    pub fn insert_raw(&mut self, buffer_id: BufferId, raw: Vec<u8>) {
        self.raw_bytes.insert(buffer_id.as_usize(), raw);
    }

    /// Get cached raw bytes for a buffer.
    #[must_use]
    pub fn get_raw(&self, buffer_id: BufferId) -> Option<&[u8]> {
        self.raw_bytes.get(&buffer_id.as_usize()).map(Vec::as_slice)
    }

    /// Remove cached raw bytes for a buffer.
    pub fn remove_raw(&mut self, buffer_id: BufferId) {
        self.raw_bytes.remove(&buffer_id.as_usize());
    }

    /// Set the active view name for a buffer.
    pub fn set_active_view(&mut self, buffer_id: BufferId, view: String) {
        self.active_view.insert(buffer_id.as_usize(), view);
    }

    /// Get the active view name for a buffer.
    #[must_use]
    pub fn active_view(&self, buffer_id: BufferId) -> Option<&str> {
        self.active_view
            .get(&buffer_id.as_usize())
            .map(String::as_str)
    }

    // ── Index management (#740 D.2) ───────────────────────────────────────

    /// Register a codec index for a buffer.
    ///
    /// The index receives [`ByteEdit`](reovim_kernel::api::v1::ByteEdit)
    /// notifications via [`ByteNotifiable::notify_byte_edit`] after every
    /// production mutation.
    pub fn set_index(&mut self, buffer_id: BufferId, index: Box<dyn ByteNotifiable>) {
        self.indices.insert(buffer_id.as_usize(), index);
    }

    /// Notify the codec index for a buffer about a byte-level edit.
    ///
    /// No-op if no index is registered for this buffer.
    pub fn notify_index(&mut self, buffer_id: BufferId, edit: &reovim_kernel::api::v1::ByteEdit) {
        if let Some(index) = self.indices.get_mut(&buffer_id.as_usize()) {
            index.notify(edit);
        }
    }

    /// Check whether a buffer has an active codec index.
    #[must_use]
    pub fn has_index(&self, buffer_id: BufferId) -> bool {
        self.indices.contains_key(&buffer_id.as_usize())
    }

    /// Remove the codec index for a buffer.
    pub fn remove_index(&mut self, buffer_id: BufferId) {
        self.indices.remove(&buffer_id.as_usize());
    }
}

impl std::fmt::Debug for CodecSessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodecSessionState")
            .field("buffer_count", &self.metadata.len())
            .field("cached_raw_count", &self.raw_bytes.len())
            .field("active_view_count", &self.active_view.len())
            .field("index_count", &self.indices.len())
            .finish()
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
