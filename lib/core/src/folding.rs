//! Code folding support
//!
//! Provides fold state management and fold range computation for code folding.
//! Folds are computed from treesitter queries that capture foldable regions.

use std::collections::{HashMap, HashSet};

/// Kind of fold (what construct it represents)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldKind {
    /// Function body
    Function,
    /// Class/struct/impl body
    Class,
    /// Import group
    Import,
    /// Comment block
    Comment,
    /// Generic block (if, for, while, etc.)
    Block,
}

/// A foldable range in the buffer
#[derive(Debug, Clone)]
pub struct FoldRange {
    /// Starting line (0-indexed)
    pub start_line: u32,
    /// Ending line (0-indexed, inclusive)
    pub end_line: u32,
    /// Kind of fold
    pub kind: FoldKind,
    /// Preview text (first line content)
    pub preview: String,
}

impl FoldRange {
    /// Create a new fold range
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // String is not const
    pub fn new(start_line: u32, end_line: u32, kind: FoldKind, preview: String) -> Self {
        Self {
            start_line,
            end_line,
            kind,
            preview,
        }
    }

    /// Check if this range spans multiple lines
    #[must_use]
    pub const fn is_foldable(&self) -> bool {
        self.end_line > self.start_line
    }

    /// Get the number of lines in this fold
    #[must_use]
    pub const fn line_count(&self) -> u32 {
        self.end_line - self.start_line + 1
    }

    /// Check if a line is contained within this fold (excluding the start line)
    #[must_use]
    pub const fn contains_line(&self, line: u32) -> bool {
        line > self.start_line && line <= self.end_line
    }
}

/// Fold state for a single buffer
#[derive(Debug, Clone, Default)]
pub struct FoldState {
    /// All foldable ranges, sorted by start line
    ranges: Vec<FoldRange>,
    /// Indices of collapsed ranges
    collapsed: HashSet<usize>,
}

impl FoldState {
    /// Create a new empty fold state
    #[must_use]
    pub fn new() -> Self {
        Self {
            ranges: Vec::new(),
            collapsed: HashSet::new(),
        }
    }

    /// Set the fold ranges (replaces existing)
    pub fn set_ranges(&mut self, mut ranges: Vec<FoldRange>) {
        // Sort by start line
        ranges.sort_by_key(|r| r.start_line);
        self.ranges = ranges;
        // Clear collapsed state since indices may have changed
        self.collapsed.clear();
    }

    /// Get all fold ranges
    #[must_use]
    pub fn ranges(&self) -> &[FoldRange] {
        &self.ranges
    }

    /// Find fold range at a specific line
    #[must_use]
    pub fn fold_at_line(&self, line: u32) -> Option<(usize, &FoldRange)> {
        self.ranges
            .iter()
            .enumerate()
            .find(|(_, r)| r.start_line == line && r.is_foldable())
    }

    /// Toggle fold at line
    ///
    /// Returns true if the fold state changed
    pub fn toggle(&mut self, line: u32) -> bool {
        if let Some((idx, _)) = self.fold_at_line(line) {
            if self.collapsed.contains(&idx) {
                self.collapsed.remove(&idx);
            } else {
                self.collapsed.insert(idx);
            }
            return true;
        }
        false
    }

    /// Open (expand) fold at line
    pub fn open(&mut self, line: u32) -> bool {
        if let Some((idx, _)) = self.fold_at_line(line) {
            return self.collapsed.remove(&idx);
        }
        false
    }

    /// Close (collapse) fold at line
    pub fn close(&mut self, line: u32) -> bool {
        if let Some((idx, _)) = self.fold_at_line(line) {
            return self.collapsed.insert(idx);
        }
        false
    }

    /// Open all folds
    pub fn open_all(&mut self) {
        self.collapsed.clear();
    }

    /// Close all folds
    pub fn close_all(&mut self) {
        for idx in 0..self.ranges.len() {
            if self.ranges[idx].is_foldable() {
                self.collapsed.insert(idx);
            }
        }
    }

    /// Check if a fold at the given line is collapsed
    #[must_use]
    pub fn is_collapsed(&self, line: u32) -> bool {
        if let Some((idx, _)) = self.fold_at_line(line) {
            return self.collapsed.contains(&idx);
        }
        false
    }

    /// Check if a line is hidden inside a collapsed fold
    #[must_use]
    pub fn is_line_hidden(&self, line: u32) -> bool {
        for &idx in &self.collapsed {
            if let Some(range) = self.ranges.get(idx)
                && range.contains_line(line)
            {
                return true;
            }
        }
        false
    }

    /// Get fold marker info if line starts a collapsed fold
    ///
    /// Returns (`hidden_line_count`, preview) if the line has a collapsed fold
    #[must_use]
    pub fn get_fold_marker(&self, line: u32) -> Option<(u32, &str)> {
        if let Some((idx, range)) = self.fold_at_line(line)
            && self.collapsed.contains(&idx)
        {
            return Some((range.line_count() - 1, &range.preview));
        }
        None
    }

    /// Map display line to buffer line (accounting for collapsed folds)
    ///
    /// Returns the buffer line number for a given display line number.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn display_to_buffer_line(&self, display_line: u32) -> u32 {
        let mut buffer_line = 0u32;
        let mut display_count = 0u32;

        while display_count < display_line {
            if !self.is_line_hidden(buffer_line) {
                display_count += 1;
            }
            buffer_line += 1;
        }

