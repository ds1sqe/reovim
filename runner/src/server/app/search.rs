//! Search state for / and ? commands.
//!
//! Tracks search patterns, direction, highlighting, and input mode
//! for Vim-style incremental search.

// ============================================================================
// Search Infrastructure
// ============================================================================

/// Search direction for / and ? commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchDirection {
    /// Search forward from cursor (/)
    #[default]
    Forward,
    /// Search backward from cursor (?)
    Backward,
}

/// Search state for the session.
///
/// Tracks the current search pattern, direction, highlighting status,
/// and input mode for pattern entry.
#[derive(Debug, Clone, Default)]
pub struct SearchState {
    /// Last search pattern (persists across buffers like Vim).
    pub pattern: Option<String>,
    /// Last search direction.
    pub direction: SearchDirection,
    /// Whether search highlighting is active.
    pub highlight_active: bool,
    /// Input buffer when in search mode (typing pattern).
    pub input_buffer: String,
    /// Whether we're in search input mode.
    pub input_active: bool,
    /// Direction for current input mode (forward for /, backward for ?).
    pub input_direction: SearchDirection,
}

impl SearchState {
    /// Create a new search state with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Start search input mode.
    pub fn start_input(&mut self, direction: SearchDirection) {
        self.input_active = true;
        self.input_direction = direction;
        self.input_buffer.clear();
    }

    /// Cancel search input mode.
    pub fn cancel_input(&mut self) {
        self.input_active = false;
        self.input_buffer.clear();
    }

    /// Complete search input and return (pattern, direction).
    ///
    /// If input is empty, returns the last pattern (Vim behavior).
    /// Returns None if no pattern available.
    pub fn complete_input(&mut self) -> Option<(String, SearchDirection)> {
        if !self.input_active {
            return None;
        }

        self.input_active = false;

        if self.input_buffer.is_empty() {
            // Empty input uses last pattern
            return self
                .pattern
                .as_ref()
                .map(|p| (p.clone(), self.input_direction));
        }

        let pattern = std::mem::take(&mut self.input_buffer);
        let direction = self.input_direction;

        // Store for future use
        self.pattern = Some(pattern.clone());
        self.direction = direction;
        self.highlight_active = true;

        Some((pattern, direction))
    }

    /// Clear search highlighting.
    pub const fn clear_highlight(&mut self) {
        self.highlight_active = false;
    }
}
