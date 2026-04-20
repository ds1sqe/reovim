//! `RenderCodecRegistry` trait and default thread-safe implementation.
//!
//! Platform codec modules (`ext/render-codec/<variant>/`) register their
//! `Arc<dyn Codec>` implementations on load (via `declare_module! on_load`)
//! and unregister them on unload (`on_unload`). The platform client uses
//! `get` to dispatch an incoming `RenderDescriptor` to the correct codec
//! for decoding.

use {
    parking_lot::RwLock,
    reovim_kernel::api::v1::Service,
    reovim_render_codec::Codec,
    std::{collections::HashMap, sync::Arc},
};

/// Server-internal contract for render-codec registration by `kind` value.
///
/// Mirrors [`reovim_subsys_surface_codec::SurfaceCodecRegistry`] for the
/// render concern. One codec per kind; re-registering replaces.
pub trait RenderCodecRegistry: Send + Sync {
    /// Register a codec. If a codec with the same `kind()` is already
    /// registered, it is replaced.
    fn register(&self, codec: Arc<dyn Codec>);

    /// Unregister the codec with the given `kind`. No-op if not registered.
    fn unregister(&self, kind: u16);

    /// Retrieve the codec registered for `kind`, or `None` if absent.
    fn get(&self, kind: u16) -> Option<Arc<dyn Codec>>;
}

/// Default thread-safe [`RenderCodecRegistry`] backed by a
/// `parking_lot::RwLock`.
#[derive(Default)]
pub struct DefaultRenderCodecRegistry {
    inner: RwLock<HashMap<u16, Arc<dyn Codec>>>,
}

impl std::fmt::Debug for DefaultRenderCodecRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kinds: Vec<u16> = {
            let guard = self.inner.read();
            guard.keys().copied().collect()
        };
        f.debug_struct("DefaultRenderCodecRegistry")
            .field("registered_kinds", &kinds)
            .finish()
    }
}

impl DefaultRenderCodecRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// Allow `DefaultRenderCodecRegistry` to be stored and retrieved via
/// `ServiceRegistry::get_or_create::<DefaultRenderCodecRegistry>()`.
impl Service for DefaultRenderCodecRegistry {}

impl RenderCodecRegistry for DefaultRenderCodecRegistry {
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
