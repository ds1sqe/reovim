//! `SurfaceDescriptorHandler` trait and default thread-safe registry.
//!
//! A surface descriptor handler decodes a payload byte slice from the
//! wire `SurfaceDescriptorProto { kind: u16, body: Vec<u8> }` envelope
//! (Plan 17 §2.1 locked decision — the envelope already lives in proto
//! v3; dispatch is pure policy) into a typed descriptor value. The
//! caller downcasts the returned `Box<dyn Any + Send>` into the
//! concrete descriptor type exported by the handler's crate.
//!
//! One handler is registered per `kind` value; ext modules call
//! [`SurfaceDescriptorHandlerRegistry::register`] on load (either
//! dynamically via `declare_client_module!` init or statically via
//! bootstrap pre-registration — the latter is required when the
//! registry must be populated before the first `SurfaceChanged`
//! notification arrives).
//!
//! The `Box<dyn Any + Send>` return type mirrors the
//! [`FrameTarget::get_mut::<T>()`](crate::FrameTarget::get_mut)
//! downcast pattern: the trait stays object-safe, new descriptor types
//! compose without breaking changes, and handler implementations
//! document their concrete return type in rustdoc.

use {
    parking_lot::RwLock,
    std::{any::Any, collections::HashMap, sync::Arc},
};

/// Errors a [`SurfaceDescriptorHandler`] may surface while decoding a
/// payload. Shape is parallel to
/// [`RenderHandlerError`](crate::RenderHandlerError).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfaceDescriptorHandlerError {
    /// Payload was shorter than the minimum required by the handler.
    TooShort {
        /// Number of bytes actually received.
        got: usize,
        /// Minimum number of bytes required.
        min: usize,
    },
    /// Payload structure was otherwise invalid (magic mismatch, bad
    /// field, etc.). `reason` is a static description.
    InvalidData {
        /// Static description of the decoding failure.
        reason: &'static str,
    },
    /// A numeric field in the payload exceeded the range accepted by
    /// the handler's typed descriptor (e.g. a `u32` width above
    /// `u16::MAX` when the TUI grid uses `u16` coordinates).
    OutOfRange {
        /// Static description identifying which field was out of range.
        reason: &'static str,
    },
}

impl std::fmt::Display for SurfaceDescriptorHandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort { got, min } => {
                write!(f, "payload too short: got {got} bytes, need at least {min}")
            }
            Self::InvalidData { reason } => write!(f, "invalid payload: {reason}"),
            Self::OutOfRange { reason } => write!(f, "value out of range: {reason}"),
        }
    }
}

impl std::error::Error for SurfaceDescriptorHandlerError {}

/// Decodes a surface-descriptor payload byte slice into a typed
/// descriptor value. One handler per `kind` value.
///
/// Implementations document their concrete boxed type; callers
/// downcast with `Box::downcast::<T>()`.
pub trait SurfaceDescriptorHandler: Send + Sync {
    /// Numeric discriminant identifying the payload shape this handler
    /// decodes. Each handler registered in a
    /// [`SurfaceDescriptorHandlerRegistry`] must have a unique
    /// `kind()`.
    fn kind(&self) -> u16;

    /// Decode `body` into a typed descriptor. Callers downcast the
    /// returned box via `Box::downcast::<T>()` using the concrete
    /// descriptor type exported by the handler's crate.
    ///
    /// # Errors
    ///
    /// Returns [`SurfaceDescriptorHandlerError`] when the payload is
    /// too short, malformed, or contains a field outside the range
    /// accepted by the handler's typed descriptor.
    fn decode(
        &self,
        body: &[u8],
    ) -> Result<Box<dyn Any + Send>, SurfaceDescriptorHandlerError>;
}

/// Thread-safe registry of [`SurfaceDescriptorHandler`] trait objects
/// keyed by `kind` value.
pub trait SurfaceDescriptorHandlerRegistry: Send + Sync {
    /// Register a handler. Replaces any existing handler for the same
    /// `kind()`.
    fn register(&self, handler: Arc<dyn SurfaceDescriptorHandler>);

    /// Unregister the handler for `kind`. No-op if none is registered.
    fn unregister(&self, kind: u16);

    /// Retrieve the handler registered for `kind`, or `None` if
    /// absent.
    fn get(&self, kind: u16) -> Option<Arc<dyn SurfaceDescriptorHandler>>;
}

/// Default thread-safe [`SurfaceDescriptorHandlerRegistry`] backed by
/// `parking_lot::RwLock`.
#[derive(Default)]
pub struct DefaultSurfaceDescriptorHandlerRegistry {
    inner: RwLock<HashMap<u16, Arc<dyn SurfaceDescriptorHandler>>>,
}

impl std::fmt::Debug for DefaultSurfaceDescriptorHandlerRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kinds: Vec<u16> = self.inner.read().keys().copied().collect();
        f.debug_struct("DefaultSurfaceDescriptorHandlerRegistry")
            .field("registered_kinds", &kinds)
            .finish()
    }
}

impl DefaultSurfaceDescriptorHandlerRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl SurfaceDescriptorHandlerRegistry for DefaultSurfaceDescriptorHandlerRegistry {
    fn register(&self, handler: Arc<dyn SurfaceDescriptorHandler>) {
        let kind = handler.kind();
        self.inner.write().insert(kind, handler);
    }

    fn unregister(&self, kind: u16) {
        self.inner.write().remove(&kind);
    }

    fn get(&self, kind: u16) -> Option<Arc<dyn SurfaceDescriptorHandler>> {
        self.inner.read().get(&kind).cloned()
    }
}
