//! Capability identifiers for abstract provides/requires matching.
//!
//! These constants are the single source of truth for capability strings
//! advertised by modules that implement this subsys's trait surface.
//! Consumers import via
//! `reovim_subsys_vfs::capabilities::<CONST>`.

/// Virtual filesystem abstraction.
pub const VFS_PROVIDER: &str = "vfs-provider";

#[cfg(test)]
#[path = "capabilities_tests.rs"]
mod tests;
