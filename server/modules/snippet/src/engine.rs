//! Active snippet engine (#136).
//!
//! Manages tab stop tracking, position updates through buffer edits,
//! and cursor navigation for an expanded snippet.

use reovim_domain_text::{Edit, Position, transform_position};

use crate::{
    ast::{SnippetBody, SnippetElement, TabStopId},
    transform::apply_transform,
    variables::{VariableContext, resolve_variable},
};

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
    ///
    /// The `var_ctx` provides context for resolving built-in variables
    /// like `$TM_FILENAME` or `$CURRENT_YEAR` during expansion.
    #[must_use]
    pub fn expand(
        body: &SnippetBody,
        insert_pos: Position,
        var_ctx: &VariableContext,
    ) -> (String, Self) {
        let mut text = String::new();
        let mut raw_stops: Vec<TabStopRange> = Vec::new();

        // Track current position as we build the text
        let mut line = insert_pos.line;
        let mut col = insert_pos.column;

        collect_tab_stops(body.elements(), &mut text, &mut raw_stops, &mut line, &mut col, var_ctx);

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

    /// Reconcile tab stop positions after user typing at the current stop.
    ///
    /// The insert-mode fallback handler bypasses the snippet engine when
    /// inserting characters, so subsequent tab stop positions become stale.
    /// Call this before navigating away from the current tab stop: it
    /// synthesizes an `Edit::insert` from the tab stop start to the actual
    /// cursor position and shifts all remaining positions accordingly.
    pub fn reconcile_typing(&mut self, cursor: Position) {
        let Some(ts) = self.tab_stops.get(self.current_index) else {
            return;
        };
        let start = ts.start;

        // Build a synthetic text whose dimensions match the cursor delta.
        // `transform_position` only cares about newline count and last-line
        // length, not the actual characters.
        let synthetic = match cursor.line.cmp(&start.line) {
            std::cmp::Ordering::Equal => {
                if cursor.column > start.column {
                    " ".repeat(cursor.column - start.column)
                } else {
                    return; // cursor didn't advance (or moved left via backspace)
                }
            }
            std::cmp::Ordering::Greater => {
                let newlines = cursor.line - start.line;
                let mut s = "\n".repeat(newlines);
                for _ in 0..cursor.column {
                    s.push(' ');
                }
                s
            }
            std::cmp::Ordering::Less => {
                return; // cursor moved above start line — unusual, bail
            }
        };

        let edit = Edit::insert(start, &synthetic);
        self.update_positions(&edit);
    }

    /// Get the current tab stop index.
    #[must_use]
    pub const fn current_index(&self) -> usize {
        self.current_index
    }

    /// Find indices of tab stops that share the same ID, excluding `exclude_index`.
    ///
    /// Used for mirroring: when the user leaves a tab stop, all other tab stops
    /// with the same ID are updated to match the typed text.
    #[must_use]
    pub fn mirror_indices(&self, id: TabStopId, exclude_index: usize) -> Vec<usize> {
        self.tab_stops
            .iter()
            .enumerate()
            .filter(|&(i, ts)| ts.id == id && i != exclude_index)
            .map(|(i, _)| i)
            .collect()
    }

    /// Update a tab stop's position range and placeholder text.
    ///
    /// Used after mirroring to reflect the new text at a mirrored tab stop.
    pub fn update_tab_stop(
        &mut self,
        index: usize,
        start: Position,
        end: Position,
        placeholder: String,
    ) {
        if let Some(ts) = self.tab_stops.get_mut(index) {
            ts.start = start;
            ts.end = end;
            ts.placeholder = placeholder;
        }
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
    var_ctx: &VariableContext,
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
                collect_tab_stops(body, text, stops, line, col, var_ctx);
                let placeholder_text = text[body_start..].to_string();
                let end = Position::new(*line, *col);
                stops.push(TabStopRange {
                    id: *id,
                    start,
                    end,
                    placeholder: placeholder_text,
                });
            }
            SnippetElement::Variable {
                name,
                default,
                transform,
            } => {
                // Try to resolve the variable.
                if let Some(value) = resolve_variable(name, var_ctx) {
                    // Apply transform if present.
                    let output = if let Some(t) = transform {
                        apply_transform(&value, t)
                    } else {
                        value
                    };
                    append_text(text, &output, line, col);
                } else if let Some(default_body) = default {
                    // Variable unknown: use default body.
                    collect_tab_stops(default_body, text, stops, line, col, var_ctx);
                }
                // If no value and no default, emit nothing.
            }
            SnippetElement::Choice { id, choices } => {
                // Insert first choice as default text, create a tab stop
                // spanning the choice text.
                let first = choices.first().map_or("", |s| s.as_str());
                let start = Position::new(*line, *col);
                append_text(text, first, line, col);
                let end = Position::new(*line, *col);
                stops.push(TabStopRange {
                    id: *id,
                    start,
                    end,
                    placeholder: first.to_owned(),
                });
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
#[path = "engine_tests.rs"]
mod tests;
