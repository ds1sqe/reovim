//! Domain-neutral register types.
//!
//! `RegisterName(Arc<str>)` — no char assumption. Vim modules construct
//! single-char names at the policy layer. Mechanism is fully neutral.

use std::sync::Arc;

use super::projection::DomainId;

/// Domain-neutral register name. Not `char` — that would leak vim's
/// single-character convention into the mechanism layer.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RegisterName(Arc<str>);

impl RegisterName {
    /// Create a new register name.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self(Arc::from(name))
    }

    /// Get the name string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for RegisterName {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl std::fmt::Display for RegisterName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Scoped register key — global (cross-domain) or domain-local.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RegisterKey {
    /// Session-global: unnamed, clipboard, numbered.
    /// Shared across all domains.
    Global(RegisterName),
    /// Domain-local: named registers scoped to a domain.
    /// Text register "a" and mesh register "a" are separate entries.
    DomainLocal(DomainId, RegisterName),
}
