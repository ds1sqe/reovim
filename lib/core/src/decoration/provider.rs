//! Abstract decoration provider traits
//!
//! This module defines traits for buffer-centric decoration providers.
//! Language plugins implement these traits to provide visual decorations
//! like conceals, backgrounds, and inline styles.
//!
//! ## Architecture
//!
//! Similar to `SyntaxProvider`, these traits allow core to remain independent
//! of specific implementations (tree-sitter, LSP, etc.):
//!
//! - **`DecorationProvider`**: Owned by buffer, provides decorations on demand
//! - **`DecorationFactory`**: Creates providers for specific file types

use std::sync::Arc;

use super::types::Decoration;

/// Abstract decoration provider for buffer rendering
///
/// Implementations generate visual decorations like conceals (e.g., markdown
/// heading markers → icons), backgrounds (e.g., code block highlighting),
/// and inline styles (e.g., emphasis formatting).
///
/// Buffers own their decoration provider, similar to `SyntaxProvider`.
pub trait DecorationProvider: Send + Sync {
    /// Language ID this decorator handles
    fn language_id(&self) -> &str;

    /// Get decorations for a line range
    ///
    /// Called during rendering to get decorations for visible lines.
    /// Returns decorations with full span information.
    fn decoration_range(&self, content: &str, start_line: u32, end_line: u32) -> Vec<Decoration>;

    /// Refresh decorations after content change
    ///
    /// Called when buffer content changes. Implementations should re-parse
    /// and update cached state as needed.
    fn refresh(&mut self, content: &str);

    /// Check if decorations are currently valid
    ///
    /// Returns false if `refresh()` needs to be called.
    fn is_valid(&self) -> bool;
}

/// Factory for creating decoration providers
///
/// Registered with `PluginStateRegistry`, called by runtime when opening files.
pub trait DecorationFactory: Send + Sync {
    /// Create a decoration provider for the given file
    ///
    /// Returns None if this factory doesn't support the file type.
    fn create_provider(
        &self,
        file_path: &str,
        content: &str,
    ) -> Option<Box<dyn DecorationProvider>>;

    /// Check if this factory supports the given file
    fn supports_file(&self, file_path: &str) -> bool;
}

/// Shared reference to a decoration factory
pub type SharedDecorationFactory = Arc<dyn DecorationFactory>;
