//! Buffer module for text storage and manipulation

mod cursor;
mod selection;
mod text;

#[cfg(test)]
mod tests;

pub use cursor::{calculate_motion, calculate_motion_with_desired_col, CursorOps};
pub use selection::{Selection, SelectionMode, SelectionOps};
pub use text::TextOps;

use crate::motion::Motion;

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

    /// Extract text between two positions (internal helper)
    #[allow(clippy::cast_possible_truncation)]
    fn extract_text(&self, start: Position, end: Position) -> String {
        let mut result = String::new();

        if start.y == end.y {
            // Single line selection
            if let Some(line) = self.contents.get(start.y as usize) {
                let start_x = start.x as usize;
                let end_x = (end.x as usize + 1).min(line.inner.len());
                if start_x < line.inner.len() {
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
                        let end_x = (end.x as usize + 1).min(line.inner.len());
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
                self.extract_text(start, end)
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

    #[allow(clippy::cast_possible_truncation)]
    fn insert_newline(&mut self) {
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
    }

    #[allow(clippy::cast_possible_truncation)]
    fn delete_char_backward(&mut self) {
        if self.cur.x > 0
            && let Some(line) = self.contents.get_mut(self.cur.y as usize)
        {
            let x = (self.cur.x - 1) as usize;
            if x < line.inner.len() {
                line.inner.remove(x);
                self.cur.x -= 1;
            }
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn delete_char_forward(&mut self) {
        if let Some(line) = self.contents.get_mut(self.cur.y as usize) {
            let x = self.cur.x as usize;
            if x < line.inner.len() {
                line.inner.remove(x);
            }
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn delete_line(&mut self) {
        let y = self.cur.y as usize;
        if y < self.contents.len() {
            self.contents.remove(y);
            if self.cur.y as usize >= self.contents.len() && !self.contents.is_empty() {
                self.cur.y = (self.contents.len() - 1) as u16;
            }
        }
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

    fn apply_motion(&mut self, motion: Motion, count: usize) {
        self.cur = calculate_motion(&self.contents, self.cur, motion, count);
    }
}
