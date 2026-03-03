//! Active snippet engine (#136).
//!
//! Manages tab stop tracking, position updates through buffer edits,
//! and cursor navigation for an expanded snippet.

use reovim_kernel::api::v1::{Edit, Position, transform_position};

use crate::ast::{SnippetBody, SnippetElement, TabStopId};

/// A resolved tab stop with its position range in the buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabStopRange {
    /// Tab stop number.
    pub id: TabStopId,
    /// Start position in the buffer.
    pub start: Position,
    /// End position in the buffer (equals start for plain `$N`).
    pub end: Position,
    /// Placeholder default text (empty for plain tab stops).
    pub placeholder: String,
}

/// An active snippet session tracking tab stop positions.
#[derive(Debug, Clone)]
pub struct ActiveSnippet {
    /// Tab stops sorted by navigation order: $1, $2, ..., $N, then $0.
    tab_stops: Vec<TabStopRange>,
    /// Index into `tab_stops` for the current tab stop.
    current_index: usize,
    /// Start of the entire snippet body in the buffer.
    snippet_start: Position,
    /// End of the entire snippet body in the buffer.
    snippet_end: Position,
}

impl ActiveSnippet {
    /// Expand a parsed snippet body into text and create an `ActiveSnippet`.
    ///
    /// Returns the expanded text string and the snippet tracker with
    /// tab stop positions computed relative to `insert_pos`.
    #[must_use]
    pub fn expand(body: &SnippetBody, insert_pos: Position) -> (String, Self) {
        let mut text = String::new();
        let mut raw_stops: Vec<TabStopRange> = Vec::new();

        // Track current position as we build the text
        let mut line = insert_pos.line;
        let mut col = insert_pos.column;

        collect_tab_stops(body.elements(), &mut text, &mut raw_stops, &mut line, &mut col);

        // Sort: numbered stops (1, 2, ..., N) first, then $0 last
        raw_stops.sort_by_key(|ts| if ts.id == 0 { (1, 0) } else { (0, ts.id) });

        // Compute snippet end position
        let snippet_end = Position::new(line, col);

        let snippet = Self {
            tab_stops: raw_stops,
            current_index: 0,
            snippet_start: insert_pos,
            snippet_end,
        };

        (text, snippet)
    }

    /// Get the current tab stop, if any.
    #[must_use]
    pub fn current(&self) -> Option<&TabStopRange> {
        self.tab_stops.get(self.current_index)
    }

    /// Advance to the next tab stop. Returns the new current tab stop,
    /// or `None` if all tab stops have been visited.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<&TabStopRange> {
        if self.current_index + 1 < self.tab_stops.len() {
            self.current_index += 1;
            self.tab_stops.get(self.current_index)
        } else {
            // Mark as done by advancing index past end
            if !self.tab_stops.is_empty() {
                self.current_index = self.tab_stops.len();
            }
            None
        }
    }

    /// Go back to the previous tab stop. Returns the new current tab stop,
    /// or `None` if already at the first tab stop.
    pub fn prev(&mut self) -> Option<&TabStopRange> {
        if self.current_index > 0 {
            self.current_index -= 1;
            self.tab_stops.get(self.current_index)
        } else {
            None
        }
    }

    /// Update all tab stop positions based on a buffer edit.
    ///
    /// Uses `transform_position` from the kernel to shift positions
    /// as text is inserted or deleted around them.
    pub fn update_positions(&mut self, edit: &Edit) {
        for stop in &mut self.tab_stops {
            stop.start = transform_position(stop.start, edit);
            stop.end = transform_position(stop.end, edit);
        }
        self.snippet_start = transform_position(self.snippet_start, edit);
        self.snippet_end = transform_position(self.snippet_end, edit);
    }

    /// Check if the cursor is within the snippet body range.
    ///
    /// Returns `false` if cursor has moved outside the snippet,
    /// indicating the session should be cancelled.
    #[must_use]
    pub const fn is_valid(&self, cursor: Position) -> bool {
        // cursor >= snippet_start && cursor <= snippet_end
        let after_start = cursor.line > self.snippet_start.line
            || (cursor.line == self.snippet_start.line
                && cursor.column >= self.snippet_start.column);
        let before_end = cursor.line < self.snippet_end.line
            || (cursor.line == self.snippet_end.line && cursor.column <= self.snippet_end.column);
        after_start && before_end
    }

    /// Check if all tab stops have been visited (including $0).
    #[must_use]
    pub const fn is_done(&self) -> bool {
        self.tab_stops.is_empty() || self.current_index >= self.tab_stops.len()
    }

    /// Get the number of tab stops.
    #[must_use]
    pub const fn tab_stop_count(&self) -> usize {
        self.tab_stops.len()
    }

    /// Get all tab stops.
    #[must_use]
    pub fn tab_stops(&self) -> &[TabStopRange] {
        &self.tab_stops
    }

    /// Get the snippet start position.
    #[must_use]
    pub const fn snippet_start(&self) -> Position {
        self.snippet_start
    }

    /// Get the snippet end position.
    #[must_use]
    pub const fn snippet_end(&self) -> Position {
        self.snippet_end
    }
}

