//! `RenderHandler` trait and default thread-safe registry.
//!
//! A render handler decodes a payload byte slice (produced by the server
//! and shipped over the wire) into one or more capability slots on a
//! [`FrameTarget`](crate::FrameTarget). One handler is registered per
//! `kind` value; ext crates call [`RenderHandlerRegistry::register`] on
//! load and [`RenderHandlerRegistry::unregister`] on unload.
//!
//! Trait objects are `Send + Sync` because handler registration and
//! dispatch can occur on different tasks (loader thread vs. render
//! thread), and the registry is stored in a cross-thread `Arc` inside
//! `ClientServiceRegistry`.

use {
    parking_lot::RwLock,
    std::{collections::HashMap, sync::Arc},
};

/// Errors a [`RenderHandler`] may surface while decoding a payload.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderHandlerError {
    /// Payload was shorter than the minimum required by the handler.
    TooShort {
        /// Number of bytes actually received.
        got: usize,
        /// Minimum number of bytes required.
        min: usize,
    },
    /// Payload structure was otherwise invalid (magic mismatch, bad
    /// coordinates, etc.). `reason` is a static description.
    InvalidData {
        /// Static description of the decoding failure.
        reason: &'static str,
    },
    /// Handler expected a capability slot on the target that was
    /// not present (e.g., a cell-grid handler running against an
    /// empty `FrameTarget`).
    CapabilityMissing {
        /// Human-readable description of the missing capability.
        expected: &'static str,
    },
}

impl std::fmt::Display for RenderHandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort { got, min } => {
                write!(f, "payload too short: got {got} bytes, need at least {min}")
            }
            Self::InvalidData { reason } => write!(f, "invalid payload: {reason}"),
            Self::CapabilityMissing { expected } => {
                write!(f, "capability missing on target: expected {expected}")
            }
        }
    }
}

impl std::error::Error for RenderHandlerError {}

/// Decodes a render-payload byte slice into one or more capability slots
/// on a [`FrameTarget`](crate::FrameTarget). One handler per `kind` value.
pub trait RenderHandler: Send + Sync {
    /// Numeric discriminant identifying the payload shape this handler
    /// decodes. Each handler registered in a
    /// [`RenderHandlerRegistry`] must have a unique `kind()`.
    fn kind(&self) -> u16;

    /// Decode `body` into capability slots on `target`.
    ///
    /// # Errors
    ///
    /// Returns [`RenderHandlerError`] when the payload is too short,
    /// malformed, or the target is missing a required capability slot.
    fn render(
        &self,
        body: &[u8],
        target: &mut crate::FrameTarget,
    ) -> Result<(), RenderHandlerError>;
}

/// Thread-safe registry of [`RenderHandler`] trait objects keyed by
/// `kind` value.
pub trait RenderHandlerRegistry: Send + Sync {
    /// Register a handler. Replaces any existing handler for the same
    /// `kind()`.
    fn register(&self, handler: Arc<dyn RenderHandler>);

    /// Unregister the handler for `kind`. No-op if none is registered.
    fn unregister(&self, kind: u16);

    /// Retrieve the handler registered for `kind`, or `None` if absent.
    fn get(&self, kind: u16) -> Option<Arc<dyn RenderHandler>>;
}

/// Default thread-safe [`RenderHandlerRegistry`] backed by
/// `parking_lot::RwLock`.
#[derive(Default)]
pub struct DefaultRenderHandlerRegistry {
    inner: RwLock<HashMap<u16, Arc<dyn RenderHandler>>>,
}

impl std::fmt::Debug for DefaultRenderHandlerRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kinds: Vec<u16> = self.inner.read().keys().copied().collect();
        f.debug_struct("DefaultRenderHandlerRegistry")
            .field("registered_kinds", &kinds)
            .finish()
    }
}

impl DefaultRenderHandlerRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl RenderHandlerRegistry for DefaultRenderHandlerRegistry {
    fn register(&self, handler: Arc<dyn RenderHandler>) {
        let kind = handler.kind();
        self.inner.write().insert(kind, handler);
    }

    fn unregister(&self, kind: u16) {
        self.inner.write().remove(&kind);
    }

    fn get(&self, kind: u16) -> Option<Arc<dyn RenderHandler>> {
        self.inner.read().get(&kind).cloned()
    }
}
