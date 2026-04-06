//! Buffer capability flags — re-exported from provider-text.
//!
//! `BufferCapabilities` is now defined in `reovim-provider-text` as part of
//! the `BufferOps` contract. This module re-exports it for backward
//! compatibility with existing driver/module consumers.

pub use reovim_provider_text::BufferCapabilities;

#[cfg(test)]
#[path = "capabilities_tests.rs"]
mod tests;
