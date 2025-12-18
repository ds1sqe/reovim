//! Cursor and word motion operations for buffers

use crate::{motion::Motion, screen::Position};

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
    calculate_motion_with_desired_col(contents, cur, None, motion, count).0
}

/// Calculate motion with desired column support for vertical movements
///
/// Returns (`new_position`, `new_desired_col`)
/// - For vertical movements (Up/Down), uses `desired_col` to preserve horizontal position
/// - For horizontal movements, clears `desired_col`
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn calculate_motion_with_desired_col(
    contents: &[crate::buffer::Line],
    cur: Position,
    desired_col: Option<u16>,
    motion: Motion,
    count: usize,
) -> (Position, Option<u16>) {
    let mut pos = cur;
    let count = count.max(1);
    let mut new_desired_col = desired_col;

    match motion {
        Motion::Left => {
            pos.x = pos.x.saturating_sub(count as u16);
            new_desired_col = None; // Clear on horizontal movement
        }
        Motion::Right => {
            if let Some(line) = contents.get(pos.y as usize) {
                let max_x = line.inner.len().saturating_sub(1) as u16;
                pos.x = (pos.x + count as u16).min(max_x);
            }
            new_desired_col = None; // Clear on horizontal movement
        }
        Motion::Up => {
            // Remember desired column before moving
            if new_desired_col.is_none() {
                new_desired_col = Some(cur.x);
            }
            pos.y = pos.y.saturating_sub(count as u16);
            // Apply desired column, clamped to line length
            let target_col = new_desired_col.unwrap_or(cur.x);
            if let Some(line) = contents.get(pos.y as usize) {
                let max_x = if line.inner.is_empty() {
                    0
                } else {
                    line.inner.len().saturating_sub(1) as u16
                };
                pos.x = target_col.min(max_x);
            }
        }
        Motion::Down => {
            // Remember desired column before moving
            if new_desired_col.is_none() {
                new_desired_col = Some(cur.x);
            }
            let max_y = contents.len().saturating_sub(1) as u16;
            pos.y = (pos.y + count as u16).min(max_y);
            // Apply desired column, clamped to line length
            let target_col = new_desired_col.unwrap_or(cur.x);
            if let Some(line) = contents.get(pos.y as usize) {
                let max_x = if line.inner.is_empty() {
                    0
                } else {
                    line.inner.len().saturating_sub(1) as u16
                };
                pos.x = target_col.min(max_x);
            }
        }
        Motion::LineStart => {
            pos.x = 0;
            new_desired_col = None;
        }
        Motion::LineEnd => {
            if let Some(line) = contents.get(pos.y as usize) {
                pos.x = line.inner.len().saturating_sub(1) as u16;
            }
            new_desired_col = None;
        }
        Motion::WordForward => {
            pos = word_forward_calc(contents, pos);
            new_desired_col = None;
        }
        Motion::WordBackward => {
            pos = word_backward_calc(contents, pos);
            new_desired_col = None;
        }
        Motion::DocumentStart => {
            pos.x = 0;
            pos.y = 0;
            new_desired_col = None;
        }
        Motion::DocumentEnd => {
            let max_y = contents.len().saturating_sub(1) as u16;
            pos.y = max_y;
            pos.x = 0;
            new_desired_col = None;
        }
    }

    (pos, new_desired_col)
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
