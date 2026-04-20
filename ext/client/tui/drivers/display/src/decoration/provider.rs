//! Decoration provider traits for extensible decoration sources.
//!
//! Plugins implement `DecorationProvider` to supply decorations for their
//! specific functionality (e.g., markdown concealment, search highlighting).
//!
//! Two provider types are available:
//!
//! - `DecorationProvider`: Per-buffer provider, refreshed with buffer content
//! - `BufferDecorationSource`: Global provider that takes `buffer_id` in queries
//!
//! # Architecture
//!
//! ```text
//! Per-buffer: DecorationProvider (created by factory for each buffer)
//!   - Markdown concealment
//!   - Syntax highlighting
//!
//! Global: BufferDecorationSource (single instance, queries with buffer_id)
//!   - Rainbow brackets (SharedPairState)
//!   - Search highlighting
//! ```

use {
    super::types::{Decoration, DecorationGroup},
    reovim_kernel::api::v1::BufferId,
};

/// Trait for decoration sources.
///
/// Plugins implement this trait to provide decorations to the rendering system.
/// Each provider belongs to a priority group that determines layering order.
///
/// # Example
///
/// ```ignore
/// struct MarkdownConcealProvider {
///     conceals: Vec<Decoration>,
/// }
///
/// impl DecorationProvider for MarkdownConcealProvider {
///     fn name(&self) -> &str { "markdown-conceal" }
///     fn group(&self) -> DecorationGroup { DecorationGroup::Language }
///
///     fn decorations_for_range(&self, start: u32, end: u32) -> Vec<Decoration> {
///         self.conceals.iter()
///             .filter(|d| d.affects_line(start) || d.affects_line(end))
///             .cloned()
///             .collect()
///     }
///
///     fn refresh(&mut self, content: &str) {
///         self.conceals = parse_markdown_links(content);
///     }
/// }
/// ```
pub trait DecorationProvider: Send + Sync {
    /// Provider name for debugging and logging.
    fn name(&self) -> &'static str;

    /// Priority group for layering.
    ///
    /// Decorations from higher-priority groups override those from lower groups.
    fn group(&self) -> DecorationGroup;

    /// Get decorations for a line range.
    ///
    /// Returns all decorations that affect lines in `[start_line, end_line]` (inclusive).
    fn decorations_for_range(&self, start_line: u32, end_line: u32) -> Vec<Decoration>;

    /// Refresh decorations after content changes.
    ///
    /// Called by the saturator when buffer content changes.
    /// The provider should re-parse the content and update its internal state.
    fn refresh(&mut self, content: &str);

    /// Check if the provider has valid state.
    ///
    /// Returns `false` if the provider needs to be refreshed before use.
    fn is_valid(&self) -> bool {
        true
    }
}

/// Factory for creating decoration providers.
///
/// Registered factories are consulted when a buffer is opened to determine
/// which providers should be attached based on the file type.
pub trait DecorationProviderFactory: Send + Sync {
    /// Factory name for debugging.
    fn name(&self) -> &str;

    /// Create a provider for the given language ID.
    ///
    /// Returns `None` if this factory doesn't handle the language.
    fn create(&self, language_id: &str) -> Option<Box<dyn DecorationProvider>>;

    /// List of language IDs this factory supports.
    fn supported_languages(&self) -> &[&str] {
        &[]
    }
}

// ============================================================================
// Buffer Decoration Source (global providers with buffer_id awareness)
// ============================================================================

/// Trait for global decoration sources that manage state across all buffers.
///
/// Unlike `DecorationProvider` which is per-buffer, `BufferDecorationSource`
/// maintains global state and accepts `buffer_id` in queries. This is suitable
/// for features like rainbow brackets where a single service tracks all buffers.
///
/// # Example
///
/// ```ignore
/// // Pair module implements this trait
/// impl BufferDecorationSource for SharedPairState {
///     fn name(&self) -> &'static str { "rainbow-brackets" }
///     fn group(&self) -> DecorationGroup { DecorationGroup::Syntax }
///
///     fn decorations_for_buffer(
///         &self,
///         buffer_id: BufferId,
///         content: &str,
///         cursor: (usize, usize),
///     ) -> Vec<Decoration> {
///         // Generate rainbow bracket decorations for this buffer
///     }
/// }
/// ```
///
/// # Architecture
///
/// This follows mechanism/policy separation:
/// - **Mechanism** (this driver): Trait definition
/// - **Policy** (modules): Implementations like `SharedPairState`
pub trait BufferDecorationSource: Send + Sync {
    /// Source name for debugging and logging.
    fn name(&self) -> &'static str;

    /// Priority group for layering.
    ///
    /// Decorations from higher-priority groups override those from lower groups.
    fn group(&self) -> DecorationGroup;

    /// Get decorations for a specific buffer.
    ///
    /// # Arguments
    ///
    /// * `buffer_id` - The buffer to get decorations for
    /// * `content` - Current buffer content (for computing bracket positions)
    /// * `cursor` - Current cursor position (line, col) for matched pair highlighting
    ///
    /// # Returns
    ///
    /// Vector of decorations for the buffer
    fn decorations_for_buffer(
        &self,
        buffer_id: BufferId,
        content: &str,
        cursor: (usize, usize),
    ) -> Vec<Decoration>;
}

#[cfg(test)]
#[path = "provider_tests.rs"]
mod tests;
