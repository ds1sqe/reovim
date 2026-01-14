use crate::screen::Position;

use super::Buffer;

impl Buffer {
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
}
