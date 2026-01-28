//! Pair state management - tracks bracket positions and matches.
//!
//! This module provides per-buffer bracket tracking with:
//! - Bracket position and depth information
//! - Matched pair detection for cursor highlighting
//! - Content hash-based cache invalidation
//!
//! # Thread Safety
//!
//! `SharedPairState` provides thread-safe access via `RwLock` and can be
//! registered in `ServiceRegistry` for cross-module access.

use std::collections::HashMap;

use {reovim_arch::sync::RwLock, reovim_kernel::api::v1::BufferId};

use crate::rainbow::compute_bracket_depths;

/// Information about a single bracket.
#[derive(Debug, Clone, Copy)]
pub struct BracketInfo {
    /// Line number (0-indexed).
    pub line: usize,
    /// Column number (0-indexed).
    pub col: usize,
    /// Nesting depth (0 = outermost, `usize::MAX` = unmatched).
    pub depth: usize,
    /// The bracket character.
    pub char: char,
}

/// A matched bracket pair.
#[derive(Debug, Clone, Copy)]
pub struct MatchedPair {
    /// The opening bracket.
    pub open: BracketInfo,
    /// The closing bracket.
    pub close: BracketInfo,
}

/// Bracket state for a single buffer.
#[derive(Debug, Default)]
pub struct BufferPairState {
    /// Bracket positions and depths: (line, col) -> `BracketInfo`.
    pub brackets: HashMap<(usize, usize), BracketInfo>,
    /// Current matched pair (if cursor is on/between brackets).
    pub matched_pair: Option<MatchedPair>,
    /// Current cursor position.
    pub cursor: (usize, usize),
    /// Content hash for cache invalidation.
    content_hash: u64,
}

impl BufferPairState {
    /// Check if cache is valid for given content hash.
    #[must_use]
    pub const fn is_valid(&self, content_hash: u64) -> bool {
        self.content_hash == content_hash
    }

    /// Update brackets from content.
    pub fn update_from_content(&mut self, content: &str, content_hash: u64) {
        self.brackets = compute_bracket_depths(content);
        self.content_hash = content_hash;
    }

    /// Get bracket at position.
    #[must_use]
    pub fn get_bracket(&self, line: usize, col: usize) -> Option<&BracketInfo> {
        self.brackets.get(&(line, col))
    }
}

/// Bracket state manager for all buffers.
#[derive(Debug, Default)]
pub struct PairState {
    buffers: HashMap<usize, BufferPairState>,
}

impl PairState {
    /// Create new bracket state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffers: HashMap::new(),
        }
    }

    /// Get or create buffer state.
    pub fn get_or_create(&mut self, buffer_id: BufferId) -> &mut BufferPairState {
        self.buffers.entry(buffer_id.as_usize()).or_default()
    }

    /// Get buffer state (immutable).
    #[must_use]
    pub fn get(&self, buffer_id: BufferId) -> Option<&BufferPairState> {
        self.buffers.get(&buffer_id.as_usize())
    }

    /// Remove buffer state.
    pub fn remove(&mut self, buffer_id: BufferId) {
        self.buffers.remove(&buffer_id.as_usize());
    }

    /// Update cursor position.
    pub fn update_cursor(&mut self, buffer_id: BufferId, cursor: (usize, usize)) {
        let state = self.get_or_create(buffer_id);
        state.cursor = cursor;
        // Clear matched_pair - will be recomputed when needed
        state.matched_pair = None;
    }

    /// Compute matched pair for current cursor position.
    pub fn compute_matched_pair(&mut self, buffer_id: BufferId) {
        let cursor = self.get(buffer_id).map(|s| s.cursor);
        if let Some(cursor) = cursor {
            let matched = self.find_matched_pair(buffer_id, cursor);
            if let Some(state) = self.buffers.get_mut(&buffer_id.as_usize()) {
                state.matched_pair = matched;
            }
        }
    }

    /// Compute matched pair with explicit cursor position.
    pub fn compute_matched_pair_with_cursor(
        &mut self,
        buffer_id: BufferId,
        cursor: (usize, usize),
    ) {
        let matched = self.find_matched_pair(buffer_id, cursor);
        if let Some(state) = self.buffers.get_mut(&buffer_id.as_usize()) {
            state.matched_pair = matched;
        }
    }

    /// Find the innermost matched bracket pair that contains the cursor position.
    ///
    /// This finds brackets where: `open_pos` <= cursor <= `close_pos`
    /// and returns the pair with the smallest range (innermost).
    fn find_matched_pair(
        &self,
        buffer_id: BufferId,
        cursor: (usize, usize),
    ) -> Option<MatchedPair> {
        let state = self.buffers.get(&buffer_id.as_usize())?;

        // Build all matched pairs first
        let mut pairs: Vec<MatchedPair> = Vec::new();

        // Find all opening brackets and their matches
        for bracket in state.brackets.values() {
            // Only process opening brackets to avoid duplicates
            let close_char = match bracket.char {
                '(' => ')',
                '[' => ']',
                '{' => '}',
                _ => continue, // Skip closing brackets
            };

            // Skip unmatched brackets
            if bracket.depth == usize::MAX {
                continue;
            }

            // Find the matching closing bracket (the NEAREST one with same depth)
            let matching = state
                .brackets
                .values()
                .filter(|b| {
                    b.depth == bracket.depth
                        && b.char == close_char
                        && (b.line, b.col) > (bracket.line, bracket.col)
                })
                .min_by_key(|b| (b.line, b.col));

            if let Some(close) = matching {
                pairs.push(MatchedPair {
                    open: *bracket,
                    close: *close,
                });
            }
        }

        // Find the innermost pair that contains the cursor
        pairs
            .into_iter()
            .filter(|pair| {
                let open_pos = (pair.open.line, pair.open.col);
                let close_pos = (pair.close.line, pair.close.col);
                cursor >= open_pos && cursor <= close_pos
            })
            .min_by(|a, b| {
                // Compare by range size: line difference first, then column difference
                let a_range = (
                    a.close.line.saturating_sub(a.open.line),
                    a.close.col.saturating_sub(a.open.col),
                );
                let b_range = (
                    b.close.line.saturating_sub(b.open.line),
                    b.close.col.saturating_sub(b.open.col),
                );
                a_range.cmp(&b_range)
            })
    }
}

