//! Jump list for cursor position history (Ctrl-O / Ctrl-I navigation)

use crate::screen::Position;

/// Maximum number of entries in the jump list
const MAX_JUMP_LIST_SIZE: usize = 100;

/// A single entry in the jump list
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JumpEntry {
    pub buffer_id: usize,
    pub position: Position,
}

/// Jump list for tracking cursor positions
///
/// Tracks "major" cursor position changes for navigation.
/// Ctrl-O jumps backward (older), Ctrl-I jumps forward (newer).
#[derive(Debug, Default)]
pub struct JumpList {
    entries: Vec<JumpEntry>,
    /// Current position in the list (index into entries)
    /// When at the end, this equals `entries.len()`
    current_index: usize,
}

impl JumpList {
    /// Create a new empty jump list
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a new position in the jump list
    ///
    /// This truncates any "future" entries if we're not at the end.
    pub fn push(&mut self, buffer_id: usize, position: Position) {
        let entry = JumpEntry {
            buffer_id,
            position,
        };

        // Don't add duplicate if same as last entry
        if self.entries.last() == Some(&entry) {
            return;
        }

        // Truncate any entries after current position
        self.entries.truncate(self.current_index);

        // Add new entry
        self.entries.push(entry);

        // Enforce max size
        if self.entries.len() > MAX_JUMP_LIST_SIZE {
            self.entries.remove(0);
        }

        // Update index to point to end
        self.current_index = self.entries.len();
    }

    /// Jump to older position (Ctrl-O)
    ///
    /// Returns the position to jump to, or None if at beginning.
    pub fn jump_older(&mut self) -> Option<&JumpEntry> {
        if self.current_index > 0 {
            self.current_index -= 1;
            self.entries.get(self.current_index)
        } else {
            None
        }
    }

    /// Jump to newer position (Ctrl-I)
    ///
    /// Returns the position to jump to, or None if at end.
    pub fn jump_newer(&mut self) -> Option<&JumpEntry> {
        if self.current_index < self.entries.len() {
            let entry = self.entries.get(self.current_index);
            self.current_index += 1;
            entry
        } else {
            None
        }
    }

    /// Get current index for debugging/display
    #[must_use]
    pub const fn current_index(&self) -> usize {
        self.current_index
    }

    /// Get total entries for debugging/display
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec::len() is not const
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the jump list is empty
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec::is_empty() is not const
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_jump_list() {
        let mut list = JumpList::new();
        assert!(list.is_empty());
        assert_eq!(list.jump_older(), None);
        assert_eq!(list.jump_newer(), None);
    }

    #[test]
    fn test_push_and_jump() {
        let mut list = JumpList::new();
        list.push(0, Position { x: 0, y: 0 });
        list.push(0, Position { x: 10, y: 5 });
        list.push(0, Position { x: 20, y: 10 });

        assert_eq!(list.len(), 3);

        // Jump older twice
        let entry = list.jump_older().unwrap();
        assert_eq!(entry.position, Position { x: 20, y: 10 });

        let entry = list.jump_older().unwrap();
        assert_eq!(entry.position, Position { x: 10, y: 5 });

        // Jump newer once
        let entry = list.jump_newer().unwrap();
        assert_eq!(entry.position, Position { x: 10, y: 5 });
    }

    #[test]
    fn test_no_duplicate_entries() {
        let mut list = JumpList::new();
        list.push(0, Position { x: 5, y: 5 });
        list.push(0, Position { x: 5, y: 5 }); // Duplicate
        list.push(0, Position { x: 5, y: 5 }); // Duplicate

        assert_eq!(list.len(), 1);
    }

    #[test]
    fn test_truncate_on_push_after_jump() {
        let mut list = JumpList::new();
        list.push(0, Position { x: 0, y: 0 });
        list.push(0, Position { x: 10, y: 10 });
        list.push(0, Position { x: 20, y: 20 });

        // Jump back
        list.jump_older();
        list.jump_older();

        // Push new entry - should truncate future entries
        list.push(0, Position { x: 5, y: 5 });

        assert_eq!(list.len(), 2); // Only first entry + new entry
    }
}
