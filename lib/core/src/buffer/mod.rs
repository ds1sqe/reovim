//! Buffer module for text storage and manipulation

mod cursor;
mod history;
mod selection;
mod text;

#[cfg(test)]
mod tests;

pub use {
    cursor::{CursorOps, calculate_motion, calculate_motion_with_desired_col},
    history::{Change, UndoHistory},
    selection::{Selection, SelectionMode, SelectionOps},
    text::TextOps,
};

use crate::{
    motion::Motion,
    textobject::{TextObject, TextObjectScope},
};

use crate::screen::Position;

#[derive(Clone, Debug)]
pub struct Line {
    pub inner: String,
}

impl From<&str> for Line {
    fn from(value: &str) -> Self {
        Self {
            inner: value.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Buffer {
    pub id: usize,
    pub cur: Position,
    /// Track preferred column for vertical movement (j/k)
    /// Used to preserve horizontal position when moving through lines of different lengths
    pub desired_col: Option<u16>,
    pub contents: Vec<Line>,
    pub selection: Selection,
    pub file_path: Option<String>,
    /// Whether the buffer has unsaved modifications
    pub modified: bool,
    /// Undo/redo history
    history: UndoHistory,
    /// Changes accumulated during insert mode to be batched as single undo unit
    pending_batch: Vec<Change>,
    /// Whether batching is active (during insert mode)
    batching: bool,
}

impl Buffer {
    #[must_use]
    pub fn empty(id: usize) -> Self {
        Self {
            id,
            cur: Position { x: 0, y: 0 },
            desired_col: None,
            contents: Vec::new(),
            selection: Selection::default(),
            file_path: None,
            modified: false,
            history: UndoHistory::new(),
            pending_batch: Vec::new(),
            batching: false,
        }
    }

    /// Convert a position to a byte offset in the buffer content
    ///
    /// This is used for tree-sitter incremental parsing which needs byte offsets.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn position_to_byte_offset(&self, pos: Position) -> usize {
        let mut offset = 0;

        for (i, line) in self.contents.iter().enumerate() {
            if i < pos.y as usize {
                // Add full line length + newline
                offset += line.inner.len() + 1;
            } else if i == pos.y as usize {
                // Add characters up to position
                offset += (pos.x as usize).min(line.inner.len());
                break;
            }
        }

        offset
    }

    /// Convert a byte offset to a position in the buffer
    ///
    /// This is used for tree-sitter to convert byte ranges back to positions.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn byte_offset_to_position(&self, byte_offset: usize) -> Position {
        let mut remaining = byte_offset;

        for (y, line) in self.contents.iter().enumerate() {
            let line_len_with_newline = line.inner.len() + 1;
            if remaining < line_len_with_newline {
                return Position {
                    x: remaining.min(line.inner.len()) as u16,
                    y: y as u16,
                };
            }
            remaining -= line_len_with_newline;
        }

        // Past end of buffer
        let last_y = self.contents.len().saturating_sub(1);
        let last_x = self.contents.last().map_or(0, |l| l.inner.len());
        Position {
            x: last_x as u16,
            y: last_y as u16,
        }
    }

    /// Clear desired column (call on horizontal movements)
    #[allow(clippy::missing_const_for_fn)] // Modifies self
    pub fn clear_desired_col(&mut self) {
        self.desired_col = None;
    }

    /// Set desired column if not already set
    #[allow(clippy::missing_const_for_fn)] // Modifies self
    pub fn ensure_desired_col(&mut self) {
        if self.desired_col.is_none() {
            self.desired_col = Some(self.cur.x);
        }
    }

    /// Record a change for undo
    ///
    /// If batching is active (during insert mode), changes are accumulated
    /// and committed as a single undo unit when batching ends.
    pub fn record_change(&mut self, change: Change) {
        if self.batching {
            self.pending_batch.push(change);
        } else {
            self.history.push(change);
        }
        self.modified = true;
    }

    /// Begin batching changes (called when entering insert mode)
    ///
    /// All changes recorded while batching is active will be committed
    /// as a single undo unit when `flush_batch` is called.
    pub fn begin_batch(&mut self) {
        self.batching = true;
        self.pending_batch.clear();
    }

    /// Flush pending batch to history (called when leaving insert mode)
    ///
    /// Commits all accumulated changes as a single undo unit.
    /// Does nothing if no changes were recorded.
    #[allow(clippy::missing_panics_doc)] // Safe: we check len() == 1 before unwrap
    pub fn flush_batch(&mut self) {
        self.batching = false;
        if self.pending_batch.is_empty() {
            return;
        }
        let changes = std::mem::take(&mut self.pending_batch);
        if changes.len() == 1 {
            // Single change doesn't need wrapping
            self.history.push(changes.into_iter().next().unwrap());
        } else {
            self.history.push(Change::Batch(changes));
        }
    }

    /// Apply undo - returns true if successful
    #[allow(clippy::cast_possible_truncation)]
    pub fn apply_undo(&mut self) -> bool {
        let Some(change) = self.history.undo() else {
            return false;
        };
        self.apply_change(&change);
        true
    }

    /// Apply redo - returns true if successful
    #[allow(clippy::cast_possible_truncation)]
    pub fn apply_redo(&mut self) -> bool {
        let Some(change) = self.history.redo() else {
            return false;
        };
        self.apply_change(&change);
        true
    }

    /// Apply a change to the buffer (used by undo/redo)
    #[allow(clippy::cast_possible_truncation)]
    fn apply_change(&mut self, change: &Change) {
        match change {
            Change::Insert { pos, text } => {
                self.cur = *pos;
                self.insert_text_at_cursor(text);
            }
            Change::Delete { pos, text } => {
                self.delete_text_at(*pos, text.len());
            }
            Change::Batch(changes) => {
                for c in changes {
                    self.apply_change(c);
                }
            }
        }
    }

    /// Insert text at current cursor position (public method with history)
    ///
    /// Handles single-line and multi-line text insertion properly.
    /// Records the change for undo support.
    #[allow(clippy::cast_possible_truncation)]
    pub fn insert_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let pos = self.cur;
        self.insert_text_at_cursor(text);
        // Record as a single change for undo
        self.record_change(Change::Insert {
            pos,
            text: text.to_string(),
        });
    }