/// Thread-safe shared bracket state.
///
/// This can be registered in `ServiceRegistry` for cross-module access.
///
/// # Example
///
/// ```ignore
/// use reovim_module_pair::state::SharedPairState;
///
/// let state = SharedPairState::new();
///
/// // Update cursor
/// state.update_cursor(buffer_id, (10, 5));
///
/// // Ensure brackets are computed
/// state.ensure_computed(buffer_id, &content, hash);
///
/// // Get matched pair
/// let pair = state.get_matched_pair(buffer_id);
/// ```
pub struct SharedPairState {
    inner: RwLock<PairState>,
}

impl std::fmt::Debug for SharedPairState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedPairState").finish()
    }
}

impl Default for SharedPairState {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedPairState {
    /// Create new shared bracket state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(PairState::new()),
        }
    }

    /// Update cursor position.
    pub fn update_cursor(&self, buffer_id: BufferId, cursor: (usize, usize)) {
        self.inner.write().update_cursor(buffer_id, cursor);
    }

    /// Invalidate buffer cache.
    pub fn invalidate_buffer(&self, buffer_id: BufferId) {
        let mut state = self.inner.write();
        if let Some(buffer_state) = state.buffers.get_mut(&buffer_id.as_usize()) {
            buffer_state.content_hash = 0; // Force recomputation
        }
    }

    /// Remove buffer state.
    pub fn remove_buffer(&self, buffer_id: BufferId) {
        self.inner.write().remove(buffer_id);
    }

    /// Execute a function with read access to the state.
    pub fn with_read<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&PairState) -> R,
    {
        f(&self.inner.read())
    }

    /// Execute a function with write access to the state.
    pub fn with_write<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut PairState) -> R,
    {
        f(&mut self.inner.write())
    }

    /// Get matched pair for a buffer.
    #[must_use]
    pub fn get_matched_pair(&self, buffer_id: BufferId) -> Option<MatchedPair> {
        self.inner
            .read()
            .get(buffer_id)
            .and_then(|bs| bs.matched_pair)
    }

    /// Get cursor position for a buffer.
    #[must_use]
    pub fn get_cursor(&self, buffer_id: BufferId) -> Option<(usize, usize)> {
        self.inner.read().get(buffer_id).map(|bs| bs.cursor)
    }

    /// Get all brackets for a buffer (for rainbow rendering).
    #[must_use]
    pub fn get_brackets(
        &self,
        buffer_id: BufferId,
    ) -> Option<HashMap<(usize, usize), BracketInfo>> {
        self.inner
            .read()
            .get(buffer_id)
            .map(|bs| bs.brackets.clone())
    }

    /// Ensure brackets are computed for buffer content.
    #[allow(clippy::significant_drop_tightening)]
    pub fn ensure_computed(&self, buffer_id: BufferId, content: &str, content_hash: u64) {
        let mut state = self.inner.write();
        let buffer_state = state.get_or_create(buffer_id);
        if !buffer_state.is_valid(content_hash) {
            buffer_state.update_from_content(content, content_hash);
        }
    }

    /// Compute matched pair for current cursor position.
    /// Must be called after `ensure_computed`.
    pub fn compute_matched_pair(&self, buffer_id: BufferId) {
        self.inner.write().compute_matched_pair(buffer_id);
    }

    /// Compute matched pair with explicit cursor position.
    pub fn compute_matched_pair_with_cursor(&self, buffer_id: BufferId, cursor: (usize, usize)) {
        self.inner
            .write()
            .compute_matched_pair_with_cursor(buffer_id, cursor);
    }
}

