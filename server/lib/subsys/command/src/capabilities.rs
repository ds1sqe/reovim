//! Capability identifiers for abstract provides/requires matching.
//!
//! These constants are the single source of truth for capability strings
//! advertised by modules that implement this subsys's trait surface.
//! Consumers import via
//! `reovim_subsys_command::capabilities::<CONST>`.

/// Command dispatch and registration.
pub const COMMAND_DISPATCH: &str = "command-dispatch";

#[cfg(test)]
#[path = "capabilities_tests.rs"]
mod tests;
