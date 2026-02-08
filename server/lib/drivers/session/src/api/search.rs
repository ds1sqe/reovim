//! Search state extension for session.
//!
//! Provides shared search state that both the runner and modules can access.
//! This stores the last search pattern and direction for n/N repeat.

use {crate::SessionExtension, reovim_driver_search::Direction};

/// Per-session search state.
///
/// Stores the last search pattern and direction for n, N, *, # commands.
/// Also tracks pending search input when user presses `/` or `?`.
#[derive(Debug, Default)]
pub struct SearchState {
    /// Last search pattern (regex format).
    pub last_pattern: Option<String>,

    /// Last search direction (forward or backward).
    pub last_direction: Direction,

    /// Pending search direction (set when entering search input mode).
    ///
    /// When Some, indicates we're in search input mode and which direction
    /// the search will go when Enter is pressed.
    pub pending_search: Option<Direction>,
}

impl SessionExtension for SearchState {
    fn create() -> Self {
        Self::default()
    }
}

impl SearchState {
    /// Set the search pattern and direction.
    pub fn set(&mut self, pattern: String, direction: Direction) {
        self.last_pattern = Some(pattern);
        self.last_direction = direction;
    }

    /// Get the pattern for repeat search (n).
    #[must_use]
    pub fn pattern_for_repeat(&self) -> Option<&str> {
        self.last_pattern.as_deref()
    }

    /// Get the direction for repeat search (n).
    #[must_use]
    pub const fn direction_for_repeat(&self) -> Direction {
        self.last_direction
    }

    /// Get the reversed direction for opposite repeat (N).
    #[must_use]
    pub const fn direction_for_opposite(&self) -> Direction {
        match self.last_direction {
            Direction::Forward => Direction::Backward,
            Direction::Backward => Direction::Forward,
        }
    }

    /// Start pending search (called when `/` or `?` is pressed).
    pub const fn start_pending_search(&mut self, direction: Direction) {
        self.pending_search = Some(direction);
    }

    /// Take the pending search direction (clears it).
    ///
    /// Called when command-line mode exits to get the search direction.
    pub const fn take_pending_search(&mut self) -> Option<Direction> {
        // Manual implementation since Option::take() is not const
        let result = self.pending_search;
        self.pending_search = None;
        result
    }

    /// Check if a search is pending.
    #[must_use]
    pub const fn has_pending_search(&self) -> bool {
        self.pending_search.is_some()
    }

    /// Clear pending search (called on cancel/escape).
    pub const fn clear_pending_search(&mut self) {
        self.pending_search = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_state_default() {
        let state = SearchState::default();
        assert!(state.last_pattern.is_none());
        assert!(matches!(state.last_direction, Direction::Forward));
    }

    #[test]
    fn test_search_state_set() {
        let mut state = SearchState::default();
        state.set("hello".to_string(), Direction::Backward);
        assert_eq!(state.pattern_for_repeat(), Some("hello"));
        assert!(matches!(state.direction_for_repeat(), Direction::Backward));
        assert!(matches!(state.direction_for_opposite(), Direction::Forward));
    }

    #[test]
    fn test_pending_search() {
        let mut state = SearchState::default();
        assert!(!state.has_pending_search());

        state.start_pending_search(Direction::Forward);
        assert!(state.has_pending_search());

        let dir = state.take_pending_search();
        assert_eq!(dir, Some(Direction::Forward));
        assert!(!state.has_pending_search());
    }

    #[test]
    fn test_clear_pending_search() {
        let mut state = SearchState::default();
        state.start_pending_search(Direction::Backward);
        assert!(state.has_pending_search());

        state.clear_pending_search();
        assert!(!state.has_pending_search());
        assert!(state.take_pending_search().is_none());
    }

    #[test]
    fn test_pending_search_backward() {
        let mut state = SearchState::default();
        state.start_pending_search(Direction::Backward);
        assert!(state.has_pending_search());

        let dir = state.take_pending_search();
        assert_eq!(dir, Some(Direction::Backward));
    }

    #[test]
    fn test_take_pending_search_when_none() {
        let mut state = SearchState::default();
        assert!(state.take_pending_search().is_none());
    }

    #[test]
    fn test_direction_for_opposite_forward() {
        let mut state = SearchState::default();
        state.set("hello".to_string(), Direction::Forward);
        assert!(matches!(state.direction_for_opposite(), Direction::Backward));
    }

    #[test]
    fn test_direction_for_opposite_backward() {
        let mut state = SearchState::default();
        state.set("hello".to_string(), Direction::Backward);
        assert!(matches!(state.direction_for_opposite(), Direction::Forward));
    }

    #[test]
    fn test_pattern_for_repeat_none_by_default() {
        let state = SearchState::default();
        assert!(state.pattern_for_repeat().is_none());
    }

    #[test]
    fn test_set_overwrites_previous_pattern() {
        let mut state = SearchState::default();
        state.set("first".to_string(), Direction::Forward);
        assert_eq!(state.pattern_for_repeat(), Some("first"));

        state.set("second".to_string(), Direction::Backward);
        assert_eq!(state.pattern_for_repeat(), Some("second"));
        assert!(matches!(state.direction_for_repeat(), Direction::Backward));
    }

    #[test]
    fn test_session_extension_create() {
        let state = SearchState::create();
        assert!(state.last_pattern.is_none());
        assert!(matches!(state.last_direction, Direction::Forward));
        assert!(!state.has_pending_search());
    }

    #[test]
    fn test_search_state_debug() {
        let state = SearchState::default();
        let debug = format!("{state:?}");
        assert!(debug.contains("SearchState"));
    }
}
