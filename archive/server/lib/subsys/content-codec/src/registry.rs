//! `ContentCodecRegistry` trait and default thread-safe implementation.
//!
//! Platform codec modules (`ext/content-codec/<variant>/`) register their
//! `Arc<dyn ContentCodec>` implementations on load (via `declare_module!
//! on_load`) and unregister them on unload (`on_unload`). The server uses
//! `get` to dispatch classifier decisions to the concrete codec.

use {
    parking_lot::RwLock,
    reovim_content_codec::{ContentCodec, ContentType},
    reovim_kernel::api::v1::Service,
    std::{collections::HashMap, sync::Arc},
};

/// Server-internal contract for content-codec registration by
/// [`ContentType`].
///
/// Mirrors [`reovim_subsys_input::InputCodecRegistry`] for the content
/// concern. One codec per content type; re-registering replaces.
pub trait ContentCodecRegistry: Send + Sync {
    /// Register a codec for a content type. Replaces any existing codec
    /// registered for the same type.
    fn register(&self, content_type: ContentType, codec: Arc<dyn ContentCodec>);

    /// Unregister the codec for a content type. No-op if not registered.
    fn unregister(&self, content_type: &ContentType);

    /// Retrieve the codec registered for `content_type`, or `None` if absent.
    fn get(&self, content_type: &ContentType) -> Option<Arc<dyn ContentCodec>>;
}

/// Default thread-safe [`ContentCodecRegistry`] backed by a
/// `parking_lot::RwLock`.
#[derive(Default)]
pub struct DefaultContentCodecRegistry {
    inner: RwLock<HashMap<ContentType, Arc<dyn ContentCodec>>>,
}

impl std::fmt::Debug for DefaultContentCodecRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let types: Vec<ContentType> = {
            let guard = self.inner.read();
            guard.keys().cloned().collect()
        };
        f.debug_struct("DefaultContentCodecRegistry")
            .field("registered_types", &types)
            .finish()
    }
}

impl DefaultContentCodecRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// Allow `DefaultContentCodecRegistry` to be stored and retrieved via
/// `ServiceRegistry::get_or_create::<DefaultContentCodecRegistry>()`.
impl Service for DefaultContentCodecRegistry {}

impl ContentCodecRegistry for DefaultContentCodecRegistry {
    fn register(&self, content_type: ContentType, codec: Arc<dyn ContentCodec>) {
        self.inner.write().insert(content_type, codec);
    }

    fn unregister(&self, content_type: &ContentType) {
        self.inner.write().remove(content_type);
    }

    fn get(&self, content_type: &ContentType) -> Option<Arc<dyn ContentCodec>> {
        self.inner.read().get(content_type).cloned()
    }
}