/// Recursively collect tab stops from snippet elements, building the
/// expanded text string and recording positions.
fn collect_tab_stops(
    elements: &[SnippetElement],
    text: &mut String,
    stops: &mut Vec<TabStopRange>,
    line: &mut usize,
    col: &mut usize,
) {
    for elem in elements {
        match elem {
            SnippetElement::Text(s) => {
                append_text(text, s, line, col);
            }
            SnippetElement::TabStop { id, .. } => {
                let pos = Position::new(*line, *col);
                stops.push(TabStopRange {
                    id: *id,
                    start: pos,
                    end: pos,
                    placeholder: String::new(),
                });
            }
            SnippetElement::Placeholder { id, body } => {
                let start = Position::new(*line, *col);
                // Render the placeholder body text
                let body_start = text.len();
                collect_tab_stops(body, text, stops, line, col);
                let placeholder_text = text[body_start..].to_string();
                let end = Position::new(*line, *col);
                stops.push(TabStopRange {
                    id: *id,
                    start,
                    end,
                    placeholder: placeholder_text,
                });
            }
            // Phase 3+: Variable, Choice handled here
            SnippetElement::Variable { .. } | SnippetElement::Choice { .. } => {
                // Unimplemented variants: emit nothing
            }
        }
    }
}

/// Append text, tracking line/column position updates for newlines.
fn append_text(text: &mut String, s: &str, line: &mut usize, col: &mut usize) {
    for ch in s.chars() {
        text.push(ch);
        if ch == '\n' {
            *line += 1;
            *col = 0;
        } else {
            *col += 1;
        }
    }
}

#[cfg(test)]
#[allow(clippy::literal_string_with_formatting_args)]
mod tests {
    use {super::*, crate::parser};

    /// Helper: parse and expand a snippet at a given position.
    fn expand_at(input: &str, pos: Position) -> (String, ActiveSnippet) {
        let body = parser::parse(input).unwrap();
        ActiveSnippet::expand(&body, pos)
    }

    // =========================================================================
    // Basic expansion
    // =========================================================================

    #[test]
    fn test_expand_plain_text() {
        let (text, snippet) = expand_at("hello world", Position::origin());
        assert_eq!(text, "hello world");
        assert!(snippet.tab_stops.is_empty());
        assert!(snippet.is_done());
    }

    #[test]
    fn test_expand_single_tabstop() {
        let (text, snippet) = expand_at("fn $1()", Position::origin());
        assert_eq!(text, "fn ()");
        assert_eq!(snippet.tab_stop_count(), 1);
        let ts = snippet.current().unwrap();
        assert_eq!(ts.id, 1);
        assert_eq!(ts.start, Position::new(0, 3));
        assert_eq!(ts.end, Position::new(0, 3));
        assert!(ts.placeholder.is_empty());
    }

    #[test]
    fn test_expand_multiple_tabstops() {
        let (text, snippet) = expand_at("$1 $2 $0", Position::origin());
        assert_eq!(text, "  ");
        assert_eq!(snippet.tab_stop_count(), 3);
        // $1, $2, then $0
        assert_eq!(snippet.tab_stops()[0].id, 1);
        assert_eq!(snippet.tab_stops()[1].id, 2);
        assert_eq!(snippet.tab_stops()[2].id, 0);
    }

