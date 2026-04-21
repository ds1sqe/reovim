//! Capability identifiers for abstract provides/requires matching.
//!
//! These constants are the single source of truth for capability strings
//! advertised by modules that implement this subsys's trait surface.
//! Consumers import via
//! `reovim_subsys_content_codec::capabilities::<CONST>`.

/// Content codec (encoding/decoding file content).
pub const CODEC_PROVIDER: &str = "codec-provider";

#[cfg(test)]
#[path = "capabilities_tests.rs"]
mod tests;
