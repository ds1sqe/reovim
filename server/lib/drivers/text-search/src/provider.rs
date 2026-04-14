//! Search provider trait.

use {
    super::{
        LineSource,
        types::{Direction, SearchError, SearchMatch},
    },
    reovim_domain_text::Position,
    reovim_provider_text::Buffer,
};

/// Search provider interface for finding patterns in buffers.
///
/// # Design Philosophy
///
/// - **Stateless**: All methods take buffer and cursor as parameters
/// - **Pure search**: No highlighting or side effects (that's display driver's job)
/// - **Vim-compatible**: Supports forward/backward search with wrapping
///
/// # Example
///
/// ```ignore
/// use reovim_driver_text_search::{SearchProvider, Direction};
///
/// // Find next occurrence of "hello"
/// let result = provider.find_next(&buffer, cursor, "hello", Direction::Forward, true)?;
/// if let Some(m) = result {
///     cursor = m.start;
/// }
/// ```
pub trait SearchProvider: Send + Sync {
    /// Find next match from cursor position.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The buffer to search in
    /// * `cursor` - Current cursor position
    /// * `pattern` - Pattern to search for (regex or literal depending on implementation)
    /// * `direction` - Search direction (forward or backward)
    /// * `wrap` - Whether to wrap around buffer boundaries
    ///
    /// # Returns
    ///
    /// * `Ok(Some(match))` - Match found
    /// * `Ok(None)` - No match found
    /// * `Err(e)` - Invalid pattern
    ///
    /// # Errors
    ///
    /// Returns `SearchError::InvalidPattern` if the pattern is invalid.
    fn find_next(
        &self,
        buffer: &Buffer,
        cursor: Position,
        pattern: &str,
        direction: Direction,
        wrap: bool,
    ) -> Result<Option<SearchMatch>, SearchError>;

    /// Find all matches in buffer (for highlighting).
    ///
    /// # Errors
    ///
    /// Returns `SearchError::InvalidPattern` if the pattern is invalid.
    fn find_all(&self, buffer: &Buffer, pattern: &str) -> Result<Vec<SearchMatch>, SearchError>;

    /// Get word under cursor for * and # commands.
    ///
    /// Returns the word as a search pattern (implementation may add word boundaries).
    fn word_at_cursor(&self, buffer: &Buffer, cursor: Position) -> Option<String>;

    // === LineSource-based methods (buffer-type-agnostic) ===

    /// Find next match using a generic `LineSource`.
    ///
    /// This generalizes `find_next` to work with both `Buffer` and
    /// `VirtualBuffer` via the [`LineSource`] trait.
    ///
    /// # Errors
    ///
    /// Returns `SearchError::InvalidPattern` if the pattern is invalid.
    fn find_next_source(
        &self,
        source: &dyn LineSource,
        cursor: Position,
        pattern: &str,
        direction: Direction,
        wrap: bool,
    ) -> Result<Option<SearchMatch>, SearchError>;

    /// Find all matches using a generic `LineSource`.
    ///
    /// # Errors
    ///
    /// Returns `SearchError::InvalidPattern` if the pattern is invalid.
    fn find_all_source(
        &self,
        source: &dyn LineSource,
        pattern: &str,
    ) -> Result<Vec<SearchMatch>, SearchError>;

    /// Get word under cursor using a generic `LineSource`.
    fn word_at_cursor_source(&self, source: &dyn LineSource, cursor: Position) -> Option<String>;
}

#[cfg(test)]
#[path = "provider_tests.rs"]
mod tests;
