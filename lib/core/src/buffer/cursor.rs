//! Cursor and word motion operations for buffers

use crate::motion::Motion;
use crate::screen::Position;

/// Cursor movement operations for Buffer
pub trait CursorOps {
    /// Move cursor forward to next word
    fn word_forward(&mut self);
    /// Move cursor backward to previous word
    fn word_backward(&mut self);
    /// Apply a motion with count
    fn apply_motion(&mut self, motion: Motion, count: usize);
}

/// Helper to calculate motion result without modifying buffer
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn calculate_motion(
    contents: &[crate::buffer::Line],
    cur: Position,
    motion: Motion,
    count: usize,
) -> Position {
    // Create a temporary buffer-like view for motion calculation
    let mut pos = cur;
    let count = count.max(1);

    match motion {
        Motion::Left => {
            pos.x = pos.x.saturating_sub(count as u16);
        }
        Motion::Right => {
            if let Some(line) = contents.get(pos.y as usize) {
                let max_x = line.inner.len().saturating_sub(1) as u16;
                pos.x = (pos.x + count as u16).min(max_x);
            }
        }
        Motion::Up => {
            pos.y = pos.y.saturating_sub(count as u16);
        }
        Motion::Down => {
            let max_y = contents.len().saturating_sub(1) as u16;
            pos.y = (pos.y + count as u16).min(max_y);
        }
        Motion::LineStart => {
            pos.x = 0;
        }
        Motion::LineEnd => {
            if let Some(line) = contents.get(pos.y as usize) {
                pos.x = line.inner.len().saturating_sub(1) as u16;
            }
        }
        Motion::WordForward => {
            pos = word_forward_calc(contents, pos);
        }
        Motion::WordBackward => {
            pos = word_backward_calc(contents, pos);
        }
        Motion::DocumentStart => {
            pos.x = 0;
            pos.y = 0;
        }
        Motion::DocumentEnd => {
            let max_y = contents.len().saturating_sub(1) as u16;
            pos.y = max_y;
            pos.x = 0;
        }
    }

    pos
}

#[allow(clippy::cast_possible_truncation)]
fn word_forward_calc(contents: &[crate::buffer::Line], mut pos: Position) -> Position {
    if let Some(line) = contents.get(pos.y as usize) {
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

#[allow(clippy::cast_possible_truncation)]
fn word_backward_calc(contents: &[crate::buffer::Line], mut pos: Position) -> Position {
    if let Some(line) = contents.get(pos.y as usize) {
        let chars: Vec<char> = line.inner.chars().collect();
        let mut x = pos.x as usize;

        x = x.saturating_sub(1);

        // Skip whitespace
        while x > 0 && chars[x].is_whitespace() {
            x -= 1;
        }
        // Skip non-whitespace
        while x > 0 && !chars[x - 1].is_whitespace() {
            x -= 1;
        }

        pos.x = x as u16;
    }
    pos
}
