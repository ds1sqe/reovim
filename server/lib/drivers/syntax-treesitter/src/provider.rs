//! Imperative decoration provider trait.
//!
//! Language modules implement [`DecorationProvider`] when declarative
//! [`DecorationRule`](reovim_driver_text_syntax::DecorationRule) mapping is
//! insufficient — for example, table rendering requires cross-row column
//! width analysis that cannot be expressed as a simple capture-name-to-kind
//! mapping.
//!
//! Providers are registered via
//! [`TreeSitterDriverBuilder::decoration_provider()`](crate::TreeSitterDriverBuilder::decoration_provider)
//! and called during [`SyntaxDriver::decorations()`](reovim_driver_text_syntax::SyntaxDriver::decorations)
//! with read access to the parsed tree and buffer content.

use std::ops::Range;

use {reovim_driver_text_syntax::Annotation, tree_sitter::Tree};

/// Imperative decoration provider for complex decoration logic.
///
/// Language modules implement this trait when declarative `DecorationRule`
/// mapping is insufficient (e.g., table rendering requires cross-row
/// column width analysis).
///
/// Providers are called during `TreeSitterDriver::decorations()` with
/// read access to the parsed tree and buffer content.
///
/// # Caching
///
/// Implementations SHOULD cache results keyed by content hash to avoid
/// redundant analysis on every `decorations()` call. The recommended
/// pattern is content-hash caching: hash the relevant source text and
/// return cached annotations when the hash matches.
///
/// # Thread Safety
///
/// Providers must be `Send + Sync`. Interior mutability (e.g., `RwLock`
/// for cache) is the standard approach.
pub trait DecorationProvider: Send + Sync {
    /// Generate decoration annotations from the parsed tree.
    ///
    /// Called on every `decorations()` invocation with:
    /// - `tree`: the current parse tree (read-only)
    /// - `content`: the full buffer content
    /// - `byte_range`: the visible byte range to generate decorations for
    ///
    /// Implementations should filter to nodes within `byte_range` and
    /// return only annotations that overlap it.
    fn decorations(&self, tree: &Tree, content: &str, byte_range: Range<usize>) -> Vec<Annotation>;
}

#[cfg(test)]
#[path = "provider_tests.rs"]
mod tests;
