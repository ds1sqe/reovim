//! Projection types — domain-tagged opaque payloads for client-facing state.
//!
//! The server stores, routes, and forwards projections without interpretation.
//! Domain drivers produce projections; clients consume them.

use std::sync::Arc;

/// Unique domain identifier. Assigned by `CoordinationRegistry` at driver enlistment.
///
/// Consistent with `ClientId(usize)`, `BufferId(usize)`, `WindowId(usize)`.
/// Prevents cross-domain ID misuse as a compile-time type error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DomainId(pub u32);

/// Domain-scoped projection tag. Identifies what kind of projection this is.
///
/// Convention: `"domain.kind"` — e.g. `"text.cursor"`, `"mesh.camera"`,
/// `"platform.haptic"`. The server never parses the tag.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectionTag(Arc<str>);

impl ProjectionTag {
    /// Create a new projection tag.
    #[must_use]
    pub fn new(tag: &str) -> Self {
        Self(Arc::from(tag))
    }

    /// Get the tag string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ProjectionTag {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl std::fmt::Display for ProjectionTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Delivery semantics — structural distinction between state and events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionDelivery {
    /// State snapshot. `ProjectionStore` caches, deduplicates by payload
    /// equality, serves to late-joining clients via `initial_projections()`.
    Persistent,
    /// Fire-and-forget event. `ProjectionStore` forwards immediately and
    /// discards. Not served to late-joining clients.
    Transient,
}

/// An opaque, domain-tagged, client-facing state snapshot or event.
///
/// The server stores, routes, and forwards it without interpretation.
#[derive(Debug, Clone)]
pub struct Projection {
    /// Domain-scoped tag: `"text.cursor"`, `"mesh.camera"`, `"platform.haptic"`.
    pub tag: ProjectionTag,
    /// Domain ID from `CoordinationRegistry`.
    pub domain_id: DomainId,
    /// Window scope (`None` = client-wide).
    pub window_id: Option<reovim_kernel::api::v1::WindowId>,
    /// Opaque payload bytes. Domain-specific encoding.
    pub payload: Vec<u8>,
    /// Human-readable display for status/debug. `None` when no useful string.
    pub display: Option<String>,
    /// Delivery semantics — Persistent (cached) or Transient (fire-and-forget).
    pub delivery: ProjectionDelivery,
}
