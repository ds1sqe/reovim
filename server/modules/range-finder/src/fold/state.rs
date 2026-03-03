//! Per-buffer fold state management.
//!
//! Tracks which foldable regions are collapsed. Fold ranges come from
//! `SyntaxDriver::folds()`; this module only manages the collapsed/open
//! status of each fold.

use std::collections::{HashMap, HashSet};

use {
    reovim_driver_session::SessionExtension, reovim_driver_syntax::FoldRange,
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
mod tests {
    use {super::*, reovim_driver_syntax::FoldKind};

    fn make_ranges() -> Vec<FoldRange> {
        vec![
            FoldRange::new(2, 10, FoldKind::Class, "impl Foo {"),
            FoldRange::new(3, 6, FoldKind::Function, "fn bar() {"),
            FoldRange::new(7, 9, FoldKind::Function, "fn baz() {"),
        ]
    }

    // ========================================================================
    // FoldState tests
    // ========================================================================

    #[test]
    fn test_fold_state_default_empty() {
        let state = FoldState::new();
        assert!(state.ranges().is_empty());
        assert!(!state.has_collapsed());
    }

    #[test]
    fn test_set_ranges_clears_collapsed() {
        let mut state = FoldState::new();
        state.set_ranges(make_ranges());
        state.close(0);
        assert!(state.has_collapsed());

        // Setting new ranges clears collapsed
        state.set_ranges(make_ranges());
        assert!(!state.has_collapsed());
    }

    #[test]
    fn test_set_ranges_sorts_by_start_line() {
        let mut state = FoldState::new();
        // Give ranges out of order
        let ranges = vec![
            FoldRange::new(7, 9, FoldKind::Function, "fn baz() {"),
            FoldRange::new(2, 10, FoldKind::Class, "impl Foo {"),
            FoldRange::new(3, 6, FoldKind::Function, "fn bar() {"),
        ];
        state.set_ranges(ranges);

        // Should be sorted by start_line
        assert_eq!(state.ranges()[0].start_line, 2);
        assert_eq!(state.ranges()[1].start_line, 3);
        assert_eq!(state.ranges()[2].start_line, 7);
    }

    #[test]
    fn test_fold_at_line_exact_start() {
        let mut state = FoldState::new();
        state.set_ranges(make_ranges());

        // Line 3 is the start of fn bar (index 1)
        let idx = state.fold_at_line(3).unwrap();
        assert_eq!(idx, 1);
    }

    #[test]
    fn test_fold_at_line_inside() {
        let mut state = FoldState::new();
        state.set_ranges(make_ranges());

        // Line 5 is inside fn bar (index 1), also inside impl Foo (index 0)
        // Should return innermost (fn bar, smaller span)
        let idx = state.fold_at_line(5).unwrap();
        assert_eq!(idx, 1);
    }

    #[test]
    fn test_fold_at_line_outside() {
        let mut state = FoldState::new();
        state.set_ranges(make_ranges());

        // Line 0 is outside all folds
        assert!(state.fold_at_line(0).is_none());
        // Line 11 is outside all folds
        assert!(state.fold_at_line(11).is_none());
    }

    #[test]
    fn test_fold_at_line_innermost_nested() {
        let mut state = FoldState::new();
        state.set_ranges(make_ranges());

        // Line 8 is inside fn baz (index 2) and impl Foo (index 0)
        // fn baz spans 7-9 (3 lines), impl Foo spans 2-10 (9 lines)
        let idx = state.fold_at_line(8).unwrap();
        assert_eq!(idx, 2); // innermost
    }

    #[test]
    fn test_fold_at_line_wider_after_narrower() {
        let mut state = FoldState::new();
        // Two folds with the same start_line: narrow first, wide second.
        // When sorted by start_line (stable sort), narrow comes first.
        let ranges = vec![
            FoldRange::new(2, 4, FoldKind::Function, "fn narrow() {"),
            FoldRange::new(2, 10, FoldKind::Class, "impl Wide {"),
        ];
        state.set_ranges(ranges);

        // Line 3 is in both folds. Narrow checked first (span=2, best_span=2),
        // then wide (span=8, 8 < 2 is false → branch not taken).
        let idx = state.fold_at_line(3).unwrap();
        assert_eq!(idx, 0); // narrow fold wins
    }

    #[test]
    fn test_get_fold_marker_mismatched_collapsed() {
        let mut state = FoldState::new();
        state.set_ranges(make_ranges());

        // Collapse both fn bar (index 1, start=3) and fn baz (index 2, start=7)
        state.close(1);
        state.close(2);

        // Query line 3: matches fold at index 1, but index 2 has start_line=7 ≠ 3 (false branch)
        let (hidden, preview) = state.get_fold_marker(3).unwrap();
        assert_eq!(hidden, 3);
        assert_eq!(preview, "fn bar() {");

        // Query line 7: matches fold at index 2, but index 1 has start_line=3 ≠ 7 (false branch)
        let (hidden, preview) = state.get_fold_marker(7).unwrap();
        assert_eq!(hidden, 2);
        assert_eq!(preview, "fn baz() {");
    }

    #[test]
    fn test_toggle_collapses() {
        let mut state = FoldState::new();
        state.set_ranges(make_ranges());

        assert!(!state.is_collapsed(0));
        state.toggle(0);
        assert!(state.is_collapsed(0));
    }

    #[test]
    fn test_toggle_expands() {
        let mut state = FoldState::new();
        state.set_ranges(make_ranges());

        state.close(0);
        assert!(state.is_collapsed(0));
        state.toggle(0);
        assert!(!state.is_collapsed(0));
    }

    #[test]
    fn test_toggle_out_of_bounds() {
        let mut state = FoldState::new();
        state.set_ranges(make_ranges());

        // Should not panic
        state.toggle(999);
        assert!(!state.has_collapsed());
    }

    #[test]
    fn test_open_already_open() {
        let mut state = FoldState::new();
        state.set_ranges(make_ranges());

        // Already open, open is no-op
        state.open(0);
        assert!(!state.is_collapsed(0));
    }

    #[test]
    fn test_open_collapsed() {
        let mut state = FoldState::new();
        state.set_ranges(make_ranges());

        state.close(0);
        assert!(state.is_collapsed(0));
        state.open(0);
        assert!(!state.is_collapsed(0));
    }

    #[test]
    fn test_close_already_closed() {
        let mut state = FoldState::new();
        state.set_ranges(make_ranges());

        state.close(0);
        assert!(state.is_collapsed(0));
        // Close again is no-op
        state.close(0);
        assert!(state.is_collapsed(0));
    }

    #[test]
    fn test_close_open() {
        let mut state = FoldState::new();
        state.set_ranges(make_ranges());

        assert!(!state.is_collapsed(0));
        state.close(0);
        assert!(state.is_collapsed(0));
    }

    #[test]
    fn test_close_out_of_bounds() {
        let mut state = FoldState::new();
        state.set_ranges(make_ranges());

        // Should not panic or add invalid index
        state.close(999);
        assert!(!state.has_collapsed());
    }

    #[test]
    fn test_open_all() {
        let mut state = FoldState::new();
        state.set_ranges(make_ranges());

        state.close_all();
        assert!(state.has_collapsed());
        state.open_all();
        assert!(!state.has_collapsed());
    }

    #[test]
    fn test_close_all() {
        let mut state = FoldState::new();
        state.set_ranges(make_ranges());

        state.close_all();
        assert!(state.is_collapsed(0));
        assert!(state.is_collapsed(1));
        assert!(state.is_collapsed(2));
    }

    #[test]
    fn test_is_collapsed() {
        let mut state = FoldState::new();
        state.set_ranges(make_ranges());

        assert!(!state.is_collapsed(0));
        state.close(0);
        assert!(state.is_collapsed(0));
        assert!(!state.is_collapsed(1));
    }

    #[test]
    fn test_is_line_hidden_inside_collapsed() {
        let mut state = FoldState::new();
        state.set_ranges(make_ranges());

        // Collapse fn bar (index 1, lines 3-6)
        state.close(1);

        // Lines 4, 5, 6 are hidden (inside collapsed fold, not start line)
        assert!(state.is_line_hidden(4));
        assert!(state.is_line_hidden(5));
        assert!(state.is_line_hidden(6));
    }

    #[test]
    fn test_is_line_hidden_outside() {
        let mut state = FoldState::new();
        state.set_ranges(make_ranges());

        // Collapse fn bar (index 1, lines 3-6)
        state.close(1);

        // Line 2 is outside
        assert!(!state.is_line_hidden(2));
        // Line 7 is outside fn bar
        assert!(!state.is_line_hidden(7));
    }

    #[test]
    fn test_is_line_hidden_fold_start() {
        let mut state = FoldState::new();
        state.set_ranges(make_ranges());

        // Collapse fn bar (index 1, lines 3-6)
        state.close(1);

        // Line 3 is start of fold - visible as fold marker
        assert!(!state.is_line_hidden(3));
    }

    #[test]
    fn test_get_fold_marker_collapsed() {
        let mut state = FoldState::new();
        state.set_ranges(make_ranges());

        // Collapse fn bar (index 1, lines 3-6)
        state.close(1);

        let (hidden, preview) = state.get_fold_marker(3).unwrap();
        assert_eq!(hidden, 3); // lines 4, 5, 6 hidden
        assert_eq!(preview, "fn bar() {");
    }

    #[test]
    fn test_get_fold_marker_open() {
        let mut state = FoldState::new();
        state.set_ranges(make_ranges());

        // fn bar is open, no marker
        assert!(state.get_fold_marker(3).is_none());
    }

    #[test]
    fn test_get_fold_marker_no_fold() {
        let mut state = FoldState::new();
        state.set_ranges(make_ranges());

        // Line 0 has no fold
        assert!(state.get_fold_marker(0).is_none());
    }

    #[test]
    fn test_nested_fold_independent_collapse() {
        let mut state = FoldState::new();
        state.set_ranges(make_ranges());

        // Collapse inner fold fn bar (index 1, lines 3-6)
        // Keep outer fold impl Foo (index 0, lines 2-10) open
        state.close(1);

        // Lines 4-6 hidden (inside collapsed fn bar)
        assert!(state.is_line_hidden(4));
        assert!(state.is_line_hidden(5));
        assert!(state.is_line_hidden(6));

        // Lines 7-10 visible (inside open impl Foo, outside fn bar)
        assert!(!state.is_line_hidden(7));
        assert!(!state.is_line_hidden(8));
        assert!(!state.is_line_hidden(10));
    }

    // ========================================================================
    // FoldSessionState tests
    // ========================================================================

    fn buffer_id(n: usize) -> BufferId {
        BufferId::from_raw(n)
    }

    #[test]
    fn test_session_extension_create() {
        let state = FoldSessionState::create();
        assert!(!state.has_collapsed_folds());
    }

    #[test]
    fn test_per_buffer_isolation() {
        let mut state = FoldSessionState::new();
        let buf1 = buffer_id(1);
        let buf2 = buffer_id(2);

        let fold1 = state.get_or_insert(buf1);
        fold1.set_ranges(make_ranges());
        fold1.close(0);

        let fold2 = state.get_or_insert(buf2);
        fold2.set_ranges(vec![FoldRange::new(0, 5, FoldKind::Function, "fn main() {")]);

        // Buffer 1 has collapsed folds
        assert!(state.get(buf1).unwrap().has_collapsed());
        // Buffer 2 has none
        assert!(!state.get(buf2).unwrap().has_collapsed());
    }

    #[test]
    fn test_has_collapsed_folds() {
        let mut state = FoldSessionState::new();
        let buf = buffer_id(1);

        assert!(!state.has_collapsed_folds());

        let fold = state.get_or_insert(buf);
        fold.set_ranges(make_ranges());
        fold.close(0);

        assert!(state.has_collapsed_folds());
    }

    #[test]
    fn test_buffers_iterator() {
        let mut state = FoldSessionState::new();

        state.get_or_insert(buffer_id(1)).set_ranges(make_ranges());
        state.get_or_insert(buffer_id(2));

        assert_eq!(state.buffers().count(), 2);
    }

    #[test]
    fn test_get_nonexistent_buffer() {
        let state = FoldSessionState::new();
        assert!(state.get(buffer_id(999)).is_none());
    }
}
