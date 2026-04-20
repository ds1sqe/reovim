//! LSP provider key - typed key for LSP provider lookup.

use reovim_kernel::api::v1::ServiceKey;

/// Typed key for LSP provider lookup.
///
/// Different variants can represent different LSP provider strategies.
/// Use [`Language`](LspKey::Language) for per-language servers (e.g.,
/// rust-analyzer for Rust, pylsp for Python).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LspKey {
    /// Default LSP provider (single server per session).
    Default,
    /// Per-language LSP provider keyed by language ID (e.g., "rust", "python").
    Language(String),
}

impl ServiceKey for LspKey {
    fn service_name() -> &'static str {
        "LSP"
    }
}

#[cfg(test)]
#[path = "key_tests.rs"]
mod tests;