    /// Insert text at current cursor position (for undo/redo, no history)
    #[allow(clippy::cast_possible_truncation)]
    fn insert_text_at_cursor(&mut self, text: &str) {
        for c in text.chars() {
            if c == '\n' {
                self.insert_newline_internal();
            } else {
                self.insert_char_internal(c);
            }
        }
    }

    /// Insert a single character without recording history
    #[allow(clippy::cast_possible_truncation)]
    fn insert_char_internal(&mut self, c: char) {
        if self.contents.is_empty() {
            self.contents.push(Line::from(""));
        }
        if let Some(line) = self.contents.get_mut(self.cur.y as usize) {
            let x = self.cur.x as usize;
            if x <= line.inner.len() {
                line.inner.insert(x, c);
                self.cur.x += 1;
            }
        }
    }

    /// Insert newline without recording history
    #[allow(clippy::cast_possible_truncation)]
    fn insert_newline_internal(&mut self) {
        if self.contents.is_empty() {
            self.contents.push(Line::from(""));
        }
        let y = self.cur.y as usize;
        let x = self.cur.x as usize;

        if let Some(line) = self.contents.get_mut(y) {
            let rest = if x < line.inner.len() {
                line.inner.split_off(x)
            } else {
                String::new()
            };
            self.contents.insert(y + 1, Line { inner: rest });
        }
        self.cur.y += 1;
        self.cur.x = 0;
    }

    /// Delete text starting at position (for undo - handles multi-char/multi-line)
    #[allow(clippy::cast_possible_truncation)]
    fn delete_text_at(&mut self, pos: Position, len: usize) {
        self.cur = pos;
        for _ in 0..len {
            self.delete_char_at_cursor();
        }
    }

    /// Delete character at cursor without recording history
    #[allow(clippy::cast_possible_truncation)]
    fn delete_char_at_cursor(&mut self) {
        if let Some(line) = self.contents.get_mut(self.cur.y as usize) {
            let x = self.cur.x as usize;
            if x < line.inner.len() {
                line.inner.remove(x);
            } else if self.cur.y as usize + 1 < self.contents.len() {
                // At end of line, merge with next line
                let next_line = self.contents.remove(self.cur.y as usize + 1);
                if let Some(current) = self.contents.get_mut(self.cur.y as usize) {
                    current.inner.push_str(&next_line.inner);
                }
            }
        }
    }

    /// Check if undo is available
    #[must_use]
    pub const fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    /// Check if redo is available
    #[must_use]
    pub const fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// Extract text between two positions (internal helper)
    ///
    /// `inclusive` - if true, includes the character at `end` position (for visual selection)
    ///               if false, excludes it (for motion-based operations like dw)
    #[allow(clippy::cast_possible_truncation)]
    fn extract_text(&self, start: Position, end: Position, inclusive: bool) -> String {
        let mut result = String::new();

        if start.y == end.y {
            // Single line selection
            if let Some(line) = self.contents.get(start.y as usize) {
                let start_x = start.x as usize;
                let end_x = if inclusive {
                    (end.x as usize + 1).min(line.inner.len())
                } else {
                    (end.x as usize).min(line.inner.len())
                };
                if start_x < line.inner.len() && start_x < end_x {
                    result.push_str(&line.inner[start_x..end_x]);
                }
            }
        } else {
            // Multi-line selection
            for y in start.y..=end.y {
                if let Some(line) = self.contents.get(y as usize) {
                    if y == start.y {
                        let start_x = start.x as usize;
                        if start_x < line.inner.len() {
                            result.push_str(&line.inner[start_x..]);
                        }
                        result.push('\n');
                    } else if y == end.y {
                        let end_x = if inclusive {
                            (end.x as usize + 1).min(line.inner.len())
                        } else {
                            (end.x as usize).min(line.inner.len())
                        };
                        result.push_str(&line.inner[..end_x]);
                    } else {
                        result.push_str(&line.inner);
                        result.push('\n');
                    }
                }
            }
        }
        result
    }

