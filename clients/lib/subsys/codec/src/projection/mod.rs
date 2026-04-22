//! Domain projection types and proto boundary transpilation.
//!
//! `DomainProjection` is the client-side domain-neutral projection value.
//! The `from_proto` transpiler (behind the `proto` feature gate) is the
//! single proto crossing point; downstream code sees only domain-neutral
//! newtypes from `subsys-coordination`, never proto types.

use reovim_subsys_coordination::{DomainId, ProjectionTag};

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
#[cfg(feature = "proto")]
mod transpiler;

#[cfg(feature = "proto")]
pub use transpiler::from_proto;

/// Error when constructing a `DomainProjection` from a proto payload.
///
/// Only used for the `try_from_proto` path. The standard `from_proto`
/// panics on invalid input per D21 (fail loud).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranspileError {
    /// The required `datum` field was missing from the proto payload.
    MissingDatum,
}

impl std::fmt::Display for TranspileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingDatum => {
                write!(f, "ProjectionUpdatedPayload missing required datum field")
            }
        }
    }
}

impl std::error::Error for TranspileError {}

/// Client-side domain projection — fully reconstructed from proto.
///
/// This is the domain-neutral projection data that client modules consume.
/// No proto types are referenced; all fields are newtypes from subsys-coordination.
#[derive(Debug, Clone)]
pub struct DomainProjection {
    /// Projection tag (e.g., "text.mode", "text.cursor").
    pub tag: ProjectionTag,
    /// Domain that produced this projection.
    pub domain_id: DomainId,
    /// Window scope (`None` = session-wide).
    pub window_id: Option<usize>,
    /// Opaque domain content bytes.
    pub content: Vec<u8>,
    /// Human-readable display string for chrome modules.
    pub display: String,
    /// Whether this is a transient (fire-and-forget) projection.
    pub transient: bool,
    /// Monotonic version for persistent projections (0 for transient).
    pub version: u64,
    /// Client that owns this projection state.
    pub client_id: u64,
}

impl DomainProjection {
    /// Create a new domain projection directly (for testing and non-proto paths).
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn new(tag: ProjectionTag, domain_id: DomainId, content: Vec<u8>, display: String) -> Self {
        Self {
            tag,
            domain_id,
            window_id: None,
            content,
            display,
            transient: false,
            version: 0,
            client_id: 0,
        }
    }

    /// The projection tag.
    #[must_use]
    pub const fn tag(&self) -> &ProjectionTag {
        &self.tag
    }

    /// Whether this projection is for a specific window.
    #[must_use]
    pub const fn is_windowed(&self) -> bool {
        self.window_id.is_some()
    }
}
