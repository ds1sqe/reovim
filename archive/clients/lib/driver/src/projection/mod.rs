//! Driver-side compat re-exports for projection types.
//!
//! `DomainProjection` and `from_proto` are canonical in
//! `reovim-client-subsys-codec::projection`; `ProjectionDisplayCache` is
//! canonical in `reovim-client-subsys-chrome`. Re-exported here so existing
//! `reovim_client_driver::projection::*` imports keep resolving; removed
//! when the driver crate is decommissioned (TODO(#753)).

#[cfg(feature = "proto")]
mod transpiler;

// `ProjectionDisplayCache` is now canonical in `reovim-client-subsys-chrome`.
// Re-export here for backward compat until Phase F.
// TODO(#753): Remove when driver is decommissioned.
pub use reovim_client_subsys_chrome::ProjectionDisplayCache;
#[cfg(feature = "proto")]
pub use transpiler::from_proto;

// TODO(#753): Remove this compat re-export when driver is decommissioned.
// The canonical path is `reovim_client_subsys_codec::projection::DomainProjection`;
// `subsys-module` also re-exports it in its own compat layer.
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
