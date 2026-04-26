//! `TextBufferDriverImpl` — `reovim_subsys_buffer::BufferDriver`
//! implementation that owns a population of [`TextBufferImpl`]
//! instances over rope-backed `provider-text` storage.
//!
//! The driver tracks two parallel structures:
//!
//! - `registry: TextBufferRegistry` — the legacy text-buffer registry
//!   re-exported from `provider-text`. Server-side text consumers
//!   (motion engine, viewport rendering, etc.) read from this registry
//!   directly via `BufferOps`.
//! - `buffers: BTreeMap<BufferId, Arc<TextBufferImpl>>` — the cdylib
//!   wrappers that satisfy the `BufferDriver` contract.
//!
//! Both structures hold the *same* `Arc<RwLock<dyn BufferOps>>`
//! storage; the registry is the read-side text view and `buffers` is
//! the byte-side ABI surface. Phase 5 will retire the dual-tracking
//! once consumers route through the buffer subsys.

use {
    crate::{BufferOps, TextBufferImpl, probe},
    parking_lot::RwLock,
    reovim_arch::sync::RwLock as ArchRwLock,
    reovim_kernel::api::v1::BufferId,
    reovim_provider_text::{Buffer as ProviderBuffer, TextBufferRegistry},
    reovim_subsys_buffer::{Buffer, BufferDriver, BufferError, abi::BufferDriverProbe},
    std::{collections::BTreeMap, sync::Arc},
};

/// Text-buffer driver implementation backed by `provider-text` ropes.
pub struct TextBufferDriverImpl {
    registry: TextBufferRegistry,
    buffers: RwLock<BTreeMap<BufferId, Arc<TextBufferImpl>>>,
}

impl TextBufferDriverImpl {
    /// Construct an empty driver.
    #[must_use]
    pub fn new() -> Self {
        Self {
            registry: TextBufferRegistry::new(),
            buffers: RwLock::new(BTreeMap::new()),
        }
    }

    /// Static probe metadata invoked pre-construct by the cdylib host.
    #[must_use]
    pub fn probe() -> BufferDriverProbe {
        BufferDriverProbe::new(probe::KIND, probe::NAME)
    }

    /// Construct a driver instance for the cdylib host trampoline.
    ///
    /// # Errors
    ///
    /// Currently infallible; the signature mirrors the SP02 net-grpc
    /// driver so the macro-emitted construct trampoline can map a
    /// `Result` directly onto the C ABI return value.
    pub fn construct() -> Result<Self, BufferError> {
        Ok(Self::new())
    }

    /// Read-side handle to the text-buffer registry, for server-side
    /// text consumers that have not yet migrated to the buffer subsys.
    ///
    /// This stays public for the duration of Phases 5–7; it disappears
    /// once consumers route exclusively through `BufferDriver`.
    #[must_use]
    pub const fn registry(&self) -> &TextBufferRegistry {
        &self.registry
    }
}

impl Default for TextBufferDriverImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl BufferDriver for TextBufferDriverImpl {
    fn kind(&self) -> &'static str {
        probe::KIND
    }

    fn create_buffer(
        &self,
        initial_bytes: &[u8],
        file_path: Option<String>,
    ) -> Result<Arc<dyn Buffer>, BufferError> {
        // Text buffers require UTF-8. Bytes that fail UTF-8 validation
        // surface as `InvalidEdit` — the closest existing error variant
        // for "the supplied bytes are not representable as text".
        let initial_text = std::str::from_utf8(initial_bytes)
            .map_err(|e| BufferError::InvalidEdit(format!("non-UTF-8 initial bytes: {e}")))?;
        let mut provider_buffer = ProviderBuffer::from_string(initial_text);
        if let Some(ref path) = file_path {
            provider_buffer.set_file_path(Some(path.clone()));
        }
        let id = provider_buffer.id();

        // Wrap in the dyn-BufferOps storage shape the registry uses.
        let inner: Arc<ArchRwLock<dyn BufferOps>> = Arc::new(ArchRwLock::new(provider_buffer));
        // Both structures share the same `inner` Arc — text consumers
        // see edits made through the cdylib `apply_edit` path, and vice
        // versa.
        self.registry.register(inner.clone());

        let wrapper = Arc::new(TextBufferImpl::new(id, inner, file_path));
        self.buffers.write().insert(id, wrapper.clone());
        Ok(wrapper as Arc<dyn Buffer>)
    }

    fn open_buffer(&self, file_path: &str) -> Result<Arc<dyn Buffer>, BufferError> {
        let bytes = std::fs::read(file_path)
            .map_err(|e| BufferError::Io(format!("read {file_path}: {e}")))?;
        self.create_buffer(&bytes, Some(file_path.to_string()))
    }

    fn list_buffers(&self) -> Vec<BufferId> {
        self.buffers.read().keys().copied().collect()
    }

    fn get_buffer(&self, id: BufferId) -> Option<Arc<dyn Buffer>> {
        self.buffers
            .read()
            .get(&id)
            .cloned()
            .map(|arc| arc as Arc<dyn Buffer>)
    }

    fn close_buffer(&self, id: BufferId) -> Result<(), BufferError> {
        let removed = {
            let mut buffers = self.buffers.write();
            buffers.remove(&id).is_some()
        };
        if !removed {
            return Err(BufferError::NotFound(id));
        }
        // Best-effort registry cleanup; missing entry is fine.
        let _ = self.registry.unregister(id);
        Ok(())
    }
}
