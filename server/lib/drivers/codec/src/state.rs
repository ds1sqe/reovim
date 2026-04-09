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

use std::{collections::HashMap, sync::Arc};

use {
    reovim_driver_session::SessionExtension,
    reovim_kernel::api::v1::{BufferId, ByteEdit},
};

use crate::{
    ByteNotifiable, CodecMetadata, ContentCodec, ContentCodecFactoryStore, DecodedEdit, InodeTable,
    Mount, SwitchViewError,
};

/// Per-session codec storage.
///
/// Maps buffer IDs to their codec metadata. Each buffer can have
/// at most one codec metadata entry (the result of its most recent
/// decode operation).
///
/// Also owns canonical byte storage via [`InodeTable`], which keeps a
/// single source of truth for decoded raw bytes.
///
/// # Bugs deleted by architecture (`#740` Phase 3)
///
/// - **B1** (HIGH — data loss): stale per-buffer byte caches drifted from
///   canonical disk-backed bytes after edits. Canonical bytes now live in
///   [`InodeTable::Inode`], so the migration is complete by structure.
/// - **B2** (HIGH — type safety): [`crate::DecodeResult::readonly`] is
///   discarded by every consumer. Phase 3 replaces the runtime flag with
///   `ContentCodec::translate_edit` returning `Some`/`None`, making
///   writability a compile-time codec capability. No Phase 0 patch.
/// - **B4** (LATENT — index stale): `set_index` registers a per-buffer
///   `dyn ByteNotifiable` index, but no production caller wires the
///   notification path today, so the index can drift from the underlying
///   bytes after edits. Becomes load-bearing in Phase 5 once `Mount.index`
///   participates in `translate_edit` peer notification; documented as a
///   known limitation here until then.
#[derive(Default)]
pub struct CodecSessionState {
    /// Metadata per buffer (`BufferId.as_usize()` -> `CodecMetadata`).
    metadata: HashMap<usize, CodecMetadata>,
    /// Canonical byte sources and mounts used by codec views.
    inodes: InodeTable,
    /// Active view name per buffer (e.g., `"default"`, `"hex"`).
    active_view: HashMap<usize, String>,
    /// Active codec index per buffer for incremental byte-edit notification.
    ///
    /// When present, the session runtime routes [`ByteEdit`] notifications
    /// through [`ByteNotifiable::notify_byte_edit`] after every mutation.
    /// Per-buffer (shared across clients), not per-client — the byte-level
    /// index tracks shared buffer state.
    ///
    /// KNOWN LIMITATION (`#740` B4): no production caller wires the
    /// notification path today, so the index drifts after edits. Phase 5
    /// makes this load-bearing via `Mount.index` and the multi-mount
    /// `translate_edit` peer-notification flow.
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

