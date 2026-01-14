use crate::{
    buffer::{Change, Line, SelectionOps},
    motion::Motion,
    screen::Position,
    textobject::{TextObject, TextObjectScope},
};

use super::Buffer;

impl Buffer {
    /// Extract text between two positions (internal helper)
    ///
    /// `inclusive` - if true, includes the character at `end` position (for visual selection)
    ///               if false, excludes it (for motion-based operations like dw)
    #[allow(clippy::cast_possible_truncation)]
    pub(super) fn extract_text(&self, start: Position, end: Position, inclusive: bool) -> String {
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
    pub(super) fn extract_block_text(&self, top_left: Position, bottom_right: Position) -> String {
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
    pub(super) fn delete_block(&mut self, top_left: Position, bottom_right: Position) -> String {
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

    /// Delete from cursor to motion target, returns deleted text
    ///
    /// For linewise motions (j, k, G, gg), deletes entire lines.
    /// For characterwise motions (w, b, $, 0), deletes character range.
    #[allow(clippy::cast_possible_truncation)]
    pub fn delete_to_motion(&mut self, motion: Motion, count: usize) -> String {
        use crate::buffer::cursor::calculate_motion;

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
        use crate::buffer::cursor::calculate_motion;

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

        // Invalidate from start_y to end (lines shifted/deleted)
        self.highlight_cache.invalidate_from(start_y);
        self.decoration_cache.invalidate_from(start_y);

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
}
