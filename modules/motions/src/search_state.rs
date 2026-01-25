//! Search state - per-session state for vim-style search.
//!
//! Stores the last search pattern and direction for n/N repeat.

use {reovim_driver_search::Direction, reovim_driver_session::SessionExtension};

/// Per-session search state.
///
/// Stores the last search pattern and direction for n, N, *, # commands.
#[derive(Debug, Default)]
pub struct SearchState {
    /// Last search pattern (regex format).
    pub last_pattern: Option<String>,

    /// Last search direction (forward or backward).
    pub last_direction: Direction,
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
}
