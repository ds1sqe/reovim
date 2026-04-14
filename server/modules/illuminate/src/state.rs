//! Per-client illuminate state.
//!
//! Stores highlighted ranges and cursor-hold tracking for each client.

use {reovim_driver_text_session::SessionExtension, reovim_kernel::api::v1::BufferId};

/// Kind of document highlight (from LSP `DocumentHighlightKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightKind {
    /// A textual occurrence.
    Text,
    /// Read-access of a symbol.
    Read,
    /// Write-access of a symbol.
    Write,
}

impl HighlightKind {
    /// Convert to a string for JSON serialization.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Read => "read",
            Self::Write => "write",
        }
    }
}

/// A highlighted range in the buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightRange {
    /// Start line (0-indexed).
    pub start_line: u32,
    /// Start column (0-indexed).
    pub start_col: u32,
    /// End line (0-indexed).
    pub end_line: u32,
    /// End column (0-indexed, exclusive).
    pub end_col: u32,
    /// Highlight kind (text, read, write).
    pub kind: HighlightKind,
}

/// Per-client illuminate state.
///
/// Tracks the word under cursor, highlighted ranges, and cursor-hold
/// timing for debounce.
#[derive(Debug)]
pub struct IlluminateState {
    /// Buffer ID the highlights apply to.
    pub buffer_id: BufferId,
    /// The word/symbol being highlighted.
    pub word: String,
    /// Highlighted ranges.
    pub ranges: Vec<HighlightRange>,
    /// Whether highlights are currently active.
    pub active: bool,
    /// Cursor position when highlights were computed (line).
    pub origin_line: u32,
    /// Cursor position when highlights were computed (col).
    pub origin_col: u32,
    /// Sequence counter for change detection.
    pub sequence: u64,
    /// Last known cursor position for hold detection (line).
    pub shadow_line: u32,
    /// Last known cursor position for hold detection (col).
    pub shadow_col: u32,
    /// Number of consecutive ticks at the same cursor position.
    pub idle_ticks: u32,
    /// Whether highlights have been computed for the current position.
    pub computed: bool,
}

impl SessionExtension for IlluminateState {
    fn create() -> Self {
        Self {
            buffer_id: BufferId::from_raw(0),
            word: String::new(),
            ranges: Vec::new(),
            active: false,
            origin_line: 0,
            origin_col: 0,
            sequence: 0,
            shadow_line: u32::MAX,
            shadow_col: u32::MAX,
            idle_ticks: 0,
            computed: false,
        }
    }
}

impl IlluminateState {
    /// Update highlights with new ranges.
    pub fn set_highlights(
        &mut self,
        buffer_id: BufferId,
        word: String,
        ranges: Vec<HighlightRange>,
        line: u32,
        col: u32,
    ) {
        self.buffer_id = buffer_id;
        self.word = word;
        self.ranges = ranges;
        self.active = true;
        self.origin_line = line;
        self.origin_col = col;
        self.sequence += 1;
        self.computed = true;
    }

    /// Clear all highlights.
    pub fn clear(&mut self) {
        if self.active {
            self.sequence += 1;
        }
        self.word.clear();
        self.ranges.clear();
        self.active = false;
        self.computed = false;
    }

    /// Record a cursor movement, resetting idle tracking.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub const fn cursor_moved(&mut self, line: u32, col: u32) {
        if self.shadow_line != line || self.shadow_col != col {
            self.shadow_line = line;
            self.shadow_col = col;
            self.idle_ticks = 0;
            self.computed = false;
        }
    }

    /// Increment idle tick counter. Returns the new count.
    pub const fn tick(&mut self) -> u32 {
        self.idle_ticks += 1;
        self.idle_ticks
    }

    /// Find the index of the next range after the given cursor position.
    ///
    /// Wraps around to the first range if past the last one.
    /// Returns `None` if no ranges exist.
    #[must_use]
    pub fn next_range_index(&self, line: u32, col: u32) -> Option<usize> {
        if self.ranges.is_empty() {
            return None;
        }

        // Find the first range that starts strictly after (line, col)
        for (i, r) in self.ranges.iter().enumerate() {
            if r.start_line > line || (r.start_line == line && r.start_col > col) {
                return Some(i);
            }
        }

        // Wrap around to first
        Some(0)
    }

    /// Find the index of the previous range before the given cursor position.
    ///
    /// Wraps around to the last range if before the first one.
    /// Returns `None` if no ranges exist.
    #[must_use]
    pub fn prev_range_index(&self, line: u32, col: u32) -> Option<usize> {
        if self.ranges.is_empty() {
            return None;
        }

        // Find the last range that starts strictly before (line, col)
        for (i, r) in self.ranges.iter().enumerate().rev() {
            if r.start_line < line || (r.start_line == line && r.start_col < col) {
                return Some(i);
            }
        }

        // Wrap around to last
        Some(self.ranges.len() - 1)
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
