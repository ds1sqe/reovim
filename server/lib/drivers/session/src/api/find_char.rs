//! Find-char state extension for session.
//!
//! Provides shared find-char state that both the coordinator and repeat
//! handlers can access. Stores the last find-char operation for `;`/`,` repeat.
//!
//! Follows the [`SearchState`](super::SearchState) precedent.

use crate::SessionExtension;

/// Record of a find-char operation for repeat.
///
/// Policy-free: uses raw booleans, not vim-specific enum names.
/// Any editor paradigm with "find character on line" can use this.
///
/// Mapping from vim concepts:
///   f -> forward=true,  inclusive=true
///   F -> forward=false, inclusive=true
///   t -> forward=true,  inclusive=false
///   T -> forward=false, inclusive=false
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FindCharRecord {
    /// The character that was searched for.
    char: char,
    /// true = forward (f/t), false = backward (F/T).
    forward: bool,
    /// true = land on char (f/F), false = land before char (t/T).
    inclusive: bool,
}

impl FindCharRecord {
    /// Create a new find-char record.
    #[must_use]
    pub const fn new(char: char, forward: bool, inclusive: bool) -> Self {
        Self {
            char,
            forward,
            inclusive,
        }
    }

    /// The character that was searched for.
    #[must_use]
    pub const fn char(&self) -> char {
        self.char
    }

    /// Whether the search was forward (true) or backward (false).
    #[must_use]
    pub const fn forward(&self) -> bool {
        self.forward
    }

    /// Whether the motion lands on the char (true) or before it (false).
    #[must_use]
    pub const fn inclusive(&self) -> bool {
        self.inclusive
    }

    /// Reverse direction for `,` (repeat in opposite direction).
    #[must_use]
    pub const fn reversed(&self) -> Self {
        Self {
            forward: !self.forward,
            char: self.char,
            inclusive: self.inclusive,
        }
    }
}

/// Per-client find-char state for repeat (`;`/`,`).
///
/// Written by the `DISPATCH_FIND_CHAR` coordinator command.
/// Read by `REPEAT_FIND_SAME` and `REPEAT_FIND_REVERSE` handlers.
///
/// Follows `SearchState` precedent -- accessor methods, private fields.
#[derive(Debug, Default)]
pub struct FindCharState {
    last: Option<FindCharRecord>,
}

impl SessionExtension for FindCharState {
    fn create() -> Self {
        Self::default()
    }
}

impl FindCharState {
    /// Record a find-char operation for later repeat.
    pub const fn record(&mut self, char: char, forward: bool, inclusive: bool) {
        self.last = Some(FindCharRecord::new(char, forward, inclusive));
    }

    /// Get the last find-char record for repeat (`;`).
    #[must_use]
    pub const fn last(&self) -> Option<&FindCharRecord> {
        self.last.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_char_record_new_and_accessors() {
        let record = FindCharRecord::new('x', true, true);
        assert_eq!(record.char(), 'x');
        assert!(record.forward());
        assert!(record.inclusive());
    }

    #[test]
    fn test_find_char_record_backward_till() {
        let record = FindCharRecord::new('a', false, false);
        assert_eq!(record.char(), 'a');
        assert!(!record.forward());
        assert!(!record.inclusive());
    }

    #[test]
    fn test_find_char_record_reversed() {
        let record = FindCharRecord::new('x', true, true);
        let rev = record.reversed();
        assert_eq!(rev.char(), 'x');
        assert!(!rev.forward());
        assert!(rev.inclusive());
    }

    #[test]
    fn test_find_char_record_reversed_preserves_inclusive() {
        let record = FindCharRecord::new('t', true, false);
        let rev = record.reversed();
        assert_eq!(rev.char(), 't');
        assert!(!rev.forward());
        assert!(!rev.inclusive());
    }

    #[test]
    fn test_find_char_record_double_reversed() {
        let original = FindCharRecord::new('z', true, false);
        let double_rev = original.reversed().reversed();
        assert_eq!(double_rev, original);
    }

    #[test]
    fn test_find_char_record_unicode() {
        let record = FindCharRecord::new('\u{1F600}', true, true);
        assert_eq!(record.char(), '\u{1F600}');
    }

    #[test]
    fn test_find_char_record_partial_eq() {
        let a = FindCharRecord::new('x', true, true);
        let b = FindCharRecord::new('x', true, true);
        let c = FindCharRecord::new('y', true, true);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_find_char_record_clone_copy() {
        let record = FindCharRecord::new('a', true, false);
        let cloned = record;
        let copied = record;
        assert_eq!(record, cloned);
        assert_eq!(record, copied);
    }

    #[test]
    fn test_find_char_record_debug() {
        let record = FindCharRecord::new('x', true, true);
        let debug = format!("{record:?}");
        assert!(debug.contains("FindCharRecord"));
    }

    #[test]
    fn test_find_char_state_default() {
        let state = FindCharState::default();
        assert!(state.last().is_none());
    }

    #[test]
    fn test_find_char_state_record_and_last() {
        let mut state = FindCharState::default();
        state.record('x', true, true);
        let record = state.last().unwrap();
        assert_eq!(record.char(), 'x');
        assert!(record.forward());
        assert!(record.inclusive());
    }

    #[test]
    fn test_find_char_state_record_overwrites() {
        let mut state = FindCharState::default();
        state.record('a', true, true);
        state.record('b', false, false);
        let record = state.last().unwrap();
        assert_eq!(record.char(), 'b');
        assert!(!record.forward());
        assert!(!record.inclusive());
    }

    #[test]
    fn test_find_char_state_last_returns_ref() {
        let mut state = FindCharState::default();
        state.record('x', true, true);
        let copied = state.last().copied();
        assert_eq!(copied.unwrap().char(), 'x');
    }

    #[test]
    fn test_session_extension_create() {
        let state = FindCharState::create();
        assert!(state.last().is_none());
    }

    #[test]
    fn test_find_char_state_debug() {
        let state = FindCharState::default();
        let debug = format!("{state:?}");
        assert!(debug.contains("FindCharState"));
    }
}
