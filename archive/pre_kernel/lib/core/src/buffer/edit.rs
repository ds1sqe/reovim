use crate::{
    buffer::{Change, Line},
    screen::Position,
};

use super::Buffer;

impl Buffer {
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

    /// Insert text linewise (paste complete lines below/above current line)
    ///
    /// This is used for linewise yank/paste operations (yy, yj, yk).
    /// Unlike `insert_text` which inserts at cursor position, this inserts
    /// complete lines below (p) or above (P) the current line.
    ///
    /// # Arguments
    /// * `text` - The text to insert (should end with newline for linewise yanks)
    /// * `before` - If true, insert above current line (P), otherwise below (p)
    #[allow(clippy::cast_possible_truncation)]
    pub fn insert_linewise(&mut self, text: &str, before: bool) {
        if text.is_empty() {
            return;
        }

        let pos = self.cur;
        let current_line = pos.y as usize;

        // Split the yanked text into lines (removing trailing newline if present)
        let yanked_lines: Vec<&str> = text.trim_end_matches('\n').split('\n').collect();

        if before {
            // P: Insert lines ABOVE current line
            for (i, line_text) in yanked_lines.iter().enumerate() {
                self.contents
                    .insert(current_line + i, Line::from(*line_text));
            }
            // Cursor moves to start of first inserted line
            self.cur.y = current_line as u16;
        } else {
            // p: Insert lines BELOW current line
            let insert_pos = current_line + 1;
            for (i, line_text) in yanked_lines.iter().enumerate() {
                self.contents.insert(insert_pos + i, Line::from(*line_text));
            }
            // Cursor moves to start of first inserted line
            self.cur.y = insert_pos as u16;
        }
        // Cursor always moves to column 0 for linewise paste
        self.cur.x = 0;

        // Record as a single change for undo
        self.record_change(Change::Insert {
            pos,
            text: text.to_string(),
        });
    }

    /// Insert text at current cursor position (for undo/redo, no history)
    #[allow(clippy::cast_possible_truncation)]
    pub(super) fn insert_text_at_cursor(&mut self, text: &str) {
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
    pub(super) fn delete_text_at(&mut self, pos: Position, len: usize) {
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
}
