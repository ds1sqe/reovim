//! Domain projection types and proto boundary transpilation.
//!
//! `DomainProjection` moved to `reovim-client-subsys-module::types` in Phase B
//! (#753). This module re-exports it for backward compatibility and owns the
//! proto-specific transpilation layer (`TranspileError`, `from_proto`).

mod cache;
#[cfg(feature = "proto")]
mod transpiler;

pub use cache::ProjectionDisplayCache;
#[cfg(feature = "proto")]
pub use transpiler::from_proto;

// `DomainProjection` is now canonical in `reovim-client-subsys-module::types`.
// Re-export here so callers that use `reovim_client_driver::projection::DomainProjection`
// keep compiling without path changes until Phase F.
// TODO(#753): Remove when driver is decommissioned.
pub use reovim_client_subsys_module::types::DomainProjection;

// Keep `TranspileError` driver-local: it is proto-transpilation machinery that
// belongs to the driver boundary, not the subsys ABI.

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

#[cfg(test)]
mod tests;
