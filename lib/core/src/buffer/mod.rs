use crate::screen::Position;

#[derive(Clone, Debug)]
pub struct Line {
    pub inner: String,
}

impl From<&str> for Line {
    fn from(value: &str) -> Self {
        Line {
            inner: value.to_string(),
        }
    }
}

/// Represents a text selection with anchor and cursor positions
#[derive(Clone, Debug, Default)]
pub struct Selection {
    /// The fixed anchor point where selection started
    pub anchor: Position,
    /// Whether selection is active
    pub active: bool,
}

#[derive(Clone, Debug)]
pub struct Buffer {
    pub id: usize,
    pub cur: Position,
    pub contents: Vec<Line>,
    pub selection: Selection,
    pub file_path: Option<String>,
}

impl Buffer {
    pub fn empty(id: usize) -> Self {
        Self {
            id,
            cur: Position { x: 0, y: 0 },
            contents: Vec::new(),
            selection: Selection::default(),
            file_path: None,
        }
    }

    /// Start visual selection at current cursor position
    pub fn start_selection(&mut self) {
        self.selection.anchor = self.cur.clone();
        self.selection.active = true;
    }

    /// Clear selection
    pub fn clear_selection(&mut self) {
        self.selection.active = false;
    }

    /// Get normalized selection bounds (start always before end)
    pub fn selection_bounds(&self) -> (Position, Position) {
        let anchor = &self.selection.anchor;
        let cursor = &self.cur;

        if anchor.y < cursor.y || (anchor.y == cursor.y && anchor.x <= cursor.x) {
            (anchor.clone(), cursor.clone())
        } else {
            (cursor.clone(), anchor.clone())
        }
    }

    /// Get selected text
    pub fn get_selected_text(&self) -> String {
        if !self.selection.active {
            return String::new();
        }
        let (start, end) = self.selection_bounds();
        self.extract_text(&start, &end)
    }

    /// Extract text between two positions
    fn extract_text(&self, start: &Position, end: &Position) -> String {
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

    /// Delete selected text and return it
    pub fn delete_selection(&mut self) -> String {
        if !self.selection.active {
            return String::new();
        }

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

    pub fn set_content(&mut self, content: &str) {
        self.contents.clear();
        for line in content.to_string().lines() {
            let new_line = Line::from(line);
            self.contents.push(new_line);
        }
    }

    pub fn to_string(&self) -> String {
        self.contents
            .iter()
            .map(|line| line.inner.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn insert_char(&mut self, c: char) {
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

    pub fn delete_char_backward(&mut self) {
        if self.cur.x > 0 {
            if let Some(line) = self.contents.get_mut(self.cur.y as usize) {
                let x = (self.cur.x - 1) as usize;
                if x < line.inner.len() {
                    line.inner.remove(x);
                    self.cur.x -= 1;
                }
            }
        }
    }

    pub fn delete_char_forward(&mut self) {
        if let Some(line) = self.contents.get_mut(self.cur.y as usize) {
            let x = self.cur.x as usize;
            if x < line.inner.len() {
                line.inner.remove(x);
            }
        }
    }

    pub fn delete_line(&mut self) {
        let y = self.cur.y as usize;
        if y < self.contents.len() {
            self.contents.remove(y);
            if self.cur.y as usize >= self.contents.len() && !self.contents.is_empty() {
                self.cur.y = (self.contents.len() - 1) as u16;
            }
        }
    }

    pub fn word_forward(&mut self) {
        if let Some(line) = self.contents.get(self.cur.y as usize) {
            let chars: Vec<char> = line.inner.chars().collect();
            let mut x = self.cur.x as usize;

            // Skip non-whitespace
            while x < chars.len() && !chars[x].is_whitespace() {
                x += 1;
            }
            // Skip whitespace
            while x < chars.len() && chars[x].is_whitespace() {
                x += 1;
            }

            self.cur.x = x as u16;
        }
    }

    pub fn word_backward(&mut self) {
        if let Some(line) = self.contents.get(self.cur.y as usize) {
            let chars: Vec<char> = line.inner.chars().collect();
            let mut x = self.cur.x as usize;

            if x > 0 {
                x -= 1;
            }

            // Skip whitespace
            while x > 0 && chars[x].is_whitespace() {
                x -= 1;
            }
            // Skip non-whitespace
            while x > 0 && !chars[x - 1].is_whitespace() {
                x -= 1;
            }

            self.cur.x = x as u16;
        }
    }
}
