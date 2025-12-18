//! Cursor and word motion operations for buffers

use crate::{motion::Motion, screen::Position};

/// Cursor movement operations for Buffer
pub trait CursorOps {
    /// Move cursor forward to next word
    fn word_forward(&mut self);
    /// Move cursor backward to previous word
    fn word_backward(&mut self);
    /// Move cursor to end of current/next word
    fn word_end(&mut self);
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
            for _ in 0..count {
                pos = word_forward_calc(contents, pos);
            }
            new_desired_col = None;
        }
        Motion::WordBackward => {
            for _ in 0..count {
                pos = word_backward_calc(contents, pos);
            }
            new_desired_col = None;
        }
        Motion::WordEnd => {
            for _ in 0..count {
                pos = word_end_calc(contents, pos);
            }
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

/// Move forward to start of next word, crossing line boundaries
#[allow(clippy::cast_possible_truncation)]
fn word_forward_calc(contents: &[crate::buffer::Line], mut pos: Position) -> Position {
    let total_lines = contents.len();
    if total_lines == 0 {
        return pos;
    }

    loop {
        let y = pos.y as usize;
        if y >= total_lines {
            // At or past last line
            pos.y = (total_lines - 1) as u16;
            if let Some(line) = contents.get(pos.y as usize) {
                pos.x = line.inner.len().saturating_sub(1) as u16;
            }
            break;
        }

        if let Some(line) = contents.get(y) {
            let chars: Vec<char> = line.inner.chars().collect();
            let mut x = pos.x as usize;

            // Skip non-whitespace (current word)
            while x < chars.len() && !chars[x].is_whitespace() {
                x += 1;
            }
            // Skip whitespace
            while x < chars.len() && chars[x].is_whitespace() {
                x += 1;
            }

            if x < chars.len() {
                // Found next word on this line
                pos.x = x as u16;
                break;
            }
            // Reached end of line, try next line
            if y + 1 < total_lines {
                pos.y += 1;
                pos.x = 0;
                // Skip leading whitespace on new line
                if let Some(next_line) = contents.get(pos.y as usize) {
                    let next_chars: Vec<char> = next_line.inner.chars().collect();
                    let mut nx = 0;
                    while nx < next_chars.len() && next_chars[nx].is_whitespace() {
                        nx += 1;
                    }
                    if nx < next_chars.len() {
                        pos.x = nx as u16;
                        break;
                    }
                    // Empty or whitespace-only line, loop will continue to next
                }
            } else {
                // Last line, stay at end
                pos.x = chars.len().saturating_sub(1) as u16;
                break;
            }
        } else {
            break;
        }
    }
    pos
}

/// Move forward to end of current/next word, crossing line boundaries
#[allow(clippy::cast_possible_truncation)]
fn word_end_calc(contents: &[crate::buffer::Line], mut pos: Position) -> Position {
    let total_lines = contents.len();
    if total_lines == 0 {
        return pos;
    }

    // First, move forward one position to handle being at end of word
    let y = pos.y as usize;
    if let Some(line) = contents.get(y) {
        let len = line.inner.chars().count();
        if pos.x as usize + 1 < len {
            pos.x += 1;
        } else if y + 1 < total_lines {
            pos.y += 1;
            pos.x = 0;
        }
    }

    loop {
        let y = pos.y as usize;
        if y >= total_lines {
            pos.y = (total_lines - 1) as u16;
            if let Some(line) = contents.get(pos.y as usize) {
                pos.x = line.inner.len().saturating_sub(1) as u16;
            }
            break;
        }

        if let Some(line) = contents.get(y) {
            let chars: Vec<char> = line.inner.chars().collect();
            let mut x = pos.x as usize;

            // Skip whitespace
            while x < chars.len() && chars[x].is_whitespace() {
                x += 1;
            }

            if x >= chars.len() {
                // End of line, try next line
                if y + 1 < total_lines {
                    pos.y += 1;
                    pos.x = 0;
                    continue;
                }
                // Last line
                pos.x = chars.len().saturating_sub(1) as u16;
                break;
            }

            // Now at start of a word, find its end
            while x + 1 < chars.len() && !chars[x + 1].is_whitespace() {
                x += 1;
            }

            pos.x = x as u16;
        }
        break;
    }
    pos
}

/// Move backward to start of previous word, crossing line boundaries
#[allow(clippy::cast_possible_truncation)]
fn word_backward_calc(contents: &[crate::buffer::Line], mut pos: Position) -> Position {
    if contents.is_empty() {
        return pos;
    }

    loop {
        let y = pos.y as usize;
        if let Some(line) = contents.get(y) {
            let chars: Vec<char> = line.inner.chars().collect();
            let mut x = pos.x as usize;

            // If at start of line, go to previous line
            if x == 0 {
                if y > 0 {
                    pos.y -= 1;
                    if let Some(prev_line) = contents.get(pos.y as usize) {
                        pos.x = prev_line.inner.len() as u16;
                    }
                    continue;
                }
                // At start of first line
                break;
            }

            x = x.saturating_sub(1);

            // Skip whitespace backward
            while x > 0 && chars.get(x).is_some_and(|c| c.is_whitespace()) {
                x -= 1;
            }

            // Handle case where we hit start of line while skipping whitespace
            if x == 0 && chars.first().is_some_and(|c| c.is_whitespace()) {
                if y > 0 {
                    pos.y -= 1;
                    if let Some(prev_line) = contents.get(pos.y as usize) {
                        pos.x = prev_line.inner.len() as u16;
                    }
                    continue;
                }
                pos.x = 0;
                break;
            }

            // Skip non-whitespace backward to find word start
            while x > 0 && chars.get(x - 1).is_some_and(|c| !c.is_whitespace()) {
                x -= 1;
            }

            pos.x = x as u16;
        }
        break;
    }
    pos
}