    /// Remove metadata, canonical source, active view, codec binding, and index for a
    /// buffer.
    ///
    /// Call this when a buffer is closed.
    pub fn remove(&mut self, buffer_id: BufferId) -> Option<CodecMetadata> {
        let key = buffer_id.as_usize();
        let _ = self.inodes.remove_file(buffer_id);
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

    /// Clear all metadata, canonical sources, active views, and indices.
    pub fn clear(&mut self) {
        self.metadata.clear();
        self.inodes = InodeTable::new();
        self.active_view.clear();
        self.indices.clear();
    }

    /// Register canonical bytes for a buffer.
    pub fn set_source(&mut self, buffer_id: BufferId, raw: Vec<u8>) {
        if let Some(inode_id) = self.inodes.file_inode(buffer_id) {
            let _ = self.inodes.set_bytes(inode_id, raw);
        } else {
            let inode_id = self
                .inodes
                .insert(Arc::new(reovim_driver_vfs::HeapByteSource::new(raw)));
            self.inodes.bind_file(buffer_id, inode_id);
        }
    }

    /// Register canonical bytes and decoded codec for a buffer.
    pub fn set_source_with_codec(
        &mut self,
        buffer_id: BufferId,
        raw: Vec<u8>,
        codec: Arc<dyn ContentCodec>,
    ) {
        let key = buffer_id.as_usize();
        let view = self
            .active_view
            .get(&key)
            .cloned()
            .unwrap_or_else(|| "default".to_string());

        let Some(inode_id) = self.inodes.file_inode(buffer_id) else {
            let inode_id = self
                .inodes
                .insert(Arc::new(reovim_driver_vfs::HeapByteSource::new(raw)));
            self.inodes.bind_file(buffer_id, inode_id);
            self.inodes
                .mount(inode_id, buffer_id, Mount::new(view, codec))
                .ok();
            return;
        };

        let _ = self.inodes.set_bytes(inode_id, raw);
        let _ = self.inodes.unmount_all(buffer_id);
        let _ = self
            .inodes
            .mount(inode_id, buffer_id, Mount::new(view, codec));
    }

    /// Read canonical bytes for a buffer.
    #[must_use]
    pub fn bytes(&self, buffer_id: BufferId) -> Option<Vec<u8>> {
        self.inodes
            .file_inode(buffer_id)
            .and_then(|inode_id| self.inodes.read_bytes(inode_id).ok())
    }

    /// Apply a byte-level edit to the canonical source for a buffer.
    pub fn apply_byte_edit(&mut self, buffer_id: BufferId, edit: &ByteEdit) {
        if let Some(inode_id) = self.inodes.file_inode(buffer_id) {
            let _ = self.inodes.apply_byte_edit(inode_id, edit);
        }
    }

    /// Apply a decoded edit through the active mount for this buffer.
    ///
    /// Returns the translated `ByteEdit` when decoding succeeds so callers
    /// can notify indices with byte coordinates.
    pub fn apply_decoded_edit(
        &mut self,
        buffer_id: BufferId,
        _view: &str,
        edit: &DecodedEdit,
    ) -> Option<ByteEdit> {
        let handle = self.inodes.active_mount(buffer_id)?;
        let byte_edit = self.inodes.apply_edit(handle, edit).ok()?;
        Some(byte_edit)
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

    // ── Orchestration helpers (#740 Phase 4) ───────────────────────────────

    /// Record a freshly-decoded codec attachment on a buffer.
    ///
    /// Replaces any existing metadata, active view, and canonical source
    /// binding for `buffer_id`. This atomicises the trio of calls
    /// (`insert` + `set_active_view` + `set_source_with_codec`) that `:e`
    /// and the large-file streaming path previously did inline — the
    /// orchestration now lives inside the codec driver so gRPC handlers
    /// and command modules never reach into codec state piecewise.
    pub fn mount_decoded(
        &mut self,
        buffer_id: BufferId,
        metadata: CodecMetadata,
        view: String,
        bytes: Vec<u8>,
        codec: Arc<dyn ContentCodec>,
    ) {
        self.insert(buffer_id, metadata);
        self.set_active_view(buffer_id, view);
        self.set_source_with_codec(buffer_id, bytes, codec);
    }

    /// Decode the buffer's canonical bytes through a different codec view
    /// and rebind the codec state for that view.
    ///
    /// This is the orchestration body that used to live inside the
    /// `SwitchCodecView` gRPC handler. The caller retains responsibility
    /// for writing the returned content into the text buffer — that is a
    /// session/provider concern — but every codec-side mutation happens
    /// atomically here so the handler shrinks to arg resolution + dispatch.
    ///
    /// `factories` is accepted as `Option` so the handler can keep
    /// `codec_state` / `factory_store` preflight checks in a single place
    /// without duplicating the bytes/metadata ordering. A missing factory
    /// store surfaces as `NoCodec` after the bytes check, matching the
    /// historic handler error order.
    ///
    /// # Errors
    ///
    /// - [`SwitchViewError::NoMetadata`] — no codec metadata recorded for
    ///   the buffer.
    /// - [`SwitchViewError::NoCanonicalBytes`] — canonical inode bytes for
    ///   the buffer are missing.
    /// - [`SwitchViewError::NoCodec`] — factory store is missing or has no
    ///   codec registered for the recorded content type.
    /// - [`SwitchViewError::ViewNotAvailable`] — the active codec does not
    ///   expose a view with the requested name.
    /// - [`SwitchViewError::DecodeFailed`] — the codec returned an error
    ///   while decoding the requested view.
    pub fn switch_view(
        &mut self,
        factories: Option<&ContentCodecFactoryStore>,
        buffer_id: BufferId,
        view_name: &str,
    ) -> Result<String, SwitchViewError> {
        let content_type = self
            .get(buffer_id)
            .ok_or(SwitchViewError::NoMetadata)?
            .content_type()
            .clone();

        let source_bytes = self
            .bytes(buffer_id)
            .ok_or(SwitchViewError::NoCanonicalBytes)?;

        let codec_box = factories
            .and_then(|store| store.find(&content_type))
            .ok_or(SwitchViewError::NoCodec)?;
        let codec: Arc<dyn ContentCodec> = codec_box.into();

        if !codec.views().iter().any(|v| v.name == view_name) {
            return Err(SwitchViewError::ViewNotAvailable {
                view_name: view_name.to_string(),
            });
        }

        let result = codec.decode_view(&source_bytes, view_name).map_err(|e| {
            SwitchViewError::DecodeFailed {
                reason: e.to_string(),
            }
        })?;

        self.mount_decoded(buffer_id, result.metadata, view_name.to_string(), source_bytes, codec);

        // View switch invalidates any previously-built codec index; Phase 5
        // wires rebuild via `Mount.index`, this phase just clears the stale
        // entry so no consumer observes mismatched byte coordinates.
        if self.has_index(buffer_id) {
            self.remove_index(buffer_id);
        }

        Ok(result.content)
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
            .field("source_count", &self.inodes.file_len())
            .field("active_view_count", &self.active_view.len())
            .field("index_count", &self.indices.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
