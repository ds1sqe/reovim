//! Per-buffer fold state management.
//!
//! Tracks which foldable regions are collapsed. Fold ranges come from
//! `SyntaxDriver::folds()`; this module only manages the collapsed/open
//! status of each fold.

use std::collections::{HashMap, HashSet};

use {
    reovim_driver_text_session::SessionExtension, reovim_driver_text_syntax::FoldRange,
    reovim_kernel::api::v1::BufferId,
};

/// Per-buffer fold tracking.
///
/// Stores fold ranges (from syntax driver) and tracks which are collapsed.
/// Ranges are sorted by `start_line` for consistent indexing.
#[derive(Debug, Default, Clone)]
pub struct FoldState {
    /// Sorted fold ranges for this buffer.
    ranges: Vec<FoldRange>,
    /// Indices into `ranges` that are currently collapsed.
    collapsed: HashSet<usize>,
}

impl FoldState {
    /// Create a new empty fold state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set fold ranges (from syntax driver).
    ///
    /// Clears collapsed state since range indices may have changed.
    /// Ranges are sorted by `start_line`.
    pub fn set_ranges(&mut self, mut ranges: Vec<FoldRange>) {
        ranges.sort_by_key(|r| r.start_line);
        self.ranges = ranges;
        self.collapsed.clear();
    }

    /// Get the fold ranges.
    #[must_use]
    pub fn ranges(&self) -> &[FoldRange] {
        &self.ranges
    }

    /// Find the innermost fold containing the given line.
    ///
    /// Returns the index into `ranges` of the narrowest fold that
    /// contains `line`. For nested folds, always returns the innermost.
    #[must_use]
    pub fn fold_at_line(&self, line: u32) -> Option<usize> {
        let mut best: Option<usize> = None;
        let mut best_span = u32::MAX;

        for (i, range) in self.ranges.iter().enumerate() {
            if range.contains_line(line) {
                let span = range.end_line - range.start_line;
                if span < best_span {
                    best = Some(i);
                    best_span = span;
                }
            }
        }

        best
    }

    /// Toggle fold at given index.
    ///
    /// If collapsed, opens it. If open, collapses it.
    pub fn toggle(&mut self, index: usize) {
        if index >= self.ranges.len() {
            return;
        }
        if self.collapsed.contains(&index) {
            self.collapsed.remove(&index);
        } else {
            self.collapsed.insert(index);
        }
    }

    /// Open (expand) fold at given index.
    pub fn open(&mut self, index: usize) {
        self.collapsed.remove(&index);
    }

    /// Close (collapse) fold at given index.
    pub fn close(&mut self, index: usize) {
        if index < self.ranges.len() {
            self.collapsed.insert(index);
        }
    }

    /// Open all folds.
    pub fn open_all(&mut self) {
        self.collapsed.clear();
    }

    /// Close all folds.
    pub fn close_all(&mut self) {
        self.collapsed = (0..self.ranges.len()).collect();
    }

    /// Check if a fold at the given index is collapsed.
    #[must_use]
    pub fn is_collapsed(&self, index: usize) -> bool {
        self.collapsed.contains(&index)
    }

    /// Check if a line is hidden by a collapsed fold.
    ///
    /// A line is hidden if it's inside a collapsed fold but is NOT the
    /// fold's start line (the start line shows the fold marker).
    #[must_use]
    pub fn is_line_hidden(&self, line: u32) -> bool {
        for &idx in &self.collapsed {
            let range = &self.ranges[idx];
            if line > range.start_line && line <= range.end_line {
                return true;
            }
        }
        false
    }

    /// Get fold marker info for a line.
    ///
    /// Returns `Some((hidden_count, preview))` if the line is the start
    /// of a collapsed fold. The fold marker replaces this line in display.
    #[must_use]
    pub fn get_fold_marker(&self, line: u32) -> Option<(u32, &str)> {
        for &idx in &self.collapsed {
            let range = &self.ranges[idx];
            if range.start_line == line {
                return Some((range.hidden_lines(), &range.preview));
            }
        }
        None
    }

    /// Check if any folds are collapsed.
    #[must_use]
    pub fn has_collapsed(&self) -> bool {
        !self.collapsed.is_empty()
    }

    /// Iterate over collapsed folds, yielding `(start_line, hidden_count, &preview)`.
    pub fn collapsed_info(&self) -> impl Iterator<Item = (u32, u32, &str)> {
        self.collapsed.iter().filter_map(|&idx| {
            self.ranges
                .get(idx)
                .map(|r| (r.start_line, r.hidden_lines(), r.preview.as_str()))
        })
    }
}

/// Per-session fold state stored in `ExtensionMap`.
///
/// Tracks fold state for each buffer independently. This is a `Shared`
/// extension (accessible to all clients in the session).
#[derive(Debug, Default)]
pub struct FoldSessionState {
    /// Per-buffer fold state.
    states: HashMap<usize, FoldState>,
}

impl SessionExtension for FoldSessionState {
    fn create() -> Self {
        Self::default()
    }
}

impl FoldSessionState {
    /// Create a new empty fold session state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get or create fold state for a buffer.
    pub fn get_or_insert(&mut self, buffer_id: BufferId) -> &mut FoldState {
        self.states.entry(buffer_id.as_usize()).or_default()
    }

    /// Get fold state for a buffer (immutable).
    #[must_use]
    pub fn get(&self, buffer_id: BufferId) -> Option<&FoldState> {
        self.states.get(&buffer_id.as_usize())
    }

    /// Check if any buffer has collapsed folds.
    #[must_use]
    pub fn has_collapsed_folds(&self) -> bool {
        self.states.values().any(FoldState::has_collapsed)
    }

    /// Iterate over all `(buffer_id, &FoldState)` pairs.
    pub fn buffers(&self) -> impl Iterator<Item = (usize, &FoldState)> + '_ {
        self.states.iter().map(|(&id, state)| (id, state))
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
