//! `SurfaceCodecRegistry` trait and default thread-safe implementation.
//!
//! Platform codec modules (`ext/surface-codec/<variant>/`) register their
//! `Arc<dyn Codec>` implementations on load (via `declare_module! on_load`)
//! and unregister them on unload (`on_unload`). The server uses `get` to
//! dispatch an incoming `SurfaceDescriptor` to the correct codec for
//! decoding.

use {
    parking_lot::RwLock,
    reovim_kernel::api::v1::Service,
    reovim_surface_codec::Codec,
    std::{collections::HashMap, sync::Arc},
};

/// Server-internal contract for surface-codec registration by `kind` value.
///
/// Mirrors [`reovim_subsys_input::InputCodecRegistry`] for the surface
/// concern. One codec per kind; re-registering replaces.
pub trait SurfaceCodecRegistry: Send + Sync {
    /// Register a codec. If a codec with the same `kind()` is already
    /// registered, it is replaced.
    fn register(&self, codec: Arc<dyn Codec>);

    /// Unregister the codec with the given `kind`. No-op if not registered.
    fn unregister(&self, kind: u16);

    /// Retrieve the codec registered for `kind`, or `None` if absent.
    fn get(&self, kind: u16) -> Option<Arc<dyn Codec>>;
}

/// Default thread-safe [`SurfaceCodecRegistry`] backed by a
/// `parking_lot::RwLock`.
#[derive(Default)]
pub struct DefaultSurfaceCodecRegistry {
    inner: RwLock<HashMap<u16, Arc<dyn Codec>>>,
}

impl std::fmt::Debug for DefaultSurfaceCodecRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kinds: Vec<u16> = {
            let guard = self.inner.read();
            guard.keys().copied().collect()
        };
        f.debug_struct("DefaultSurfaceCodecRegistry")
            .field("registered_kinds", &kinds)
            .finish()
    }
}

impl DefaultSurfaceCodecRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// Allow `DefaultSurfaceCodecRegistry` to be stored and retrieved via
/// `ServiceRegistry::get_or_create::<DefaultSurfaceCodecRegistry>()`.
impl Service for DefaultSurfaceCodecRegistry {}

impl SurfaceCodecRegistry for DefaultSurfaceCodecRegistry {
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