        // Skip hidden lines to find the next visible line
        while self.is_line_hidden(buffer_line) {
            buffer_line += 1;
        }

        buffer_line
    }

    /// Map buffer line to display line
    #[must_use]
    pub fn buffer_to_display_line(&self, buffer_line: u32) -> u32 {
        let mut display_line = 0u32;

        for line in 0..buffer_line {
            if !self.is_line_hidden(line) {
                display_line += 1;
            }
        }

        display_line
    }

    /// Get the number of visible lines
    #[must_use]
    pub fn visible_line_count(&self, total_lines: u32) -> u32 {
        let mut count = 0;
        for line in 0..total_lines {
            if !self.is_line_hidden(line) {
                count += 1;
            }
        }
        count
    }
}

/// Manages fold state for all buffers
#[derive(Debug, Default)]
pub struct FoldManager {
    /// Per-buffer fold state
    states: HashMap<usize, FoldState>,
}

impl FoldManager {
    /// Create a new fold manager
    #[must_use]
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
        }
    }

    /// Get or create fold state for a buffer
    pub fn get_or_create(&mut self, buffer_id: usize) -> &mut FoldState {
        self.states.entry(buffer_id).or_default()
    }

    /// Get fold state for a buffer (read-only)
    #[must_use]
    pub fn get(&self, buffer_id: usize) -> Option<&FoldState> {
        self.states.get(&buffer_id)
    }

    /// Update fold ranges for a buffer
    pub fn set_ranges(&mut self, buffer_id: usize, ranges: Vec<FoldRange>) {
        self.get_or_create(buffer_id).set_ranges(ranges);
    }

    /// Toggle fold at line in buffer
    pub fn toggle(&mut self, buffer_id: usize, line: u32) -> bool {
        self.get_or_create(buffer_id).toggle(line)
    }

    /// Open fold at line in buffer
    pub fn open(&mut self, buffer_id: usize, line: u32) -> bool {
        self.get_or_create(buffer_id).open(line)
    }

    /// Close fold at line in buffer
    pub fn close(&mut self, buffer_id: usize, line: u32) -> bool {
        self.get_or_create(buffer_id).close(line)
    }

    /// Open all folds in buffer
    pub fn open_all(&mut self, buffer_id: usize) {
        self.get_or_create(buffer_id).open_all();
    }

    /// Close all folds in buffer
    pub fn close_all(&mut self, buffer_id: usize) {
        self.get_or_create(buffer_id).close_all();
    }

    /// Check if line is hidden in buffer
    #[must_use]
    pub fn is_line_hidden(&self, buffer_id: usize, line: u32) -> bool {
        self.states
            .get(&buffer_id)
            .is_some_and(|s| s.is_line_hidden(line))
    }

    /// Get fold marker for line if it's a collapsed fold start
    #[must_use]
    pub fn get_fold_marker(&self, buffer_id: usize, line: u32) -> Option<(u32, String)> {
        self.states
            .get(&buffer_id)
            .and_then(|s| s.get_fold_marker(line))
            .map(|(count, preview)| (count, preview.to_string()))
    }

    /// Remove state for a buffer
    pub fn remove_buffer(&mut self, buffer_id: usize) {
        self.states.remove(&buffer_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fold_range() {
        let range = FoldRange::new(5, 10, FoldKind::Function, "fn foo()".to_string());

        assert!(range.is_foldable());
        assert_eq!(range.line_count(), 6);
        assert!(!range.contains_line(5)); // Start line not contained
        assert!(range.contains_line(6));
        assert!(range.contains_line(10));
        assert!(!range.contains_line(11));
    }

    #[test]
    fn test_fold_state_toggle() {
        let mut state = FoldState::new();
        state.set_ranges(vec![
            FoldRange::new(0, 5, FoldKind::Function, "fn one()".to_string()),
            FoldRange::new(10, 15, FoldKind::Function, "fn two()".to_string()),
        ]);

        // Initially not collapsed
        assert!(!state.is_collapsed(0));
        assert!(!state.is_line_hidden(3));

        // Toggle to collapse
        assert!(state.toggle(0));
        assert!(state.is_collapsed(0));
        assert!(state.is_line_hidden(3));

        // Toggle to expand
        assert!(state.toggle(0));
        assert!(!state.is_collapsed(0));
        assert!(!state.is_line_hidden(3));
    }

    #[test]
    fn test_fold_marker() {
        let mut state = FoldState::new();
        state.set_ranges(vec![FoldRange::new(
            0,
            5,
            FoldKind::Function,
            "fn foo()".to_string(),
        )]);

        // Not collapsed - no marker
        assert!(state.get_fold_marker(0).is_none());

        // Collapse
        state.close(0);

        // Should have marker
        let marker = state.get_fold_marker(0);
        assert!(marker.is_some());
        let (count, preview) = marker.unwrap();
        assert_eq!(count, 5); // 5 hidden lines (1-5)
        assert_eq!(preview, "fn foo()");
    }

    #[test]
    fn test_open_close_all() {
        let mut state = FoldState::new();
        state.set_ranges(vec![
            FoldRange::new(0, 5, FoldKind::Function, "fn one()".to_string()),
            FoldRange::new(10, 15, FoldKind::Function, "fn two()".to_string()),
        ]);

        state.close_all();
        assert!(state.is_collapsed(0));
        assert!(state.is_collapsed(10));

        state.open_all();
        assert!(!state.is_collapsed(0));
        assert!(!state.is_collapsed(10));
    }
}
