//! Pair state management - tracks bracket positions and matches

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::rainbow::compute_bracket_depths;

/// Information about a single bracket
#[derive(Debug, Clone, Copy)]
pub struct BracketInfo {
    /// Line number (0-indexed)
    pub line: usize,
    /// Column number (0-indexed)
    pub col: usize,
    /// Nesting depth (0 = outermost)
    pub depth: usize,
    /// The bracket character
    pub char: char,
}

/// A matched bracket pair
#[derive(Debug, Clone, Copy)]
pub struct MatchedPair {
    pub open: BracketInfo,
    pub close: BracketInfo,
}

/// Bracket state for a single buffer
#[derive(Debug, Default)]
pub struct BufferPairState {
    /// Bracket positions and depths: (line, col) -> `BracketInfo`
    pub brackets: HashMap<(usize, usize), BracketInfo>,
    /// Current matched pair (if cursor is on a bracket)
    pub matched_pair: Option<MatchedPair>,
    /// Current cursor position
    pub cursor: (usize, usize),
    /// Content hash for cache invalidation
    content_hash: u64,
}

impl BufferPairState {
    /// Check if cache is valid for given content hash
    #[must_use]
    pub const fn is_valid(&self, content_hash: u64) -> bool {
        self.content_hash == content_hash
    }

    /// Update brackets from content
    pub fn update_from_content(&mut self, content: &str, content_hash: u64) {
        self.brackets = compute_bracket_depths(content);
        self.content_hash = content_hash;
    }

    /// Get bracket at position
    #[must_use]
    pub fn get_bracket(&self, line: usize, col: usize) -> Option<&BracketInfo> {
        self.brackets.get(&(line, col))
    }
}

/// Bracket state manager for all buffers
#[derive(Debug, Default)]
pub struct PairState {
    buffers: HashMap<usize, BufferPairState>,
}

