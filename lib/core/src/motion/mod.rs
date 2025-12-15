use crate::buffer::Buffer;
use crate::screen::Position;

/// All possible cursor motions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Motion {
    // Character motions
    Left,
    Right,
    Up,
    Down,

    // Line motions
    LineStart,
    LineEnd,

    // Word motions
    WordForward,
    WordBackward,

    // Document motions
    DocumentStart,
    DocumentEnd,
}

impl Motion {
    /// Apply motion to buffer, returning new cursor position
    pub fn apply(&self, buffer: &Buffer, count: usize) -> Position {
        let mut pos = buffer.cur.clone();
        let count = count.max(1);

        match self {
            Motion::Left => {
                pos.x = pos.x.saturating_sub(count as u16);
            }
            Motion::Right => {
                if let Some(line) = buffer.contents.get(pos.y as usize) {
                    let max_x = line.inner.len().saturating_sub(1).max(0) as u16;
                    pos.x = (pos.x + count as u16).min(max_x);
                }
            }
            Motion::Up => {
                pos.y = pos.y.saturating_sub(count as u16);
            }
            Motion::Down => {
                let max_y = buffer.contents.len().saturating_sub(1) as u16;
                pos.y = (pos.y + count as u16).min(max_y);
            }
            Motion::LineStart => {
                pos.x = 0;
            }
            Motion::LineEnd => {
                if let Some(line) = buffer.contents.get(pos.y as usize) {
                    pos.x = line.inner.len().saturating_sub(1).max(0) as u16;
                }
            }
            Motion::WordForward => {
                pos = Self::word_forward_impl(buffer, pos);
            }
            Motion::WordBackward => {
                pos = Self::word_backward_impl(buffer, pos);
            }
            Motion::DocumentStart => {
                pos.x = 0;
                pos.y = 0;
            }
            Motion::DocumentEnd => {
                let max_y = buffer.contents.len().saturating_sub(1) as u16;
                pos.y = max_y;
                pos.x = 0;
            }
        }

        pos
    }

    fn word_forward_impl(buffer: &Buffer, mut pos: Position) -> Position {
        if let Some(line) = buffer.contents.get(pos.y as usize) {
            let chars: Vec<char> = line.inner.chars().collect();
            let mut x = pos.x as usize;

            // Skip non-whitespace
            while x < chars.len() && !chars[x].is_whitespace() {
                x += 1;
            }
            // Skip whitespace
            while x < chars.len() && chars[x].is_whitespace() {
                x += 1;
            }

            pos.x = x as u16;
        }
        pos
    }

    fn word_backward_impl(buffer: &Buffer, mut pos: Position) -> Position {
        if let Some(line) = buffer.contents.get(pos.y as usize) {
            let chars: Vec<char> = line.inner.chars().collect();
            let mut x = pos.x as usize;

            if x > 0 {
                x -= 1;
            }

            // Skip whitespace
            while x > 0 && chars.get(x).map(|c| c.is_whitespace()).unwrap_or(false) {
                x -= 1;
            }
            // Skip non-whitespace
            while x > 0
                && chars
                    .get(x.saturating_sub(1))
                    .map(|c| !c.is_whitespace())
                    .unwrap_or(false)
            {
                x -= 1;
            }

            pos.x = x as u16;
        }
        pos
    }
}
