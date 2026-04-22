//! Opaque render target for domain content submission.
//!
//! [`RenderTarget`] defines a platform-agnostic byte-submission interface.
//! Domain view modules submit encoded render commands; platform adapters
//! decode them using a platform-specific render pipeline.

/// Error returned when a render submission fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    /// The submitted data could not be decoded by the platform adapter.
    InvalidData(String),
    /// The render target is not ready (e.g., surface not yet initialized).
    NotReady,
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidData(msg) => write!(f, "invalid render data: {msg}"),
            Self::NotReady => write!(f, "render target not ready"),
        }
    }
}

impl std::error::Error for RenderError {}

/// Opaque render target for domain content submission.
///
/// Domain view modules call [`submit`](RenderTarget::submit) with encoded
/// render command bytes. Platform adapters implement this trait and decode
/// the bytes using a platform-specific render pipeline.
///
/// No coordinates or surface dimensions appear in this trait — those are
/// encoded in the command buffer by the domain driver and decoded by the
/// platform adapter.
pub trait RenderTarget: Send + Sync {
    /// Submit encoded render commands.
    ///
    /// The byte slice is an opaque render payload whose shape is
    /// platform-specific. The platform adapter decodes the bytes and
    /// executes the rendering operation.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if the data cannot be processed.
    fn submit(&mut self, data: &[u8]) -> Result<(), RenderError>;
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