    #[test]
    fn test_expand_with_offset() {
        let (text, snippet) = expand_at("$1", Position::new(5, 10));
        assert_eq!(text, "");
        let ts = snippet.current().unwrap();
        assert_eq!(ts.start, Position::new(5, 10));
    }

    #[test]
    fn test_expand_placeholder() {
        let (text, snippet) = expand_at("${1:name}", Position::origin());
        assert_eq!(text, "name");
        assert_eq!(snippet.tab_stop_count(), 1);
        let ts = snippet.current().unwrap();
        assert_eq!(ts.id, 1);
        assert_eq!(ts.start, Position::origin());
        assert_eq!(ts.end, Position::new(0, 4));
        assert_eq!(ts.placeholder, "name");
    }

    #[test]
    fn test_expand_placeholder_with_text() {
        let (text, snippet) = expand_at("fn ${1:name}()", Position::origin());
        assert_eq!(text, "fn name()");
        let ts = snippet.current().unwrap();
        assert_eq!(ts.id, 1);
        assert_eq!(ts.start, Position::new(0, 3));
        assert_eq!(ts.end, Position::new(0, 7));
    }

    #[test]
    fn test_expand_multiline() {
        let (text, snippet) = expand_at("fn $1() {\n\t$0\n}", Position::origin());
        assert_eq!(text, "fn () {\n\t\n}");
        assert_eq!(snippet.snippet_end, Position::new(2, 1));
    }

    #[test]
    fn test_expand_nested_placeholder() {
        let (text, snippet) = expand_at("${1:outer ${2:inner}}", Position::origin());
        assert_eq!(text, "outer inner");
        assert_eq!(snippet.tab_stop_count(), 2);
        // $1 comes first (sorted by id), then $2
        assert_eq!(snippet.tab_stops()[0].id, 1);
        assert_eq!(snippet.tab_stops()[1].id, 2);
    }

    // =========================================================================
    // Navigation: next / prev
    // =========================================================================

    #[test]
    fn test_next_advances() {
        let (_, mut snippet) = expand_at("$1 $2 $0", Position::origin());
        assert_eq!(snippet.current().unwrap().id, 1);
        assert_eq!(snippet.next().unwrap().id, 2);
        assert_eq!(snippet.next().unwrap().id, 0);
        assert!(snippet.next().is_none());
    }

    #[test]
    fn test_prev_goes_back() {
        let (_, mut snippet) = expand_at("$1 $2 $0", Position::origin());
        snippet.next(); // → $2
        snippet.next(); // → $0
        assert_eq!(snippet.prev().unwrap().id, 2);
        assert_eq!(snippet.prev().unwrap().id, 1);
        assert!(snippet.prev().is_none()); // already at first
    }

    #[test]
    fn test_prev_at_first_returns_none() {
        let (_, mut snippet) = expand_at("$1 $2", Position::origin());
        assert!(snippet.prev().is_none());
    }

    #[test]
    fn test_next_exhausted_is_done() {
        let (_, mut snippet) = expand_at("$1", Position::origin());
        assert!(!snippet.is_done());
        snippet.next(); // past end
        assert!(snippet.is_done());
    }

    #[test]
    fn test_no_tabstops_is_done() {
        let (_, snippet) = expand_at("plain text", Position::origin());
        assert!(snippet.is_done());
    }

    // =========================================================================
    // Position tracking: update_positions
    // =========================================================================

    #[test]
    fn test_update_positions_insert_before() {
        let (_, mut snippet) = expand_at("$1", Position::new(0, 5));
        // Insert 3 chars before the tab stop on same line
        let edit = Edit::insert(Position::new(0, 2), "abc");
        snippet.update_positions(&edit);
        assert_eq!(snippet.current().unwrap().start, Position::new(0, 8));
    }

    #[test]
    fn test_update_positions_insert_after() {
        let (_, mut snippet) = expand_at("$1", Position::new(0, 5));
        // Insert after the tab stop: no change
        let edit = Edit::insert(Position::new(0, 10), "xyz");
        snippet.update_positions(&edit);
        assert_eq!(snippet.current().unwrap().start, Position::new(0, 5));
    }

