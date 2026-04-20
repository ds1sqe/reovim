//! Mark storage for bookmark operations.
//!
//! Marks allow jumping to saved positions within buffers.
//!
//! # Vim Mark Types
//!
//! - `'a` to `'z` - Buffer-local marks
//! - `'A` to `'Z` - Global marks (across buffers)
//! - `''` - Last jump position
//! - `'.` - Last edit position
//! - `'^` - Last insert position

use std::collections::HashMap;

use {reovim_domain_text::Position, reovim_kernel::api::v1::BufferId};

/// A bookmark position with optional buffer association.
///
/// For global marks, both `position` and `buffer_id` are significant.
/// For buffer-local marks, only `position` is used (buffer is implicit).
///
/// # Example
///
/// ```
/// use reovim_driver_text_session::Mark;
/// use reovim_kernel::api::v1::BufferId;
/// use reovim_domain_text::Position;
///
/// let mark = Mark {
///     position: Position::new(10, 5),
///     buffer_id: BufferId::new(),
/// };
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mark {
    /// Position within the buffer.
    pub position: Position,
    /// Buffer this mark belongs to (for global marks).
    pub buffer_id: BufferId,
}

impl Mark {
    /// Create a new mark at the given position and buffer.
    #[must_use]
    pub const fn new(position: Position, buffer_id: BufferId) -> Self {
        Self {
            position,
            buffer_id,
        }
    }
}

/// Special mark types for automatic bookmarks.
///
/// These marks are automatically maintained by the editor.
///
/// # Example
///
/// ```
/// use reovim_driver_text_session::SpecialMark;
///
/// // Jump to last position before current jump
/// let last_jump = SpecialMark::LastJump; // ''
///
/// // Jump to last edit location
/// let last_edit = SpecialMark::LastEdit; // '.
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpecialMark {
    /// Position before last jump (vim: `''` or backtick-backtick)
    LastJump,
    /// Position of last edit (`'.`)
    LastEdit,
    /// Position of last insert (`'^`)
    LastInsert,
    /// Start of last visual selection (`'<`)
    VisualStart,
    /// End of last visual selection (`'>`)
    VisualEnd,
    /// Position where last exited insert mode
    LastExitInsert,
}

/// Mark storage for all mark types.
///
/// Manages buffer-local marks, global marks, and special marks.
///
/// # Mark Types
///
/// - **Local marks** (`'a`-`'z`): Stored per-buffer, only position is tracked
/// - **Global marks** (`'A`-`'Z`): Stored globally, includes buffer ID
/// - **Special marks**: Automatically maintained editor positions
///
/// # Example
///
/// ```
/// use reovim_driver_text_session::{Mark, MarkBank, SpecialMark};
/// use reovim_kernel::api::v1::BufferId;
/// use reovim_domain_text::Position;
///
/// let mut marks = MarkBank::new();
///
/// // Set local mark
/// marks.set_local('a', Position::new(10, 0));
///
/// // Set global mark
/// let buffer_id = BufferId::new();
/// marks.set_global('A', Mark::new(Position::new(5, 0), buffer_id));
///
/// // Set special mark (usually done automatically)
/// marks.set_special(SpecialMark::LastJump, Mark::new(Position::new(0, 0), buffer_id));
/// ```
#[derive(Debug, Clone, Default)]
pub struct MarkBank {
    /// Buffer-local marks ('a'-'z).
    ///
    /// These are positions within a single buffer.
    /// The buffer context is provided externally when accessing.
    local: HashMap<char, Position>,

    /// Global marks ('A'-'Z).
    ///
    /// These include buffer ID for cross-buffer jumps.
    global: HashMap<char, Mark>,

    /// Special marks maintained by the editor.
    special: HashMap<SpecialMark, Mark>,
}

impl MarkBank {
    /// Create a new empty mark bank.
    #[must_use]
    pub fn new() -> Self {
        Self {
            local: HashMap::new(),
            global: HashMap::new(),
            special: HashMap::new(),
        }
    }

    // === Local Marks ===

    /// Set a local mark ('a'-'z').
    ///
    /// Returns `true` if successful, `false` if the mark name is invalid.
    pub fn set_local(&mut self, name: char, position: Position) -> bool {
        if name.is_ascii_lowercase() {
            self.local.insert(name, position);
            true
        } else {
            false
        }
    }

    /// Get a local mark ('a'-'z').
    ///
    /// Returns `None` if the mark doesn't exist or name is invalid.
    #[must_use]
    pub fn get_local(&self, name: char) -> Option<Position> {
        if name.is_ascii_lowercase() {
            self.local.get(&name).copied()
        } else {
            None
        }
    }

