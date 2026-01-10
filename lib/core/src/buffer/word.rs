//! Word boundary detection for text objects
//!
//! This module provides word boundary finding logic for operations like:
//! - `diw` / `daw` - inner/around small word
//! - `diW` / `daW` - inner/around big WORD

use crate::{
    screen::Position,
    textobject::{TextObjectScope, WordTextObject, WordType},
};

use super::Buffer;

impl Buffer {
    /// Delete word text object (diw, daw, diW, daW)
    pub fn delete_word_text_object(&mut self, text_object: WordTextObject) -> String {
        if let Some((start, end)) = self.find_word_text_object_bounds(text_object) {
            self.delete_range(start, end)
        } else {
            String::new()
        }
    }

    /// Yank word text object (yiw, yaw, yiW, yaW)
    #[must_use]
    pub fn yank_word_text_object(&self, text_object: WordTextObject) -> String {
        if let Some((start, end)) = self.find_word_text_object_bounds(text_object) {
            self.yank_range(start, end)
        } else {
            String::new()
        }
    }

    /// Find the bounds of a word text object at cursor position
    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn find_word_text_object_bounds(
        &self,
        text_object: WordTextObject,
    ) -> Option<(Position, Position)> {
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
