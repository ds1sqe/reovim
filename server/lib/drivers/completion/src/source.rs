//! Completion source trait.
//!
//! Defines the pluggable interface for completion item providers.
//! Sources produce items for a given context; the engine collects
//! from all registered sources, applies fuzzy filtering, and merges.

use crate::{CompletionContext, CompletionItem};

/// Trait for pluggable completion sources.
///
/// Sources produce items for a given context. The engine collects from
/// all registered sources, applies fuzzy filtering, and merges results.
///
/// # Sync Design
///
/// `complete()` is synchronous. For async sources (e.g., LSP), the module
/// pre-fetches and caches results; the source returns cached items.
///
/// # Priority
///
/// Higher priority sources rank first at equal match scores.
/// Convention: buffer-words=100, LSP=200, snippets=150.
pub trait CompletionSource: Send + Sync {
    /// Unique identifier (e.g., "buffer-words", "lsp", "snippets").
    fn id(&self) -> &'static str;

    /// Source priority (higher = preferred at equal match scores).
    fn priority(&self) -> u16;

    /// Whether this source is currently available for the given context.
    fn is_available(&self, ctx: &CompletionContext) -> bool;

    /// Produce completion items for the given context.
    fn complete(&self, ctx: &CompletionContext) -> Vec<CompletionItem>;
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;
