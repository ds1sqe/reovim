//! Capability identifiers for abstract provides/requires matching.
//!
//! These constants are the single source of truth for capability strings
//! advertised by modules that implement this subsys's trait surface.
//! Consumers import via
//! `reovim_subsys_clipboard::capabilities::<CONST>`.

/// System clipboard integration.
pub const CLIPBOARD_PROVIDER: &str = "clipboard-provider";

#[cfg(test)]
#[path = "capabilities_tests.rs"]
mod tests;
