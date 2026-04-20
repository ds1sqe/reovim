//! `InputCodecRegistry` trait and default thread-safe implementation.
//!
//! The registry is the server-internal contract for registering and
//! looking up input codecs by kind value.  Platform codec modules
//! call `register` on load and `unregister` on unload.

use {
    parking_lot::RwLock,
    reovim_input_codec::Codec,
    reovim_kernel::api::v1::Service,
    std::{collections::HashMap, sync::Arc},
};

/// Server-internal contract for input codec registration.
///
/// Each platform codec module registers its `Arc<dyn Codec>` implementations
/// on load (via `declare_module! on_load`) and unregisters them on unload
/// (`on_unload`).  The server uses `get` to dispatch an incoming
/// `InputEvent` to the correct codec for decoding.
pub trait InputCodecRegistry: Send + Sync {
    /// Register a codec.  If a codec with the same `kind()` is already
    /// registered, it is replaced.
    fn register(&self, codec: Arc<dyn Codec>);

    /// Unregister the codec with the given `kind`.  No-op if not registered.
    fn unregister(&self, kind: u16);

    /// Retrieve the codec registered for `kind`, or `None` if absent.
    fn get(&self, kind: u16) -> Option<Arc<dyn Codec>>;
}

/// Default thread-safe `InputCodecRegistry` backed by a `parking_lot::RwLock`.
///
/// Suitable for use in the server main loop and integration tests.
#[derive(Default)]
pub struct DefaultInputCodecRegistry {
    inner: RwLock<HashMap<u16, Arc<dyn Codec>>>,
}

impl std::fmt::Debug for DefaultInputCodecRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kinds: Vec<u16> = {
            let guard = self.inner.read();
            guard.keys().copied().collect()
        };
        f.debug_struct("DefaultInputCodecRegistry")
            .field("registered_kinds", &kinds)
            .finish()
    }
}

impl DefaultInputCodecRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// Allow `DefaultInputCodecRegistry` to be stored and retrieved via
/// `ServiceRegistry::get_or_create::<DefaultInputCodecRegistry>()`.
impl Service for DefaultInputCodecRegistry {}

impl InputCodecRegistry for DefaultInputCodecRegistry {
    fn register(&self, codec: Arc<dyn Codec>) {
        let kind = codec.kind();
        self.inner.write().insert(kind, codec);
    }

    fn unregister(&self, kind: u16) {
        self.inner.write().remove(&kind);
    }

    fn get(&self, kind: u16) -> Option<Arc<dyn Codec>> {
        self.inner.read().get(&kind).cloned()
    }
}
