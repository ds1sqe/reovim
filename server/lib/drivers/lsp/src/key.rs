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
mod tests {
    use super::*;

    #[test]
    fn test_service_name() {
        assert_eq!(LspKey::service_name(), "LSP");
    }

    #[test]
    fn test_debug_default() {
        assert_eq!(format!("{:?}", LspKey::Default), "Default");
    }

    #[test]
    fn test_debug_language() {
        let key = LspKey::Language("rust".to_owned());
        let debug = format!("{key:?}");
        assert!(debug.contains("Language"));
        assert!(debug.contains("rust"));
    }

    #[test]
    fn test_clone_eq() {
        let a = LspKey::Default;
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn test_language_clone_eq() {
        let a = LspKey::Language("python".to_owned());
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn test_language_ne_default() {
        let a = LspKey::Default;
        let b = LspKey::Language("rust".to_owned());
        assert_ne!(a, b);
    }

    #[test]
    fn test_different_languages_ne() {
        let a = LspKey::Language("rust".to_owned());
        let b = LspKey::Language("python".to_owned());
        assert_ne!(a, b);
    }

    #[test]
    fn test_hash_default() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(LspKey::Default);
        set.insert(LspKey::Default);
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_hash_language() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(LspKey::Language("rust".to_owned()));
        set.insert(LspKey::Language("rust".to_owned()));
        set.insert(LspKey::Language("python".to_owned()));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_hash_mixed() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(LspKey::Default);
        set.insert(LspKey::Language("rust".to_owned()));
        assert_eq!(set.len(), 2);
    }
}
