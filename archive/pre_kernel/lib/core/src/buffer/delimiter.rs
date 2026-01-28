//! Delimiter pair finding for text objects
//!
//! This module provides delimiter matching logic for operations like:
//! - `di(` / `da(` - inner/around parentheses
//! - `di{` / `da{` - inner/around braces
//! - `di"` / `da"` - inner/around quotes

use crate::screen::Position;

use super::Buffer;

impl Buffer {
    /// Find matching delimiter pair containing cursor.
    /// Returns `(open_position, close_position)`.
    #[allow(clippy::cast_possible_truncation)]
    pub(super) fn find_delimiter_pair(
        &self,
        open: char,
        close: char,
    ) -> Option<(Position, Position)> {
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
}