    #[test]
    fn test_update_positions_insert_newline() {
        let (_, mut snippet) = expand_at("$1", Position::new(0, 5));
        // Insert newline before
        let edit = Edit::insert(Position::new(0, 2), "a\nb");
        snippet.update_positions(&edit);
        // Position shifts down by 1 line
        assert_eq!(snippet.current().unwrap().start, Position::new(1, 4));
    }

    #[test]
    fn test_update_positions_delete() {
        let (_, mut snippet) = expand_at("$1", Position::new(0, 5));
        // Delete 2 chars before the tab stop
        let edit = Edit::delete(Position::new(0, 2), "ab");
        snippet.update_positions(&edit);
        assert_eq!(snippet.current().unwrap().start, Position::new(0, 3));
    }

    #[test]
    fn test_update_positions_updates_snippet_bounds() {
        let (_, mut snippet) = expand_at("hello $1 world", Position::origin());
        let orig_end = snippet.snippet_end;
        let edit = Edit::insert(Position::origin(), "prefix ");
        snippet.update_positions(&edit);
        // Snippet start and end should both shift
        assert!(snippet.snippet_start.column > 0);
        assert!(snippet.snippet_end.column > orig_end.column);
    }

    // =========================================================================
    // Validity: is_valid
    // =========================================================================

    #[test]
    fn test_is_valid_at_start() {
        let (_, snippet) = expand_at("hello $1", Position::origin());
        assert!(snippet.is_valid(Position::origin()));
    }

    #[test]
    fn test_is_valid_at_end() {
        let (_, snippet) = expand_at("hello", Position::origin());
        assert!(snippet.is_valid(snippet.snippet_end));
    }

    #[test]
    fn test_is_valid_in_middle() {
        let (_, snippet) = expand_at("hello world", Position::origin());
        assert!(snippet.is_valid(Position::new(0, 5)));
    }

    #[test]
    fn test_is_valid_before_start() {
        let (_, snippet) = expand_at("hello", Position::new(0, 5));
        assert!(!snippet.is_valid(Position::new(0, 3)));
    }

    #[test]
    fn test_is_valid_after_end() {
        let (_, snippet) = expand_at("hi", Position::origin());
        assert!(!snippet.is_valid(Position::new(0, 100)));
    }

    #[test]
    fn test_is_valid_different_line_before() {
        let (_, snippet) = expand_at("hello", Position::new(5, 0));
        assert!(!snippet.is_valid(Position::new(4, 0)));
    }

    #[test]
    fn test_is_valid_different_line_after() {
        let (_, snippet) = expand_at("hello", Position::new(0, 0));
        assert!(!snippet.is_valid(Position::new(1, 0)));
    }

    #[test]
    fn test_is_valid_multiline() {
        let (_, snippet) = expand_at("line1\nline2\nline3", Position::origin());
        assert!(snippet.is_valid(Position::new(1, 3)));
        assert!(!snippet.is_valid(Position::new(3, 0)));
    }

    // =========================================================================
    // Accessors
    // =========================================================================

    #[test]
    fn test_tab_stops_accessor() {
        let (_, snippet) = expand_at("$1 $2", Position::origin());
        assert_eq!(snippet.tab_stops().len(), 2);
    }

    #[test]
    fn test_snippet_start_accessor() {
        let pos = Position::new(3, 7);
        let (_, snippet) = expand_at("hello", pos);
        assert_eq!(snippet.snippet_start(), pos);
    }

    #[test]
    fn test_snippet_end_accessor() {
        let (_, snippet) = expand_at("hi", Position::origin());
        assert_eq!(snippet.snippet_end(), Position::new(0, 2));
    }

    // =========================================================================
    // TabStopRange
    // =========================================================================

    #[test]
    fn test_tabstop_range_clone() {
        let range = TabStopRange {
            id: 1,
            start: Position::origin(),
            end: Position::new(0, 5),
            placeholder: "hello".to_string(),
        };
        let cloned = range.clone();
        assert_eq!(range, cloned);
    }