impl PairState {
    /// Create new bracket state
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffers: HashMap::new(),
        }
    }

    /// Get or create buffer state
    pub fn get_or_create(&mut self, buffer_id: usize) -> &mut BufferPairState {
        self.buffers.entry(buffer_id).or_default()
    }

    /// Get buffer state (immutable)
    #[must_use]
    pub fn get(&self, buffer_id: usize) -> Option<&BufferPairState> {
        self.buffers.get(&buffer_id)
    }

    /// Remove buffer state
    pub fn remove(&mut self, buffer_id: usize) {
        self.buffers.remove(&buffer_id);
    }

    /// Update cursor position (matched pair is computed later in render stage)
    pub fn update_cursor(&mut self, buffer_id: usize, cursor: (usize, usize)) {
        let state = self.get_or_create(buffer_id);
        state.cursor = cursor;
        // Clear matched_pair - will be recomputed in render stage after brackets are computed
        state.matched_pair = None;
    }

    /// Compute matched pair for current cursor position
    /// Called from render stage after brackets are computed
    pub fn compute_matched_pair(&mut self, buffer_id: usize) {
        if let Some(state) = self.buffers.get(&buffer_id) {
            let cursor = state.cursor;
            let matched = self.find_matched_pair(buffer_id, cursor);
            if let Some(state) = self.buffers.get_mut(&buffer_id) {
                state.matched_pair = matched;
            }
        }
    }

    /// Compute matched pair with explicit cursor position
    /// This bypasses the event-based cursor tracking and uses the cursor
    /// position directly from render data (which is always up-to-date)
    pub fn compute_matched_pair_with_cursor(&mut self, buffer_id: usize, cursor: (usize, usize)) {
        let matched = self.find_matched_pair(buffer_id, cursor);
        if let Some(state) = self.buffers.get_mut(&buffer_id) {
            state.matched_pair = matched;
        }
    }

    /// Find the innermost matched bracket pair that contains the cursor position.
    ///
    /// This finds brackets where: `open_pos` <= cursor <= `close_pos`
    /// and returns the pair with the smallest range (innermost).
    fn find_matched_pair(&self, buffer_id: usize, cursor: (usize, usize)) -> Option<MatchedPair> {
        let state = self.buffers.get(&buffer_id)?;

        // Build all matched pairs first
        let mut pairs: Vec<MatchedPair> = Vec::new();

        // Find all opening brackets and their matches
        for bracket in state.brackets.values() {
            // Only process opening brackets to avoid duplicates
            let (open_char, close_char) = match bracket.char {
                '(' => ('(', ')'),
                '[' => ('[', ']'),
                '{' => ('{', '}'),
                _ => continue, // Skip closing brackets
            };

            if bracket.char != open_char {
                continue;
            }

            // Skip unmatched brackets
            if bracket.depth == usize::MAX {
                continue;
            }

            // Find the matching closing bracket (the NEAREST one with same depth)
            // We must use min_by_key instead of find because HashMap iteration
            // order is non-deterministic, and there may be multiple closing
            // brackets at the same depth level (e.g., "(a)(b)" has two ')' at depth 0)
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

/// Thread-safe shared bracket state
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
    /// Create new shared bracket state
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(PairState::new()),
        }
    }

    /// Update cursor position
    pub fn update_cursor(&self, buffer_id: usize, cursor: (usize, usize)) {
        if let Ok(mut state) = self.inner.write() {
            state.update_cursor(buffer_id, cursor);
        }
    }

    /// Invalidate buffer cache
    pub fn invalidate_buffer(&self, buffer_id: usize) {
        if let Ok(mut state) = self.inner.write()
            && let Some(buffer_state) = state.buffers.get_mut(&buffer_id)
        {
            buffer_state.content_hash = 0; // Force recomputation
        }
    }

    /// Remove buffer state
    pub fn remove_buffer(&self, buffer_id: usize) {
        if let Ok(mut state) = self.inner.write() {
            state.remove(buffer_id);
        }
    }

    /// Execute a function with read access to the state
    pub fn with_read<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&PairState) -> R,
    {
        self.inner.read().ok().map(|guard| f(&guard))
    }

    /// Execute a function with write access to the state
    pub fn with_write<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut PairState) -> R,
    {
        self.inner.write().ok().map(|mut guard| f(&mut guard))
    }

    /// Get matched pair for a buffer
    pub fn get_matched_pair(&self, buffer_id: usize) -> Option<MatchedPair> {
        self.inner
            .read()
            .ok()
            .and_then(|state| state.get(buffer_id).and_then(|bs| bs.matched_pair))
    }

    /// Get cursor position for a buffer
    pub fn get_cursor(&self, buffer_id: usize) -> Option<(usize, usize)> {
        self.inner
            .read()
            .ok()
            .and_then(|state| state.get(buffer_id).map(|bs| bs.cursor))
    }

    /// Get all brackets for a buffer (for rainbow rendering)
    pub fn get_brackets(
        &self,
        buffer_id: usize,
    ) -> Option<Arc<HashMap<(usize, usize), BracketInfo>>> {
        self.inner
            .read()
            .ok()
            .and_then(|state| state.get(buffer_id).map(|bs| Arc::new(bs.brackets.clone())))
    }

    /// Ensure brackets are computed for buffer content
    pub fn ensure_computed(&self, buffer_id: usize, content: &str, content_hash: u64) {
        if let Ok(mut state) = self.inner.write() {
            let buffer_state = state.get_or_create(buffer_id);
            if !buffer_state.is_valid(content_hash) {
                buffer_state.update_from_content(content, content_hash);
            }
        }
    }

    /// Compute matched pair for current cursor position
    /// Must be called after `ensure_computed`
    pub fn compute_matched_pair(&self, buffer_id: usize) {
        if let Ok(mut state) = self.inner.write() {
            state.compute_matched_pair(buffer_id);
        }
    }

    /// Compute matched pair with explicit cursor position from render data
    /// This is the preferred method as it uses always-fresh cursor position
    pub fn compute_matched_pair_with_cursor(&self, buffer_id: usize, cursor: (usize, usize)) {
        if let Ok(mut state) = self.inner.write() {
            state.compute_matched_pair_with_cursor(buffer_id, cursor);
        }
    }
}
