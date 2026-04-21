//! `SurfaceEncoder` trait and default thread-safe registry.
//!
//! A surface encoder reads one or more capability slots from a
//! [`FrameTarget`](crate::FrameTarget) and serializes them into bytes
//! (for transport back to the server, for capture tooling, etc.). This
//! is the reverse direction of [`RenderHandler`](crate::RenderHandler).
//!
//! Contract and registry shape parallel the render side.

use {
    parking_lot::RwLock,
    std::{collections::HashMap, sync::Arc},
};

/// Errors a [`SurfaceEncoder`] may surface while reading a target.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfaceEncoderError {
    /// Encoder expected a capability slot on the target that was not
    /// present.
    CapabilityMissing {
        /// Human-readable description of the missing capability.
        expected: &'static str,
    },
    /// Capability state was invalid for encoding (e.g., zero-size grid).
    InvalidState {
        /// Static description of the encoding failure.
        reason: &'static str,
    },
}

impl std::fmt::Display for SurfaceEncoderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CapabilityMissing { expected } => {
                write!(f, "capability missing on target: expected {expected}")
            }
            Self::InvalidState { reason } => write!(f, "invalid capability state: {reason}"),
        }
    }
}

impl std::error::Error for SurfaceEncoderError {}

/// Encodes capability slots on a [`FrameTarget`](crate::FrameTarget)
/// into a byte payload. One encoder per `kind` value.
pub trait SurfaceEncoder: Send + Sync {
    /// Numeric discriminant identifying the payload shape this encoder
    /// produces. Each encoder registered in a
    /// [`SurfaceEncoderRegistry`] must have a unique `kind()`.
    fn kind(&self) -> u16;

    /// Encode the capability slot(s) on `target` into a byte payload.
    ///
    /// # Errors
    ///
    /// Returns [`SurfaceEncoderError`] when the required capability slot
    /// is absent or its state is invalid for encoding.
    fn encode_from(
        &self,
        target: &crate::FrameTarget,
    ) -> Result<Vec<u8>, SurfaceEncoderError>;
}

/// Thread-safe registry of [`SurfaceEncoder`] trait objects keyed by
/// `kind` value.
pub trait SurfaceEncoderRegistry: Send + Sync {
    /// Register an encoder. Replaces any existing encoder for the same
    /// `kind()`.
    fn register(&self, encoder: Arc<dyn SurfaceEncoder>);

    /// Unregister the encoder for `kind`. No-op if none is registered.
    fn unregister(&self, kind: u16);

    /// Retrieve the encoder registered for `kind`, or `None` if absent.
    fn get(&self, kind: u16) -> Option<Arc<dyn SurfaceEncoder>>;
}

/// Default thread-safe [`SurfaceEncoderRegistry`] backed by
/// `parking_lot::RwLock`.
#[derive(Default)]
pub struct DefaultSurfaceEncoderRegistry {
    inner: RwLock<HashMap<u16, Arc<dyn SurfaceEncoder>>>,
}

impl std::fmt::Debug for DefaultSurfaceEncoderRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kinds: Vec<u16> = self.inner.read().keys().copied().collect();
        f.debug_struct("DefaultSurfaceEncoderRegistry")
            .field("registered_kinds", &kinds)
            .finish()
    }
}

impl DefaultSurfaceEncoderRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl SurfaceEncoderRegistry for DefaultSurfaceEncoderRegistry {
    fn register(&self, encoder: Arc<dyn SurfaceEncoder>) {
        let kind = encoder.kind();
        self.inner.write().insert(kind, encoder);
    }

    fn unregister(&self, kind: u16) {
        self.inner.write().remove(&kind);
    }

    fn get(&self, kind: u16) -> Option<Arc<dyn SurfaceEncoder>> {
        self.inner.read().get(&kind).cloned()
    }
}
