//! Command-line input buffer.
//!
//! Stores only text input and cursor position.
//! Active state and prompt type come from `CmdlineState` extension (SSOT).

// ============================================================================
// Command-line Buffer (Issue #452)
// ============================================================================

/// Input buffer for command-line mode.
///
/// This struct only stores the text input and cursor position.
/// The active state, prompt type, and cancellation status are stored
/// in the `CmdlineState` session extension (single source of truth).
#[derive(Debug, Clone, Default)]
pub struct CmdlineBuffer {
    /// Input text.
    input: String,
    /// Cursor position within input.
    cursor: usize,
}

impl CmdlineBuffer {
    /// Create a new empty buffer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear the buffer and reset cursor.
    pub fn clear(&mut self) {
        self.input.clear();
        self.cursor = 0;
    }

    /// Insert a character at cursor position.
    pub fn insert_char(&mut self, ch: char) {
        if self.cursor >= self.input.len() {
            self.input.push(ch);
        } else {
            self.input.insert(self.cursor, ch);
        }
        self.cursor += 1;
    }

    /// Delete character before cursor (Backspace).
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.input.remove(self.cursor);
        }
    }

    /// Get the current input buffer.
    #[must_use]
    pub fn input(&self) -> &str {
        &self.input
    }

    /// Take ownership of the input, clearing the buffer.
    ///
    /// Returns the input string and resets the buffer.
    pub fn take(&mut self) -> String {
        self.cursor = 0;
        std::mem::take(&mut self.input)
    }

    /// Get cursor position.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Check if buffer is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.input.is_empty()
    }

    /// Move cursor left.
    ///
    /// Does nothing if already at start.
    #[allow(clippy::missing_const_for_fn)] // Mutating self
    pub fn cursor_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Move cursor right.
    ///
    /// Does nothing if already at end.
    #[allow(clippy::missing_const_for_fn)] // Accesses self.input.len()
    pub fn cursor_right(&mut self) {
        if self.cursor < self.input.len() {
            self.cursor += 1;
        }
    }

    /// Delete character at cursor (Delete key).
    ///
    /// Does nothing if cursor is at end.
    pub fn delete_char(&mut self) {
        if self.cursor < self.input.len() {
            self.input.remove(self.cursor);
        }
    }

    /// Move cursor to start of line.
    #[allow(clippy::missing_const_for_fn)] // Mutating self
    pub fn cursor_home(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to end of line.
    #[allow(clippy::missing_const_for_fn)] // Accesses self.input.len()
    pub fn cursor_end(&mut self) {
        self.cursor = self.input.len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_buffer_is_empty() {
        let buf = CmdlineBuffer::new();
        assert!(buf.is_empty());
        assert_eq!(buf.input(), "");
        assert_eq!(buf.cursor(), 0);
    }

    #[test]
    fn test_insert_char() {
        let mut buf = CmdlineBuffer::new();
        buf.insert_char('a');
        buf.insert_char('b');
        buf.insert_char('c');
        assert_eq!(buf.input(), "abc");
        assert_eq!(buf.cursor(), 3);
    }

    #[test]
    fn test_backspace() {
        let mut buf = CmdlineBuffer::new();
        buf.insert_char('a');
        buf.insert_char('b');
        buf.backspace();
        assert_eq!(buf.input(), "a");
        assert_eq!(buf.cursor(), 1);
    }

    #[test]
    fn test_backspace_at_start() {
        let mut buf = CmdlineBuffer::new();
        buf.backspace(); // Should do nothing
        assert!(buf.is_empty());
        assert_eq!(buf.cursor(), 0);
    }

    #[test]
    fn test_clear() {
        let mut buf = CmdlineBuffer::new();
        buf.insert_char('x');
        buf.clear();
        assert!(buf.is_empty());
        assert_eq!(buf.cursor(), 0);
    }

    #[test]
    fn test_take() {
        let mut buf = CmdlineBuffer::new();
        buf.insert_char('t');
        buf.insert_char('e');
        buf.insert_char('s');
        buf.insert_char('t');
        let taken = buf.take();
        assert_eq!(taken, "test");
        assert!(buf.is_empty());
        assert_eq!(buf.cursor(), 0);
    }

    #[test]
    fn test_cursor_left() {
        let mut buf = CmdlineBuffer::new();
        buf.insert_char('a');
        buf.insert_char('b');
        buf.insert_char('c');
        assert_eq!(buf.cursor(), 3);

        buf.cursor_left();
        assert_eq!(buf.cursor(), 2);

        buf.cursor_left();
        assert_eq!(buf.cursor(), 1);
    }

    #[test]
    fn test_cursor_left_at_start() {
        let mut buf = CmdlineBuffer::new();
        buf.cursor_left(); // Should do nothing
        assert_eq!(buf.cursor(), 0);
    }

    #[test]
    fn test_cursor_right() {
        let mut buf = CmdlineBuffer::new();
        buf.insert_char('a');
        buf.insert_char('b');
        buf.cursor_left();
        buf.cursor_left();
        assert_eq!(buf.cursor(), 0);

        buf.cursor_right();
        assert_eq!(buf.cursor(), 1);

        buf.cursor_right();
        assert_eq!(buf.cursor(), 2);
    }

    #[test]
    fn test_cursor_right_at_end() {
        let mut buf = CmdlineBuffer::new();
        buf.insert_char('a');
        buf.cursor_right(); // Should do nothing, already at end
        assert_eq!(buf.cursor(), 1);
    }

    #[test]
    fn test_delete_char() {
        let mut buf = CmdlineBuffer::new();
        buf.insert_char('a');
        buf.insert_char('b');
        buf.insert_char('c');
        buf.cursor_left(); // cursor at 'c'
        buf.cursor_left(); // cursor at 'b'

        buf.delete_char(); // delete 'b'
        assert_eq!(buf.input(), "ac");
        assert_eq!(buf.cursor(), 1);
    }

    #[test]
    fn test_delete_char_at_end() {
        let mut buf = CmdlineBuffer::new();
        buf.insert_char('a');
        buf.delete_char(); // Should do nothing, cursor at end
        assert_eq!(buf.input(), "a");
    }

    #[test]
    fn test_cursor_home() {
        let mut buf = CmdlineBuffer::new();
        buf.insert_char('a');
        buf.insert_char('b');
        buf.insert_char('c');
        assert_eq!(buf.cursor(), 3);

        buf.cursor_home();
        assert_eq!(buf.cursor(), 0);
    }

    #[test]
    fn test_cursor_end() {
        let mut buf = CmdlineBuffer::new();
        buf.insert_char('a');
        buf.insert_char('b');
        buf.insert_char('c');
        buf.cursor_home();
        assert_eq!(buf.cursor(), 0);

        buf.cursor_end();
        assert_eq!(buf.cursor(), 3);
    }
}