    #[test]
    fn test_tabstop_range_debug() {
        let range = TabStopRange {
            id: 0,
            start: Position::origin(),
            end: Position::origin(),
            placeholder: String::new(),
        };
        let debug = format!("{range:?}");
        assert!(debug.contains("TabStopRange"));
    }

    // =========================================================================
    // ActiveSnippet clone/debug
    // =========================================================================

    #[test]
    fn test_active_snippet_clone() {
        let (_, snippet) = expand_at("$1", Position::origin());
        let cloned = snippet.clone();
        assert_eq!(cloned.tab_stop_count(), snippet.tab_stop_count());
    }

    #[test]
    fn test_active_snippet_debug() {
        let (_, snippet) = expand_at("$1", Position::origin());
        let debug = format!("{snippet:?}");
        assert!(debug.contains("ActiveSnippet"));
    }

    // =========================================================================
    // Tab stop ordering
    // =========================================================================

    #[test]
    fn test_tabstop_ordering_with_zero() {
        let (_, snippet) = expand_at("$2 $1 $0", Position::origin());
        // Should be sorted: $1, $2, $0
        assert_eq!(snippet.tab_stops()[0].id, 1);
        assert_eq!(snippet.tab_stops()[1].id, 2);
        assert_eq!(snippet.tab_stops()[2].id, 0);
    }

    #[test]
    fn test_tabstop_ordering_no_zero() {
        let (_, snippet) = expand_at("$3 $1 $2", Position::origin());
        assert_eq!(snippet.tab_stops()[0].id, 1);
        assert_eq!(snippet.tab_stops()[1].id, 2);
        assert_eq!(snippet.tab_stops()[2].id, 3);
    }

    #[test]
    fn test_tabstop_ordering_multi_digit() {
        let (_, snippet) = expand_at("${10} ${1} ${5}", Position::origin());
        assert_eq!(snippet.tab_stops()[0].id, 1);
        assert_eq!(snippet.tab_stops()[1].id, 5);
        assert_eq!(snippet.tab_stops()[2].id, 10);
    }

    // =========================================================================
    // Realistic snippet
    // =========================================================================

    #[test]
    fn test_function_snippet() {
        let (text, snippet) =
            expand_at("fn ${1:name}(${2:params}) -> ${3:Type} {\n\t$0\n}", Position::origin());
        assert_eq!(text, "fn name(params) -> Type {\n\t\n}");
        assert_eq!(snippet.tab_stop_count(), 4);
        // $1, $2, $3, $0
        assert_eq!(snippet.tab_stops()[0].id, 1);
        assert_eq!(snippet.tab_stops()[1].id, 2);
        assert_eq!(snippet.tab_stops()[2].id, 3);
        assert_eq!(snippet.tab_stops()[3].id, 0);
    }

    #[test]
    fn test_function_snippet_positions() {
        let (_, snippet) =
            expand_at("fn ${1:name}(${2:params}) -> ${3:Type} {\n\t$0\n}", Position::origin());
        // $1: "name" at col 3..7
        assert_eq!(snippet.tab_stops()[0].start, Position::new(0, 3));
        assert_eq!(snippet.tab_stops()[0].end, Position::new(0, 7));
        // $2: "params" at col 8..14
        assert_eq!(snippet.tab_stops()[1].start, Position::new(0, 8));
        assert_eq!(snippet.tab_stops()[1].end, Position::new(0, 14));
        // $3: "Type" at col 19..23
        assert_eq!(snippet.tab_stops()[2].start, Position::new(0, 19));
        assert_eq!(snippet.tab_stops()[2].end, Position::new(0, 23));
        // $0: at line 1, col 1 (after \t)
        assert_eq!(snippet.tab_stops()[3].start, Position::new(1, 1));
    }

    // =========================================================================
    // Edge: expand with empty body
    // =========================================================================

    #[test]
    fn test_expand_empty_body() {
        let body = SnippetBody::new(vec![]);
        let (text, snippet) = ActiveSnippet::expand(&body, Position::origin());
        assert!(text.is_empty());
        assert!(snippet.is_done());
        assert_eq!(snippet.snippet_start(), Position::origin());
        assert_eq!(snippet.snippet_end(), Position::origin());
    }
}