    /// Delete a local mark.
    ///
    /// Returns `true` if the mark existed and was deleted.
    pub fn delete_local(&mut self, name: char) -> bool {
        if name.is_ascii_lowercase() {
            self.local.remove(&name).is_some()
        } else {
            false
        }
    }

    /// List all local marks.
    #[must_use]
    pub fn list_local(&self) -> Vec<(char, Position)> {
        let mut marks: Vec<_> = self.local.iter().map(|(&c, &p)| (c, p)).collect();
        marks.sort_by_key(|(c, _)| *c);
        marks
    }

    // === Global Marks ===

    /// Set a global mark ('A'-'Z').
    ///
    /// Returns `true` if successful, `false` if the mark name is invalid.
    pub fn set_global(&mut self, name: char, mark: Mark) -> bool {
        if name.is_ascii_uppercase() {
            self.global.insert(name, mark);
            true
        } else {
            false
        }
    }

    /// Get a global mark ('A'-'Z').
    ///
    /// Returns `None` if the mark doesn't exist or name is invalid.
    #[must_use]
    pub fn get_global(&self, name: char) -> Option<&Mark> {
        if name.is_ascii_uppercase() {
            self.global.get(&name)
        } else {
            None
        }
    }

    /// Delete a global mark.
    ///
    /// Returns `true` if the mark existed and was deleted.
    pub fn delete_global(&mut self, name: char) -> bool {
        if name.is_ascii_uppercase() {
            self.global.remove(&name).is_some()
        } else {
            false
        }
    }

    /// List all global marks.
    #[must_use]
    pub fn list_global(&self) -> Vec<(char, &Mark)> {
        let mut marks: Vec<_> = self.global.iter().map(|(&c, m)| (c, m)).collect();
        marks.sort_by_key(|(c, _)| *c);
        marks
    }

    // === Special Marks ===

    /// Set a special mark.
    pub fn set_special(&mut self, mark: SpecialMark, value: Mark) {
        self.special.insert(mark, value);
    }

    /// Get a special mark.
    #[must_use]
    pub fn get_special(&self, mark: SpecialMark) -> Option<&Mark> {
        self.special.get(&mark)
    }

    /// Clear a special mark.
    pub fn clear_special(&mut self, mark: SpecialMark) {
        self.special.remove(&mark);
    }

    // === Combined Operations ===

    /// Get mark by character (local, global, or special shorthand).
    ///
    /// - `'a'`-`'z'` - Local marks (returns `None` for position, needs buffer context)
    /// - `'A'`-`'Z'` - Global marks
    /// - `'\''` or `` '`' `` - Last jump
    /// - `'.'` - Last edit
    /// - `'^'` - Last insert
    /// - `'<'` - Visual start
    /// - `'>'` - Visual end
    #[must_use]
    pub fn get_by_char(&self, c: char) -> Option<MarkResult<'_>> {
        match c {
            'a'..='z' => self.local.get(&c).map(|&p| MarkResult::Local(p)),
            'A'..='Z' => self.global.get(&c).map(MarkResult::Global),
            '\'' | '`' => self
                .special
                .get(&SpecialMark::LastJump)
                .map(MarkResult::Global),
            '.' => self
                .special
                .get(&SpecialMark::LastEdit)
                .map(MarkResult::Global),
            '^' => self
                .special
                .get(&SpecialMark::LastInsert)
                .map(MarkResult::Global),
            '<' => self
                .special
                .get(&SpecialMark::VisualStart)
                .map(MarkResult::Global),
            '>' => self
                .special
                .get(&SpecialMark::VisualEnd)
                .map(MarkResult::Global),
            _ => None,
        }
    }

    /// Clear all local marks (for when buffer is closed).
    pub fn clear_local(&mut self) {
        self.local.clear();
    }

    /// Clear all marks.
    pub fn clear_all(&mut self) {
        self.local.clear();
        self.global.clear();
        self.special.clear();
    }
}

/// Result of a mark lookup.
#[derive(Debug, Clone, Copy)]
pub enum MarkResult<'a> {
    /// Local mark - position within current buffer.
    Local(Position),
    /// Global mark - includes buffer ID.
    Global(&'a Mark),
}

impl MarkResult<'_> {
    /// Get the position from either mark type.
    #[must_use]
    pub const fn position(&self) -> Position {
        match self {
            Self::Local(pos) => *pos,
            Self::Global(mark) => mark.position,
        }
    }

    /// Get buffer ID if this is a global mark.
    #[must_use]
    pub const fn buffer_id(&self) -> Option<BufferId> {
        match self {
            Self::Local(_) => None,
            Self::Global(mark) => Some(mark.buffer_id),
        }
    }
}

#[cfg(test)]
#[path = "mark_tests.rs"]
mod tests;