    /// Extract text from a block (rectangular) selection
    #[allow(clippy::cast_possible_truncation)]
    fn extract_block_text(&self, top_left: Position, bottom_right: Position) -> String {
        let mut lines_text = Vec::new();

        for y in top_left.y..=bottom_right.y {
            if let Some(line) = self.contents.get(y as usize) {
                let start_x = top_left.x as usize;
                let end_x = (bottom_right.x as usize + 1).min(line.inner.len());

                if start_x < line.inner.len() {
                    lines_text.push(line.inner[start_x..end_x].to_string());
                } else {
                    lines_text.push(String::new());
                }
            }
        }

        lines_text.join("\n")
    }

    /// Delete a block (rectangular) selection
    #[allow(clippy::cast_possible_truncation)]
    fn delete_block(&mut self, top_left: Position, bottom_right: Position) -> String {
        let text = self.extract_block_text(top_left, bottom_right);

        for y in top_left.y..=bottom_right.y {
            if let Some(line) = self.contents.get_mut(y as usize) {
                let start_x = top_left.x as usize;
                let end_x = (bottom_right.x as usize + 1).min(line.inner.len());

                if start_x < line.inner.len() {
                    line.inner.drain(start_x..end_x);
                }
            }
        }

        self.cur = top_left;
        self.clear_selection();
        text
    }
}

// === Selection Operations ===
impl SelectionOps for Buffer {
    #[allow(clippy::missing_const_for_fn)]
    fn start_selection(&mut self) {
        self.selection.anchor = self.cur;
        self.selection.active = true;
        self.selection.mode = SelectionMode::Character;
    }

    #[allow(clippy::missing_const_for_fn)]
    fn start_block_selection(&mut self) {
        self.selection.anchor = self.cur;
        self.selection.active = true;
        self.selection.mode = SelectionMode::Block;
    }

    #[allow(clippy::missing_const_for_fn)]
    fn start_line_selection(&mut self) {
        self.selection.anchor = self.cur;
        self.selection.active = true;
        self.selection.mode = SelectionMode::Line;
    }

    #[allow(clippy::missing_const_for_fn)]
    fn clear_selection(&mut self) {
        self.selection.active = false;
    }

    #[allow(clippy::missing_const_for_fn)]
    fn selection_bounds(&self) -> (Position, Position) {
        let anchor = self.selection.anchor;
        let cursor = self.cur;

        if anchor.y < cursor.y || (anchor.y == cursor.y && anchor.x <= cursor.x) {
            (anchor, cursor)
        } else {
            (cursor, anchor)
        }
    }

    #[allow(clippy::missing_const_for_fn)]
    fn block_bounds(&self) -> (Position, Position) {
        let anchor = self.selection.anchor;
        let cursor = self.cur;
        let top_left = Position {
            x: anchor.x.min(cursor.x),
            y: anchor.y.min(cursor.y),
        };
        let bottom_right = Position {
            x: anchor.x.max(cursor.x),
            y: anchor.y.max(cursor.y),
        };
        (top_left, bottom_right)
    }

    fn selection_mode(&self) -> SelectionMode {
        self.selection.mode
    }