// Implement Service marker trait for ServiceRegistry integration
impl reovim_kernel::api::v1::Service for SharedPairState {}

// Implement BufferDecorationSource for generic decoration registry integration (#440)
impl reovim_driver_display::BufferDecorationSource for SharedPairState {
    fn name(&self) -> &'static str {
        "rainbow-brackets"
    }

    fn group(&self) -> reovim_driver_display::DecorationGroup {
        reovim_driver_display::DecorationGroup::Syntax
    }

    fn decorations_for_buffer(
        &self,
        buffer_id: BufferId,
        content: &str,
        cursor: (usize, usize),
    ) -> Vec<reovim_driver_display::Decoration> {
        // Compute content hash for cache
        let content_hash = crate::rainbow::content_hash(content);

        // Ensure brackets are computed for current content
        self.ensure_computed(buffer_id, content, content_hash);

        // Compute matched pair for cursor position
        self.compute_matched_pair_with_cursor(buffer_id, cursor);

        // Generate decorations
        crate::decorations::generate_decorations_for_buffer(self, buffer_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_pair_state_creation() {
        let state = BufferPairState::default();
        assert!(state.brackets.is_empty());
        assert!(state.matched_pair.is_none());
        assert_eq!(state.cursor, (0, 0));
    }

    #[test]
    fn test_buffer_pair_state_update() {
        let mut state = BufferPairState::default();
        let content = "(a + b)";
        let hash = crate::rainbow::content_hash(content);

        state.update_from_content(content, hash);

        assert_eq!(state.brackets.len(), 2);
        assert!(state.is_valid(hash));
        assert!(!state.is_valid(hash + 1));
    }

    #[test]
    fn test_pair_state_buffer_isolation() {
        let mut state = PairState::new();
        let buf1 = BufferId::from_raw(1);
        let buf2 = BufferId::from_raw(2);

        state.update_cursor(buf1, (10, 5));
        state.update_cursor(buf2, (20, 10));

        assert_eq!(state.get(buf1).unwrap().cursor, (10, 5));
        assert_eq!(state.get(buf2).unwrap().cursor, (20, 10));
    }

    #[test]
    fn test_pair_state_remove_buffer() {
        let mut state = PairState::new();
        let buf = BufferId::from_raw(1);

        state.update_cursor(buf, (10, 5));
        assert!(state.get(buf).is_some());

        state.remove(buf);
        assert!(state.get(buf).is_none());
    }

    #[test]
    fn test_shared_pair_state_thread_safety() {
        let state = SharedPairState::new();
        let buf = BufferId::from_raw(1);

        state.update_cursor(buf, (10, 5));
        assert_eq!(state.get_cursor(buf), Some((10, 5)));

        state.invalidate_buffer(buf);
        state.remove_buffer(buf);
        assert_eq!(state.get_cursor(buf), None);
    }

    #[test]
    fn test_matched_pair_finding() {
        let mut state = PairState::new();
        let buf = BufferId::from_raw(1);
        let content = "(a + b)";
        let hash = crate::rainbow::content_hash(content);

        let buffer_state = state.get_or_create(buf);
        buffer_state.update_from_content(content, hash);
        buffer_state.cursor = (0, 3); // Cursor on 'a'

        state.compute_matched_pair(buf);

        let matched = state.get(buf).unwrap().matched_pair;
        assert!(matched.is_some());
        let pair = matched.unwrap();
        assert_eq!(pair.open.col, 0);
        assert_eq!(pair.close.col, 6);
    }

    #[test]
    fn test_innermost_matched_pair() {
        let mut state = PairState::new();
        let buf = BufferId::from_raw(1);
        let content = "((a))";
        let hash = crate::rainbow::content_hash(content);

        let buffer_state = state.get_or_create(buf);
        buffer_state.update_from_content(content, hash);
        buffer_state.cursor = (0, 2); // Cursor on 'a'

        state.compute_matched_pair(buf);

        let matched = state.get(buf).unwrap().matched_pair;
        assert!(matched.is_some());
        let pair = matched.unwrap();
        // Should get the inner pair, not the outer
        assert_eq!(pair.open.col, 1);
        assert_eq!(pair.close.col, 3);
    }
}
