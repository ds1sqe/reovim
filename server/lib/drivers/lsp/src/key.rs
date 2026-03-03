//! LSP provider key - typed key for LSP provider lookup.

use reovim_kernel::api::v1::ServiceKey;

/// Typed key for LSP provider lookup.
///
/// Different variants can represent different LSP provider strategies
/// (e.g., per-language servers in the future).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LspKey {
    /// Default LSP provider (single server per session).
    Default,
}

impl ServiceKey for LspKey {
    fn service_name() -> &'static str {
        "LSP"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_name() {
        assert_eq!(LspKey::service_name(), "LSP");
    }

    #[test]
    fn test_debug() {
        assert_eq!(format!("{:?}", LspKey::Default), "Default");
    }

    #[test]
    fn test_clone_copy_eq() {
        let a = LspKey::Default;
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn test_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(LspKey::Default);
        set.insert(LspKey::Default);
        assert_eq!(set.len(), 1);
    }
}