    fn get_selected_text(&self) -> String {
        if !self.selection.active {
            return String::new();
        }

        match self.selection.mode {
            SelectionMode::Block => {
                let (top_left, bottom_right) = self.block_bounds();
                self.extract_block_text(top_left, bottom_right)
            }
            SelectionMode::Character | SelectionMode::Line => {
                let (start, end) = self.selection_bounds();
                self.extract_text(start, end, true) // Visual selection is inclusive
            }
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn delete_selection(&mut self) -> String {
        if !self.selection.active {
            return String::new();
        }

        // Handle block mode deletion separately
        if self.selection.mode == SelectionMode::Block {
            let (top_left, bottom_right) = self.block_bounds();
            return self.delete_block(top_left, bottom_right);
        }

        // Character mode deletion
        let text = self.get_selected_text();
        let (start, end) = self.selection_bounds();

        if start.y == end.y {
            // Single line deletion
            if let Some(line) = self.contents.get_mut(start.y as usize) {
                let start_x = start.x as usize;
                let end_x = (end.x as usize + 1).min(line.inner.len());
                if start_x < line.inner.len() {
                    line.inner.drain(start_x..end_x);
                }
            }
        } else {
            // Multi-line deletion
            if let Some(first_line) = self.contents.get(start.y as usize) {
                let prefix = first_line.inner[..start.x as usize].to_string();
                if let Some(last_line) = self.contents.get(end.y as usize) {
                    let end_x = (end.x as usize + 1).min(last_line.inner.len());
                    let suffix = last_line.inner[end_x..].to_string();

                    // Remove lines from end to start+1
                    for _ in (start.y + 1..=end.y).rev() {
                        if (start.y as usize + 1) < self.contents.len() {
                            self.contents.remove(start.y as usize + 1);
                        }
                    }

                    // Merge prefix and suffix into start line
                    if let Some(line) = self.contents.get_mut(start.y as usize) {
                        line.inner = prefix + &suffix;
                    }
                }
            }
        }

        self.cur = start;
        self.clear_selection();
        text
    }
}

// === Text Operations ===
impl TextOps for Buffer {
    fn set_content(&mut self, content: &str) {
        self.contents.clear();
        for line in content.lines() {
            let new_line = Line::from(line);
            self.contents.push(new_line);
        }
    }

    fn content_to_string(&self) -> String {
        self.contents
            .iter()
            .map(|line| line.inner.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[allow(clippy::cast_possible_truncation)]
    fn insert_char(&mut self, c: char) {
        let pos = self.cur;
        if self.contents.is_empty() {
            self.contents.push(Line::from(""));
        }
        if let Some(line) = self.contents.get_mut(self.cur.y as usize) {
            let x = self.cur.x as usize;
            if x <= line.inner.len() {
                line.inner.insert(x, c);
                self.cur.x += 1;
                // Record change for undo
                self.record_change(Change::Insert {
                    pos,
                    text: c.to_string(),
                });
            }
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn insert_newline(&mut self) {
        let pos = self.cur;
        if self.contents.is_empty() {
            self.contents.push(Line::from(""));
        }
        let y = self.cur.y as usize;
        let x = self.cur.x as usize;

        if let Some(line) = self.contents.get_mut(y) {
            // Split the current line at cursor position
            let rest = if x < line.inner.len() {
                line.inner.split_off(x)
            } else {
                String::new()
            };
            // Insert the rest as a new line below
            self.contents.insert(y + 1, Line { inner: rest });
        }
        // Move cursor to start of the new line
        self.cur.y += 1;
        self.cur.x = 0;
        // Record change for undo
        self.record_change(Change::Insert {
            pos,
            text: "\n".to_string(),
        });
    }

    #[allow(clippy::cast_possible_truncation)]
    fn delete_char_backward(&mut self) {
        if self.cur.x > 0
            && let Some(line) = self.contents.get_mut(self.cur.y as usize)
        {
            let x = (self.cur.x - 1) as usize;
            if x < line.inner.len() {
                let deleted_char = line.inner.remove(x);
                self.cur.x -= 1;
                // Record change for undo
                self.record_change(Change::Delete {
                    pos: self.cur,
                    text: deleted_char.to_string(),
                });
            }
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn delete_char_forward(&mut self) {
        let pos = self.cur;
        if let Some(line) = self.contents.get_mut(self.cur.y as usize) {
            let x = self.cur.x as usize;
            if x < line.inner.len() {
                let deleted_char = line.inner.remove(x);
                // Record change for undo
                self.record_change(Change::Delete {
                    pos,
                    text: deleted_char.to_string(),
                });
            }
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn delete_line(&mut self) -> String {
        let y = self.cur.y as usize;
        if y < self.contents.len() {
            let deleted_line = self.contents.remove(y);
            // Record change for undo (include newline if not last line)
            let pos = Position {
                x: 0,
                y: self.cur.y,
            };
            let text = if y < self.contents.len() {
                deleted_line.inner + "\n"
            } else {
                deleted_line.inner
            };
            self.record_change(Change::Delete {
                pos,
                text: text.clone(),
            });

            if self.cur.y as usize >= self.contents.len() && !self.contents.is_empty() {
                self.cur.y = (self.contents.len() - 1) as u16;
            }
            return text;
        }
        String::new()
    }
}

// === Cursor Operations ===
impl CursorOps for Buffer {
    fn word_forward(&mut self) {
        self.cur = calculate_motion(&self.contents, self.cur, Motion::WordForward, 1);
    }

    fn word_backward(&mut self) {
        self.cur = calculate_motion(&self.contents, self.cur, Motion::WordBackward, 1);
    }

    fn word_end(&mut self) {
        self.cur = calculate_motion(&self.contents, self.cur, Motion::WordEnd, 1);
    }

    fn apply_motion(&mut self, motion: Motion, count: usize) {
        self.cur = calculate_motion(&self.contents, self.cur, motion, count);
    }
}

// === Operator + Motion Operations ===
impl Buffer {
    /// Delete from cursor to motion target, returns deleted text
    ///
    /// For linewise motions (j, k, G, gg), deletes entire lines.
    /// For characterwise motions (w, b, $, 0), deletes character range.
    #[allow(clippy::cast_possible_truncation)]
    pub fn delete_to_motion(&mut self, motion: Motion, count: usize) -> String {
        let start = self.cur;

        if motion.is_linewise() {
            // Special case: count=0 means delete current line only (dd with count=1)
            // This is needed because calculate_motion forces count=0 to count=1
            if count == 0 {
                return self.delete_lines(start.y as usize, start.y as usize);
            }

            let target = calculate_motion(&self.contents, self.cur, motion, count);
            // Linewise delete (dj, dk, dG, dgg)
            let (from_y, to_y) = if start.y <= target.y {
                (start.y as usize, target.y as usize)
            } else {
                (target.y as usize, start.y as usize)
            };
            self.delete_lines(from_y, to_y)
        } else {
            // Characterwise delete (dw, db, d$, d0)
            // Use motion's inclusivity: $ and e are inclusive, w and b are exclusive
            let target = calculate_motion(&self.contents, self.cur, motion, count);
            let (from, to) = if start.y < target.y || (start.y == target.y && start.x <= target.x) {
                (start, target)
            } else {
                (target, start)
            };
            self.delete_range_ex(from, to, motion.is_inclusive())
        }
    }

    /// Yank from cursor to motion target, returns yanked text
    ///
    /// Similar to `delete_to_motion` but doesn't modify the buffer.
    #[allow(clippy::cast_possible_truncation)]
    pub fn yank_to_motion(&mut self, motion: Motion, count: usize) -> String {
        let start = self.cur;

        if motion.is_linewise() {
            // Special case: count=0 means yank current line only (yy with count=1)
            // This is needed because calculate_motion forces count=0 to count=1
            if count == 0 {
                return self.yank_lines(start.y as usize, start.y as usize);
            }

            let target = calculate_motion(&self.contents, self.cur, motion, count);
            // Linewise yank
            let (from_y, to_y) = if start.y <= target.y {
                (start.y as usize, target.y as usize)
            } else {
                (target.y as usize, start.y as usize)
            };
            self.yank_lines(from_y, to_y)
        } else {
            // Characterwise yank
            // Use motion's inclusivity: $ and e are inclusive, w and b are exclusive
            let target = calculate_motion(&self.contents, self.cur, motion, count);
            let (from, to) = if start.y < target.y || (start.y == target.y && start.x <= target.x) {
                (start, target)
            } else {
                (target, start)
            };
            self.yank_range_ex(from, to, motion.is_inclusive())
        }
    }

    /// Delete a range of characters between two positions
    ///
    /// `inclusive` - if true, includes the character at `end` position
    ///               if false, excludes it (for motion-based operations)
    #[allow(clippy::cast_possible_truncation)]
    pub fn delete_range_ex(&mut self, start: Position, end: Position, inclusive: bool) -> String {
        let text = self.extract_text(start, end, inclusive);
        if text.is_empty() {
            return text;
        }

        // Use selection-style deletion logic
        if start.y == end.y {
            // Single line deletion
            if let Some(line) = self.contents.get_mut(start.y as usize) {
                let start_x = start.x as usize;
                let end_x = if inclusive {
                    (end.x as usize + 1).min(line.inner.len())
                } else {
                    (end.x as usize).min(line.inner.len())
                };
                if start_x < line.inner.len() && start_x < end_x {
                    line.inner.drain(start_x..end_x);
                }
            }
        } else {
            // Multi-line deletion
            if let Some(first_line) = self.contents.get(start.y as usize) {
                let prefix = first_line.inner[..start.x as usize].to_string();
                if let Some(last_line) = self.contents.get(end.y as usize) {
                    let end_x = if inclusive {
                        (end.x as usize + 1).min(last_line.inner.len())
                    } else {
                        (end.x as usize).min(last_line.inner.len())
                    };
                    let suffix = last_line.inner[end_x..].to_string();

                    // Remove lines from end to start+1
                    for _ in (start.y + 1..=end.y).rev() {
                        if (start.y as usize + 1) < self.contents.len() {
                            self.contents.remove(start.y as usize + 1);
                        }
                    }

                    // Merge prefix and suffix into start line
                    if let Some(line) = self.contents.get_mut(start.y as usize) {
                        line.inner = prefix + &suffix;
                    }
                }
            }
        }

        // Record change for undo
        self.record_change(Change::Delete {
            pos: start,
            text: text.clone(),
        });

        self.cur = start;
        text
    }

    /// Delete a range of characters between two positions (inclusive end)
    ///
    /// Convenience wrapper that calls `delete_range_ex` with inclusive=true
    #[allow(clippy::cast_possible_truncation)]
    pub fn delete_range(&mut self, start: Position, end: Position) -> String {
        self.delete_range_ex(start, end, true)
    }

    /// Delete entire lines from `start_y` to `end_y` (inclusive)
    #[allow(clippy::cast_possible_truncation)]
    fn delete_lines(&mut self, start_y: usize, end_y: usize) -> String {
        let mut deleted_text = String::new();

        // Collect text from all lines to be deleted
        for y in start_y..=end_y {
            if let Some(line) = self.contents.get(y) {
                if !deleted_text.is_empty() {
                    deleted_text.push('\n');
                }
                deleted_text.push_str(&line.inner);
            }
        }
        deleted_text.push('\n'); // Linewise deletes include trailing newline

        // Remove lines from end to start
        for _ in (start_y..=end_y).rev() {
            if start_y < self.contents.len() {
                self.contents.remove(start_y);
            }
        }

        // Record change for undo
        self.record_change(Change::Delete {
            pos: Position {
                x: 0,
                y: start_y as u16,
            },
            text: deleted_text.clone(),
        });

        // Move cursor to start of deleted region
        self.cur.y = start_y.min(self.contents.len().saturating_sub(1)) as u16;
        self.cur.x = 0;

        // Clamp cursor if buffer is now empty
        if self.contents.is_empty() {
            self.contents.push(Line::from(""));
            self.cur = Position { x: 0, y: 0 };
        }

        deleted_text
    }

    /// Yank a range of characters (doesn't modify buffer)
    ///
    /// `inclusive` - if true, includes the character at `end` position
    #[must_use]
    pub fn yank_range_ex(&self, start: Position, end: Position, inclusive: bool) -> String {
        self.extract_text(start, end, inclusive)
    }

    /// Yank a range of characters (inclusive end, doesn't modify buffer)
    #[must_use]
    pub fn yank_range(&self, start: Position, end: Position) -> String {
        self.extract_text(start, end, true)
    }

    /// Yank entire lines (doesn't modify buffer)
    fn yank_lines(&self, start_y: usize, end_y: usize) -> String {
        let mut yanked_text = String::new();

        for y in start_y..=end_y {
            if let Some(line) = self.contents.get(y) {
                if !yanked_text.is_empty() {
                    yanked_text.push('\n');
                }
                yanked_text.push_str(&line.inner);
            }
        }
        yanked_text.push('\n'); // Linewise yanks include trailing newline

        yanked_text
    }

    /// Delete text object (di(, da{, etc.)
    pub fn delete_text_object(&mut self, text_object: TextObject) -> String {
        if let Some((start, end)) = self.find_text_object_bounds(text_object) {
            self.delete_range(start, end)
        } else {
            String::new()
        }
    }

    /// Yank text object (yi(, ya{, etc.)
    pub fn yank_text_object(&mut self, text_object: TextObject) -> String {
        if let Some((start, end)) = self.find_text_object_bounds(text_object) {
            self.yank_range(start, end)
        } else {
            String::new()
        }
    }

    /// Find the bounds of a text object at cursor position
    #[allow(clippy::cast_possible_truncation)]
    #[must_use]
    pub fn find_text_object_bounds(&self, text_object: TextObject) -> Option<(Position, Position)> {
        let (open_char, close_char) = text_object.delimiter.chars();

        // Find the matching delimiter pair containing cursor
        let (open_pos, close_pos) = self.find_delimiter_pair(open_char, close_char)?;

        match text_object.scope {
            TextObjectScope::Inner => {
                // Inner: between delimiters (exclusive)
                // Start after opening delimiter
                let start = if open_pos.x as usize + 1
                    < self.contents.get(open_pos.y as usize)?.inner.len()
                {
                    Position {
                        x: open_pos.x + 1,
                        y: open_pos.y,
                    }
                } else if open_pos.y < close_pos.y {
                    // Opening delimiter at end of line, start at beginning of next line
                    Position {
                        x: 0,
                        y: open_pos.y + 1,
                    }
                } else {
                    // Empty content
                    return None;
                };

                // End before closing delimiter
                let end = if close_pos.x > 0 {
                    Position {
                        x: close_pos.x - 1,
                        y: close_pos.y,
                    }
                } else if close_pos.y > open_pos.y {
                    // Closing delimiter at start of line, end at end of previous line
                    let prev_line = self.contents.get(close_pos.y as usize - 1)?;
                    Position {
                        x: prev_line.inner.len().saturating_sub(1) as u16,
                        y: close_pos.y - 1,
                    }
                } else {
                    // Empty content
                    return None;
                };

                // Check if start is before or equal to end
                if start.y < end.y || (start.y == end.y && start.x <= end.x) {
                    Some((start, end))
                } else {
                    None // Empty content between delimiters
                }
            }
            TextObjectScope::Around => {
                // Around: including delimiters
                Some((open_pos, close_pos))
            }
        }
    }

    /// Find matching delimiter pair containing cursor.
    /// Returns `(open_position, close_position)`.
    #[allow(clippy::cast_possible_truncation)]
    fn find_delimiter_pair(&self, open: char, close: char) -> Option<(Position, Position)> {
        let is_symmetric = open == close;

        if is_symmetric {
            self.find_symmetric_delimiter_pair(open)
        } else {
            self.find_asymmetric_delimiter_pair(open, close)
        }
    }

    /// Find matching symmetric delimiter pair (quotes)
    #[allow(clippy::cast_possible_truncation)]
    fn find_symmetric_delimiter_pair(&self, quote: char) -> Option<(Position, Position)> {
        let cur_y = self.cur.y as usize;
        let cur_x = self.cur.x as usize;

        let line = self.contents.get(cur_y)?;
        let chars: Vec<char> = line.inner.chars().collect();

        // Find all quote positions on the current line
        let quote_positions: Vec<usize> = chars
            .iter()
            .enumerate()
            .filter(|&(_, c)| *c == quote)
            .map(|(i, _)| i)
            .collect();

        // Find pair containing cursor
        for i in 0..quote_positions.len() / 2 {
            let open_idx = quote_positions[i * 2];
            let close_idx = quote_positions[i * 2 + 1];
            if cur_x >= open_idx && cur_x <= close_idx {
                return Some((
                    Position {
                        x: open_idx as u16,
                        y: cur_y as u16,
                    },
                    Position {
                        x: close_idx as u16,
                        y: cur_y as u16,
                    },
                ));
            }
        }
        None
    }

    /// Find matching asymmetric delimiter pair (brackets)
    #[allow(clippy::cast_possible_truncation)]
    fn find_asymmetric_delimiter_pair(
        &self,
        open: char,
        close: char,
    ) -> Option<(Position, Position)> {
        // Search backward for opening delimiter
        let open_pos = self.find_backward(open, close)?;
        // Search forward for closing delimiter
        let close_pos = self.find_forward(open, close)?;
        Some((open_pos, close_pos))
    }

    /// Search backward for unmatched opening delimiter
    #[allow(clippy::cast_possible_truncation)]
    fn find_backward(&self, open: char, close: char) -> Option<Position> {
        let mut y = self.cur.y as usize;
        let mut x = self.cur.x as usize;
        let mut depth = 0;

        loop {
            if let Some(line) = self.contents.get(y) {
                let chars: Vec<char> = line.inner.chars().collect();
                // Search from current position backward
                for i in (0..=x.min(chars.len().saturating_sub(1))).rev() {
                    let c = chars[i];
                    if c == close {
                        depth += 1;
                    } else if c == open {
                        if depth == 0 {
                            return Some(Position {
                                x: i as u16,
                                y: y as u16,
                            });
                        }
                        depth -= 1;
                    }
                }
            }

            if y == 0 {
                break;
            }
            y -= 1;
            x = self
                .contents
                .get(y)
                .map_or(0, |line| line.inner.len().saturating_sub(1));
        }
        None
    }

    /// Search forward for unmatched closing delimiter
    #[allow(clippy::cast_possible_truncation)]
    fn find_forward(&self, open: char, close: char) -> Option<Position> {
        let mut y = self.cur.y as usize;
        let mut x = self.cur.x as usize;
        let mut depth = 0;

        loop {
            if let Some(line) = self.contents.get(y) {
                let chars: Vec<char> = line.inner.chars().collect();
                // Search from current position forward
                let start = if y == self.cur.y as usize { x } else { 0 };
                for (i, &c) in chars.iter().enumerate().skip(start) {
                    if c == open {
                        depth += 1;
                    } else if c == close {
                        if depth == 0 {
                            return Some(Position {
                                x: i as u16,
                                y: y as u16,
                            });
                        }
                        depth -= 1;
                    }
                }
            }

            y += 1;
            if y >= self.contents.len() {
                break;
            }
            x = 0;
        }
        None
    }

    // === Word Text Object Operations ===

    /// Delete word text object (diw, daw, diW, daW)
    pub fn delete_word_text_object(
        &mut self,
        text_object: crate::textobject::WordTextObject,
    ) -> String {
        if let Some((start, end)) = self.find_word_text_object_bounds(text_object) {
            self.delete_range(start, end)
        } else {
            String::new()
        }
    }

    /// Yank word text object (yiw, yaw, yiW, yaW)
    #[must_use]
    pub fn yank_word_text_object(&self, text_object: crate::textobject::WordTextObject) -> String {
        if let Some((start, end)) = self.find_word_text_object_bounds(text_object) {
            self.yank_range(start, end)
        } else {
            String::new()
        }
    }

    /// Find the bounds of a word text object at cursor position
    #[allow(clippy::cast_possible_truncation)]
    pub fn find_word_text_object_bounds(
        &self,
        text_object: crate::textobject::WordTextObject,
    ) -> Option<(Position, Position)> {
        use crate::textobject::{TextObjectScope, WordType};

        let line = self.contents.get(self.cur.y as usize)?;
        let chars: Vec<char> = line.inner.chars().collect();
        let cursor_x = self.cur.x as usize;

        if chars.is_empty() {
            return None;
        }

        // Get the character under cursor (or last char if at end)
        let cursor_x = cursor_x.min(chars.len().saturating_sub(1));
        let cursor_char = chars.get(cursor_x)?;

        // Determine word boundaries based on word type
        let (word_start, word_end) = match text_object.word_type {
            WordType::Word => {
                // For small word, we need to handle word chars vs punctuation vs whitespace
                if cursor_char.is_whitespace() {
                    // Cursor on whitespace - select whitespace region
                    let start =
                        Self::find_word_boundary_backward(&chars, cursor_x, char::is_whitespace);
                    let end =
                        Self::find_word_boundary_forward(&chars, cursor_x, char::is_whitespace);
                    (start, end)
                } else if WordType::is_word_char(*cursor_char) {
                    // Cursor on word char - select word
                    let start =
                        Self::find_word_boundary_backward(&chars, cursor_x, WordType::is_word_char);
                    let end =
                        Self::find_word_boundary_forward(&chars, cursor_x, WordType::is_word_char);
                    (start, end)
                } else {
                    // Cursor on punctuation - select punctuation sequence
                    let is_punct = |c: char| !c.is_whitespace() && !WordType::is_word_char(c);
                    let start = Self::find_word_boundary_backward(&chars, cursor_x, is_punct);
                    let end = Self::find_word_boundary_forward(&chars, cursor_x, is_punct);
                    (start, end)
                }
            }
            WordType::BigWord => {
                // For big WORD, just find non-whitespace boundaries
                if cursor_char.is_whitespace() {
                    // Cursor on whitespace - select whitespace region
                    let start =
                        Self::find_word_boundary_backward(&chars, cursor_x, char::is_whitespace);
                    let end =
                        Self::find_word_boundary_forward(&chars, cursor_x, char::is_whitespace);
                    (start, end)
                } else {
                    // Cursor on non-whitespace - select WORD
                    let is_non_whitespace = |c: char| !c.is_whitespace();
                    let start =
                        Self::find_word_boundary_backward(&chars, cursor_x, is_non_whitespace);
                    let end = Self::find_word_boundary_forward(&chars, cursor_x, is_non_whitespace);
                    (start, end)
                }
            }
        };

        let start_pos = Position {
            x: word_start as u16,
            y: self.cur.y,
        };
        let end_pos = Position {
            x: word_end as u16,
            y: self.cur.y,
        };

        match text_object.scope {
            TextObjectScope::Inner => Some((start_pos, end_pos)),
            TextObjectScope::Around => {
                // "Around" includes trailing whitespace (or leading if no trailing)
                let trailing_end =
                    Self::find_word_boundary_forward(&chars, word_end, char::is_whitespace);
                if trailing_end > word_end {
                    // Include trailing whitespace
                    Some((
                        start_pos,
                        Position {
                            x: trailing_end as u16,
                            y: self.cur.y,
                        },
                    ))
                } else {
                    // No trailing whitespace, try leading
                    let leading_start =
                        Self::find_word_boundary_backward(&chars, word_start, char::is_whitespace);
                    if leading_start < word_start {
                        Some((
                            Position {
                                x: leading_start as u16,
                                y: self.cur.y,
                            },
                            end_pos,
                        ))
                    } else {
                        // No surrounding whitespace
                        Some((start_pos, end_pos))
                    }
                }
            }
        }
    }

    /// Find word boundary going backward from position
    fn find_word_boundary_backward<F>(chars: &[char], start: usize, matches: F) -> usize
    where
        F: Fn(char) -> bool,
    {
        let mut pos = start;
        while pos > 0 && matches(chars[pos - 1]) {
            pos -= 1;
        }
        pos
    }

    /// Find word boundary going forward from position
    fn find_word_boundary_forward<F>(chars: &[char], start: usize, matches: F) -> usize
    where
        F: Fn(char) -> bool,
    {
        let mut pos = start;
        while pos < chars.len() - 1 && matches(chars[pos + 1]) {
            pos += 1;
        }
        pos
    }
}
