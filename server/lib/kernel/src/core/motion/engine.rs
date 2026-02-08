//! Motion calculation engine.

use crate::{
    core::direction::{Direction, LinePosition, WordBoundary},
    mm::{Buffer, Cursor, Position},
};

use super::types::Motion;

/// Supported bracket pairs for % motion.
const BRACKET_PAIRS: [(char, char); 3] = [('(', ')'), ('[', ']'), ('{', '}')];

/// Motion calculation engine.
///
/// Provides pure calculations for cursor positions without modifying any state.
/// This is the "mechanism" that modules use to implement motion commands.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::*;
///
/// let buffer = Buffer::from_string("hello world");
/// let cursor = Cursor::new(Position::new(0, 0));
///
/// let new_pos = MotionEngine::calculate(
///     &buffer,
///     &cursor,
///     Motion::Char(Direction::Forward),
///     1,
/// );
///
/// assert_eq!(new_pos, Some(Position::new(0, 1)));
/// ```
pub struct MotionEngine;

impl MotionEngine {
    /// Calculate new position after applying a motion.
    ///
    /// Returns `None` if the motion is invalid or impossible.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The buffer to calculate motion in
    /// * `cursor` - Current cursor state (includes position and preferred column)
    /// * `motion` - The motion to apply
    /// * `count` - Number of times to apply the motion (minimum 1)
    #[must_use]
    pub fn calculate(
        buffer: &Buffer,
        cursor: &Cursor,
        motion: Motion,
        count: usize,
    ) -> Option<Position> {
        let count = count.max(1);

        match motion {
            Motion::Char(dir) => Self::char_motion(buffer, cursor.position, dir, count),
            Motion::Line(dir) => Self::line_motion(buffer, cursor, dir, count),
            Motion::Word {
                direction,
                boundary,
                end,
            } => Self::word_motion(buffer, cursor.position, direction, boundary, end, count),
            Motion::LinePosition(pos) => Self::line_position(buffer, cursor.position, pos),
            Motion::Paragraph(dir) => Self::paragraph_motion(buffer, cursor.position, dir, count),
            Motion::FindChar {
                char,
                direction,
                till,
            } => Self::find_char(buffer, cursor.position, char, direction, till, count),
            Motion::JumpLine(line) => Self::jump_line(buffer, cursor, line),
            Motion::MatchBracket => Self::match_bracket(buffer, cursor.position),
        }
    }

    /// Calculate new position and desired column after applying a motion.
    ///
    /// This variant is useful for vertical motions where the cursor should
    /// try to maintain its horizontal position across lines of different lengths.
    ///
    /// Returns `(new_position, new_desired_column)`
    #[must_use]
    pub fn calculate_with_desired_col(
        buffer: &Buffer,
        cursor: &Cursor,
        motion: Motion,
        count: usize,
    ) -> (Option<Position>, Option<usize>) {
        let count = count.max(1);

        if matches!(motion, Motion::Line(_)) {
            // For vertical motions, preserve or establish desired column
            let desired_col = cursor.preferred_column.unwrap_or(cursor.position.column);
            let new_pos = Self::calculate(buffer, cursor, motion, count);
            (new_pos, Some(desired_col))
        } else {
            // Horizontal motions clear the desired column
            let new_pos = Self::calculate(buffer, cursor, motion, count);
            (new_pos, None)
        }
    }

    // === Private Implementation ===

    fn char_motion(
        buffer: &Buffer,
        pos: Position,
        direction: Direction,
        count: usize,
    ) -> Option<Position> {
        let line_len = buffer.line_len(pos.line)?;

        match direction {
            Direction::Forward => {
                // Move right, clamped to line length minus 1 (stay on last char)
                let max_col = line_len.saturating_sub(1);
                let new_col = pos.column.saturating_add(count).min(max_col);
                Some(Position::new(pos.line, new_col))
            }
            Direction::Backward => {
                // Move left, clamped to 0
                let new_col = pos.column.saturating_sub(count);
                Some(Position::new(pos.line, new_col))
            }
        }
    }

    fn line_motion(
        buffer: &Buffer,
        cursor: &Cursor,
        direction: Direction,
        count: usize,
    ) -> Option<Position> {
        let line_count = buffer.line_count();
        if line_count == 0 {
            return None;
        }

        let new_line = match direction {
            Direction::Forward => {
                // Move down
                let target = cursor.position.line.saturating_add(count);
                target.min(line_count.saturating_sub(1))
            }
            Direction::Backward => {
                // Move up
                cursor.position.line.saturating_sub(count)
            }
        };

        // Use preferred column if set, otherwise current column
        let target_col = cursor.effective_column();
        let line_len = buffer.line_len(new_line)?;
        let max_col = if line_len == 0 {
            0
        } else {
            line_len.saturating_sub(1)
        };
        let new_col = target_col.min(max_col);

        Some(Position::new(new_line, new_col))
    }

    fn word_motion(
        buffer: &Buffer,
        pos: Position,
        direction: Direction,
        boundary: WordBoundary,
        end: bool,
        count: usize,
    ) -> Option<Position> {
        let mut current = pos;

        for _ in 0..count {
            current = match (direction, end) {
                (Direction::Forward, false) => Self::word_forward(buffer, current, boundary)?,
                (Direction::Forward, true) => Self::word_end(buffer, current, boundary)?,
                (Direction::Backward, false) => Self::word_backward(buffer, current, boundary)?,
                (Direction::Backward, true) => Self::word_end_backward(buffer, current, boundary)?,
            };
        }

        Some(current)
    }

    fn word_forward(
        buffer: &Buffer,
        mut pos: Position,
        boundary: WordBoundary,
    ) -> Option<Position> {
        let line_count = buffer.line_count();
        if line_count == 0 {
            return Some(pos);
        }

        loop {
            if pos.line >= line_count {
                pos.line = line_count.saturating_sub(1);
                pos.column = buffer.line_len(pos.line).unwrap_or(0).saturating_sub(1);
                break;
            }

            let line = buffer.line(pos.line)?;
            let chars: Vec<char> = line.chars().collect();
            let mut x = pos.column;

            // Skip current word (non-whitespace for BigWord, word chars or punctuation for Word)
            if boundary == WordBoundary::Word {
                // For small word, we need to handle word chars and punctuation separately
                if let Some(&c) = chars.get(x) {
                    if c.is_alphanumeric() || c == '_' {
                        // Skip word characters
                        while x < chars.len() && (chars[x].is_alphanumeric() || chars[x] == '_') {
                            x += 1;
                        }
                    } else if !c.is_whitespace() {
                        // Skip punctuation
                        while x < chars.len()
                            && !chars[x].is_whitespace()
                            && !chars[x].is_alphanumeric()
                            && chars[x] != '_'
                        {
                            x += 1;
                        }
                    }
                }
            } else {
                // BigWord: skip non-whitespace
                while x < chars.len() && !chars[x].is_whitespace() {
                    x += 1;
                }
            }

            // Skip whitespace
            while x < chars.len() && chars[x].is_whitespace() {
                x += 1;
            }

            if x < chars.len() {
                pos.column = x;
                break;
            }

            // Reached end of line, try next line
            if pos.line + 1 < line_count {
                pos.line += 1;
                pos.column = 0;

                // Skip leading whitespace on new line
                if let Some(next_line) = buffer.line(pos.line) {
                    let next_chars: Vec<char> = next_line.chars().collect();
                    let mut nx = 0;
                    while nx < next_chars.len() && next_chars[nx].is_whitespace() {
                        nx += 1;
                    }
                    if nx < next_chars.len() {
                        pos.column = nx;
                        break;
                    }
                }
            } else {
                pos.column = chars.len().saturating_sub(1);
                break;
            }
        }

        Some(pos)
    }

    fn word_end(buffer: &Buffer, mut pos: Position, boundary: WordBoundary) -> Option<Position> {
        let line_count = buffer.line_count();
        if line_count == 0 {
            return Some(pos);
        }

        // First, move forward one position
        let line_len = buffer.line_len(pos.line)?;
        if pos.column + 1 < line_len {
            pos.column += 1;
        } else if pos.line + 1 < line_count {
            pos.line += 1;
            pos.column = 0;
        }

        loop {
            if pos.line >= line_count {
                pos.line = line_count.saturating_sub(1);
                pos.column = buffer.line_len(pos.line).unwrap_or(0).saturating_sub(1);
                break;
            }

            let line = buffer.line(pos.line)?;
            let chars: Vec<char> = line.chars().collect();
            let mut x = pos.column;

            // Skip whitespace
            while x < chars.len() && chars[x].is_whitespace() {
                x += 1;
            }

            if x >= chars.len() {
                // End of line, try next
                if pos.line + 1 < line_count {
                    pos.line += 1;
                    pos.column = 0;
                    continue;
                }
                pos.column = chars.len().saturating_sub(1);
                break;
            }

            // Find end of word
            if boundary == WordBoundary::Word {
                let is_word_char = chars[x].is_alphanumeric() || chars[x] == '_';
                if is_word_char {
                    while x + 1 < chars.len()
                        && (chars[x + 1].is_alphanumeric() || chars[x + 1] == '_')
                    {
                        x += 1;
                    }
                } else {
                    while x + 1 < chars.len()
                        && !chars[x + 1].is_whitespace()
                        && !chars[x + 1].is_alphanumeric()
                        && chars[x + 1] != '_'
                    {
                        x += 1;
                    }
                }
            } else {
                // BigWord: find end of non-whitespace
                while x + 1 < chars.len() && !chars[x + 1].is_whitespace() {
                    x += 1;
                }
            }

            pos.column = x;
            break;
        }

        Some(pos)
    }

    fn word_backward(
        buffer: &Buffer,
        mut pos: Position,
        boundary: WordBoundary,
    ) -> Option<Position> {
        if buffer.is_empty() {
            return Some(pos);
        }

        loop {
            let line = buffer.line(pos.line)?;
            let chars: Vec<char> = line.chars().collect();

            // If at start of line, go to previous line
            if pos.column == 0 {
                if pos.line > 0 {
                    pos.line -= 1;
                    pos.column = buffer.line_len(pos.line).unwrap_or(0);
                    continue;
                }
                break;
            }

            let mut x = pos.column.saturating_sub(1);

            // Skip whitespace backward
            while x > 0 && chars.get(x).is_some_and(|c| c.is_whitespace()) {
                x -= 1;
            }

            // Handle case where we hit start of line while skipping whitespace
            if x == 0 && chars.first().is_some_and(|c| c.is_whitespace()) {
                if pos.line > 0 {
                    pos.line -= 1;
                    pos.column = buffer.line_len(pos.line).unwrap_or(0);
                    continue;
                }
                pos.column = 0;
                break;
            }

            // Find start of word
            if boundary == WordBoundary::Word {
                if let Some(&c) = chars.get(x) {
                    if c.is_alphanumeric() || c == '_' {
                        while x > 0
                            && chars
                                .get(x - 1)
                                .is_some_and(|c| c.is_alphanumeric() || *c == '_')
                        {
                            x -= 1;
                        }
                    } else if !c.is_whitespace() {
                        while x > 0
                            && chars.get(x - 1).is_some_and(|c| {
                                !c.is_whitespace() && !c.is_alphanumeric() && *c != '_'
                            })
                        {
                            x -= 1;
                        }
                    }
                }
            } else {
                // BigWord: find start of non-whitespace
                while x > 0 && chars.get(x - 1).is_some_and(|c| !c.is_whitespace()) {
                    x -= 1;
                }
            }

            pos.column = x;
            break;
        }

        Some(pos)
    }

    fn word_end_backward(
        buffer: &Buffer,
        mut pos: Position,
        _boundary: WordBoundary,
    ) -> Option<Position> {
        // ge/gE: Move backward to end of previous word
        // Note: For ge/gE, the boundary distinction mainly affects where we stop,
        // but the basic algorithm of finding end of previous word is similar.
        // TODO: Implement full word/BigWord distinction if needed.
        if buffer.is_empty() {
            return Some(pos);
        }

        // First move back one position
        if pos.column > 0 {
            pos.column -= 1;
        } else if pos.line > 0 {
            pos.line -= 1;
            pos.column = buffer.line_len(pos.line).unwrap_or(0).saturating_sub(1);
        }

        loop {
            let line = buffer.line(pos.line)?;
            let chars: Vec<char> = line.chars().collect();

            if chars.is_empty() {
                if pos.line > 0 {
                    pos.line -= 1;
                    pos.column = buffer.line_len(pos.line).unwrap_or(0).saturating_sub(1);
                    continue;
                }
                break;
            }

            let mut x = pos.column.min(chars.len().saturating_sub(1));

            // Skip whitespace backward
            while x > 0 && chars.get(x).is_some_and(|c| c.is_whitespace()) {
                x -= 1;
            }

            if chars.get(x).is_some_and(|c| c.is_whitespace()) {
                // Still on whitespace, go to previous line
                if pos.line > 0 {
                    pos.line -= 1;
                    pos.column = buffer.line_len(pos.line).unwrap_or(0).saturating_sub(1);
                    continue;
                }
                pos.column = 0;
                break;
            }

            // Now at end of a word
            pos.column = x;
            break;
        }

        Some(pos)
    }

    fn line_position(buffer: &Buffer, pos: Position, line_pos: LinePosition) -> Option<Position> {
        let line = buffer.line(pos.line)?;
        let chars: Vec<char> = line.chars().collect();
        let line_len = chars.len();

        let new_col = match line_pos {
            LinePosition::Start => 0,
            LinePosition::FirstNonBlank => {
                chars.iter().position(|c| !c.is_whitespace()).unwrap_or(0)
            }
            LinePosition::End => line_len.saturating_sub(1),
            LinePosition::LastNonBlank => chars
                .iter()
                .enumerate()
                .rev()
                .find(|(_, c)| !c.is_whitespace())
                .map_or(0, |(i, _)| i),
        };

        Some(Position::new(pos.line, new_col))
    }

    fn paragraph_motion(
        buffer: &Buffer,
        pos: Position,
        direction: Direction,
        count: usize,
    ) -> Option<Position> {
        let line_count = buffer.line_count();
        if line_count == 0 {
            return None;
        }

        let mut current_line = pos.line;
        let mut found = 0;

        match direction {
            Direction::Forward => {
                // Skip current paragraph (non-empty lines)
                while current_line < line_count {
                    let line = buffer.line(current_line)?;
                    if line.trim().is_empty() {
                        break;
                    }
                    current_line += 1;
                }

                // Find next paragraphs
                while current_line < line_count && found < count {
                    // Skip empty lines
                    while current_line < line_count {
                        let line = buffer.line(current_line)?;
                        if !line.trim().is_empty() {
                            break;
                        }
                        current_line += 1;
                    }

                    if current_line >= line_count {
                        break;
                    }

                    found += 1;
                    if found >= count {
                        break;
                    }

                    // Skip non-empty lines
                    while current_line < line_count {
                        let line = buffer.line(current_line)?;
                        if line.trim().is_empty() {
                            break;
                        }
                        current_line += 1;
                    }
                }
            }
            Direction::Backward => {
                // Skip current paragraph
                while current_line > 0 {
                    let line = buffer.line(current_line)?;
                    if line.trim().is_empty() {
                        break;
                    }
                    current_line -= 1;
                }

                // Find previous paragraphs
                while current_line > 0 && found < count {
                    // Skip empty lines
                    while current_line > 0 {
                        let line = buffer.line(current_line)?;
                        if !line.trim().is_empty() {
                            break;
                        }
                        current_line -= 1;
                    }

                    if current_line == 0 {
                        let line = buffer.line(0)?;
                        if line.trim().is_empty() {
                            break;
                        }
                    }

                    found += 1;
                    if found >= count {
                        // Find start of this paragraph
                        while current_line > 0 {
                            let prev = buffer.line(current_line - 1)?;
                            if prev.trim().is_empty() {
                                break;
                            }
                            current_line -= 1;
                        }
                        break;
                    }

                    // Skip non-empty lines
                    while current_line > 0 {
                        let line = buffer.line(current_line)?;
                        if line.trim().is_empty() {
                            break;
                        }
                        current_line -= 1;
                    }
                }
            }
        }

        Some(Position::new(current_line.min(line_count.saturating_sub(1)), 0))
    }

    fn find_char(
        buffer: &Buffer,
        pos: Position,
        target: char,
        direction: Direction,
        till: bool,
        count: usize,
    ) -> Option<Position> {
        let line = buffer.line(pos.line)?;
        let chars: Vec<char> = line.chars().collect();

        let mut found_count = 0;
        let mut found_pos = None;

        match direction {
            Direction::Forward => {
                for (i, &c) in chars.iter().enumerate().skip(pos.column + 1) {
                    if c == target {
                        found_count += 1;
                        if found_count == count {
                            found_pos = Some(i);
                            break;
                        }
                    }
                }
            }
            Direction::Backward => {
                for i in (0..pos.column).rev() {
                    if chars.get(i) == Some(&target) {
                        found_count += 1;
                        if found_count == count {
                            found_pos = Some(i);
                            break;
                        }
                    }
                }
            }
        }

        found_pos.map(|col| {
            let adjusted_col = if till {
                match direction {
                    Direction::Forward => col.saturating_sub(1).max(pos.column),
                    Direction::Backward => col.saturating_add(1).min(chars.len().saturating_sub(1)),
                }
            } else {
                col
            };
            Position::new(pos.line, adjusted_col)
        })
    }

    fn jump_line(buffer: &Buffer, _cursor: &Cursor, target: Option<usize>) -> Option<Position> {
        let line_count = buffer.line_count();
        if line_count == 0 {
            return None;
        }

        // G without count goes to last line, with count goes to that line
        let max_line = line_count.saturating_sub(1);
        let new_line = target.map_or(max_line, |line| line.min(max_line));

        // Jump to first non-blank on target line
        let line_content = buffer.line(new_line)?;
        let first_non_blank = line_content
            .chars()
            .position(|c| !c.is_whitespace())
            .unwrap_or(0);

        Some(Position::new(new_line, first_non_blank))
    }

    fn match_bracket(buffer: &Buffer, pos: Position) -> Option<Position> {
        let line = buffer.line(pos.line)?;
        let chars: Vec<char> = line.chars().collect();
        let cursor_char = chars.get(pos.column).copied();

        // If cursor is on a bracket, find its match
        if let Some(ch) = cursor_char {
            for (open, close) in BRACKET_PAIRS {
                if ch == open {
                    return Self::find_forward_bracket(buffer, pos, open, close);
                } else if ch == close {
                    return Self::find_backward_bracket(buffer, pos, open, close);
                }
            }
        }

        // Search forward on current line for a bracket
        for (offset, &ch) in chars.iter().enumerate().skip(pos.column + 1) {
            for (open, close) in BRACKET_PAIRS {
                if ch == open {
                    let search_pos = Position::new(pos.line, offset);
                    return Self::find_forward_bracket(buffer, search_pos, open, close);
                } else if ch == close {
                    let search_pos = Position::new(pos.line, offset);
                    return Self::find_backward_bracket(buffer, search_pos, open, close);
                }
            }
        }

        None
    }

    fn find_forward_bracket(
        buffer: &Buffer,
        start: Position,
        open: char,
        close: char,
    ) -> Option<Position> {
        let mut depth = 1;
        let mut line_idx = start.line;
        let mut col = start.column + 1;

        while line_idx < buffer.line_count() {
            let line = buffer.line(line_idx)?;
            let chars: Vec<char> = line.chars().collect();

            while col < chars.len() {
                let ch = chars[col];
                if ch == open {
                    depth += 1;
                } else if ch == close {
                    depth -= 1;
                    if depth == 0 {
                        return Some(Position::new(line_idx, col));
                    }
                }
                col += 1;
            }

            line_idx += 1;
            col = 0;
        }

        None
    }

    fn find_backward_bracket(
        buffer: &Buffer,
        start: Position,
        open: char,
        close: char,
    ) -> Option<Position> {
        let mut depth = 1;
        let mut line_idx = start.line;

        // Handle first line specially
        if let Some(line) = buffer.line(line_idx) {
            let chars: Vec<char> = line.chars().collect();
            if start.column > 0 {
                for col in (0..start.column).rev() {
                    let ch = chars[col];
                    if ch == close {
                        depth += 1;
                    } else if ch == open {
                        depth -= 1;
                        if depth == 0 {
                            return Some(Position::new(line_idx, col));
                        }
                    }
                }
            }
        }

        // Continue scanning previous lines
        while line_idx > 0 {
            line_idx -= 1;
            let line = buffer.line(line_idx)?;
            let chars: Vec<char> = line.chars().collect();

            for col in (0..chars.len()).rev() {
                let ch = chars[col];
                if ch == close {
                    depth += 1;
                } else if ch == open {
                    depth -= 1;
                    if depth == 0 {
                        return Some(Position::new(line_idx, col));
                    }
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_buffer(content: &str) -> Buffer {
        Buffer::from_string(content)
    }

    fn make_cursor(line: usize, column: usize) -> Cursor {
        Cursor::new(Position::new(line, column))
    }

    #[test]
    fn test_char_motion_forward() {
        let buffer = make_buffer("hello");
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Char(Direction::Forward), 1);
        assert_eq!(pos, Some(Position::new(0, 1)));

        // With count
        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Char(Direction::Forward), 3);
        assert_eq!(pos, Some(Position::new(0, 3)));

        // Clamped to line end
        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Char(Direction::Forward), 10);
        assert_eq!(pos, Some(Position::new(0, 4))); // "hello" has 5 chars, max column is 4
    }

    #[test]
    fn test_char_motion_backward() {
        let buffer = make_buffer("hello");
        let cursor = make_cursor(0, 4);

        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Char(Direction::Backward), 1);
        assert_eq!(pos, Some(Position::new(0, 3)));

        // Clamped to 0
        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Char(Direction::Backward), 10);
        assert_eq!(pos, Some(Position::new(0, 0)));
    }

    #[test]
    fn test_line_motion() {
        let buffer = make_buffer("line1\nline2\nline3");
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Line(Direction::Forward), 1);
        assert_eq!(pos, Some(Position::new(1, 0)));

        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Line(Direction::Forward), 2);
        assert_eq!(pos, Some(Position::new(2, 0)));

        // Clamped to last line
        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Line(Direction::Forward), 10);
        assert_eq!(pos, Some(Position::new(2, 0)));
    }

    #[test]
    fn test_line_position() {
        let buffer = make_buffer("  hello world  ");
        let cursor = make_cursor(0, 5);

        let pos =
            MotionEngine::calculate(&buffer, &cursor, Motion::LinePosition(LinePosition::Start), 1);
        assert_eq!(pos, Some(Position::new(0, 0)));

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::LinePosition(LinePosition::FirstNonBlank),
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 2))); // First 'h'

        let pos =
            MotionEngine::calculate(&buffer, &cursor, Motion::LinePosition(LinePosition::End), 1);
        assert_eq!(pos, Some(Position::new(0, 14))); // Last char index

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::LinePosition(LinePosition::LastNonBlank),
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 12))); // Last 'd'
    }

    #[test]
    fn test_word_forward() {
        let buffer = make_buffer("hello world foo");
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Forward,
                boundary: WordBoundary::Word,
                end: false,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 6))); // 'w' of world
    }

    #[test]
    fn test_word_backward() {
        let buffer = make_buffer("hello world foo");
        let cursor = make_cursor(0, 12);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Backward,
                boundary: WordBoundary::Word,
                end: false,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 6))); // 'w' of world
    }

    #[test]
    fn test_word_end() {
        let buffer = make_buffer("hello world foo");
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Forward,
                boundary: WordBoundary::Word,
                end: true,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 4))); // 'o' of hello
    }

    #[test]
    fn test_jump_line() {
        let buffer = make_buffer("line0\nline1\nline2");
        let cursor = make_cursor(0, 0);

        // G (no count) goes to last line
        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::JumpLine(None), 1);
        assert_eq!(pos, Some(Position::new(2, 0)));

        // Jump to specific line
        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::JumpLine(Some(1)), 1);
        assert_eq!(pos, Some(Position::new(1, 0)));
    }

    #[test]
    fn test_match_bracket() {
        let buffer = make_buffer("(hello)");
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
        assert_eq!(pos, Some(Position::new(0, 6))); // closing )

        // From closing bracket
        let cursor = make_cursor(0, 6);
        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
        assert_eq!(pos, Some(Position::new(0, 0))); // opening (
    }

    #[test]
    fn test_match_bracket_nested() {
        let buffer = make_buffer("((inner))");
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
        assert_eq!(pos, Some(Position::new(0, 8))); // outermost closing )
    }

    #[test]
    fn test_find_char() {
        let buffer = make_buffer("hello world");
        let cursor = make_cursor(0, 0);

        // f (find forward)
        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::FindChar {
                char: 'w',
                direction: Direction::Forward,
                till: false,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 6)));

        // t (till forward)
        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::FindChar {
                char: 'w',
                direction: Direction::Forward,
                till: true,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 5)));
    }

    #[test]
    fn test_paragraph_motion() {
        let buffer = make_buffer("para1\n\npara2\npara2b\n\npara3");
        let cursor = make_cursor(0, 0);

        // } (forward)
        let pos =
            MotionEngine::calculate(&buffer, &cursor, Motion::Paragraph(Direction::Forward), 1);
        assert_eq!(pos.map(|p| p.line), Some(2)); // Start of para2

        // { (backward) from para3
        let cursor = make_cursor(5, 0);
        let pos =
            MotionEngine::calculate(&buffer, &cursor, Motion::Paragraph(Direction::Backward), 1);
        assert_eq!(pos.map(|p| p.line), Some(2)); // Start of para2
    }

    // === Coverage: calculate_with_desired_col ===

    #[test]
    fn test_calculate_with_desired_col_vertical() {
        let buffer = make_buffer("hello\nhi\nworld");
        let cursor = make_cursor(0, 4);

        // Vertical motion should preserve desired column
        let (new_pos, desired_col) = MotionEngine::calculate_with_desired_col(
            &buffer,
            &cursor,
            Motion::Line(Direction::Forward),
            1,
        );
        assert_eq!(new_pos, Some(Position::new(1, 1))); // "hi" only 2 chars, clamped
        assert_eq!(desired_col, Some(4)); // Desired column preserved
    }

    #[test]
    fn test_calculate_with_desired_col_horizontal() {
        let buffer = make_buffer("hello");
        let cursor = make_cursor(0, 0);

        // Horizontal motion should clear desired column
        let (new_pos, desired_col) = MotionEngine::calculate_with_desired_col(
            &buffer,
            &cursor,
            Motion::Char(Direction::Forward),
            1,
        );
        assert_eq!(new_pos, Some(Position::new(0, 1)));
        assert_eq!(desired_col, None); // Cleared for horizontal
    }

    #[test]
    fn test_calculate_with_desired_col_with_preferred() {
        let buffer = make_buffer("hello\nhi\nworld");
        let mut cursor = make_cursor(0, 4);
        cursor.preferred_column = Some(4);

        let (new_pos, desired_col) = MotionEngine::calculate_with_desired_col(
            &buffer,
            &cursor,
            Motion::Line(Direction::Forward),
            1,
        );
        assert_eq!(new_pos, Some(Position::new(1, 1)));
        assert_eq!(desired_col, Some(4));
    }

    // === Coverage: BigWord motions ===

    #[test]
    fn test_big_word_forward() {
        let buffer = make_buffer("hello.world foo");
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Forward,
                boundary: WordBoundary::BigWord,
                end: false,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 12))); // 'f' of foo (skip hello.world as one WORD)
    }

    #[test]
    fn test_big_word_end() {
        let buffer = make_buffer("hello.world foo");
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Forward,
                boundary: WordBoundary::BigWord,
                end: true,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 10))); // 'd' of hello.world
    }

    #[test]
    fn test_big_word_backward() {
        let buffer = make_buffer("hello.world foo");
        let cursor = make_cursor(0, 14);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Backward,
                boundary: WordBoundary::BigWord,
                end: false,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 12))); // 'f' of foo
    }

    // === Coverage: word_end_backward (ge motion) ===

    #[test]
    fn test_word_end_backward() {
        let buffer = make_buffer("hello world foo");
        let cursor = make_cursor(0, 12);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Backward,
                boundary: WordBoundary::Word,
                end: true,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 10))); // 'd' of world
    }

    #[test]
    fn test_word_end_backward_at_start() {
        let buffer = make_buffer("hello world");
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Backward,
                boundary: WordBoundary::Word,
                end: true,
            },
            1,
        );
        // At start, backs up to (empty buffer check passes, then backs up 1)
        assert!(pos.is_some());
    }

    #[test]
    fn test_word_end_backward_empty_buffer() {
        let buffer = Buffer::new();
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Backward,
                boundary: WordBoundary::Word,
                end: true,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 0)));
    }

    #[test]
    fn test_word_end_backward_multiline() {
        let buffer = make_buffer("hello\n\nworld");
        let cursor = make_cursor(2, 0);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Backward,
                boundary: WordBoundary::Word,
                end: true,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 4))); // 'o' of hello
    }

    // === Coverage: backward paragraph with multiple counts ===

    #[test]
    fn test_paragraph_backward_multiple_count() {
        let buffer = make_buffer("para1\n\npara2\n\npara3");
        let cursor = make_cursor(4, 0);

        let pos =
            MotionEngine::calculate(&buffer, &cursor, Motion::Paragraph(Direction::Backward), 2);
        assert_eq!(pos.map(|p| p.line), Some(0)); // Back to para1
    }

    #[test]
    fn test_paragraph_forward_multiple_count() {
        let buffer = make_buffer("para1\n\npara2\n\npara3");
        let cursor = make_cursor(0, 0);

        let pos =
            MotionEngine::calculate(&buffer, &cursor, Motion::Paragraph(Direction::Forward), 2);
        assert_eq!(pos.map(|p| p.line), Some(4)); // Start of para3
    }

    #[test]
    fn test_paragraph_empty_buffer() {
        let buffer = Buffer::new();
        let cursor = make_cursor(0, 0);

        let pos =
            MotionEngine::calculate(&buffer, &cursor, Motion::Paragraph(Direction::Forward), 1);
        assert_eq!(pos, None);
    }

    // === Coverage: find_char backward and till backward ===

    #[test]
    fn test_find_char_backward_hello() {
        let buffer = make_buffer("hello world");
        let cursor = make_cursor(0, 10);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::FindChar {
                char: 'o',
                direction: Direction::Backward,
                till: false,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 7))); // 'o' in world
    }

    #[test]
    fn test_find_char_till_backward() {
        let buffer = make_buffer("hello world");
        let cursor = make_cursor(0, 10);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::FindChar {
                char: 'o',
                direction: Direction::Backward,
                till: true,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 8))); // one after 'o'
    }

    #[test]
    fn test_find_char_not_found() {
        let buffer = make_buffer("hello");
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::FindChar {
                char: 'z',
                direction: Direction::Forward,
                till: false,
            },
            1,
        );
        assert_eq!(pos, None);
    }

    #[test]
    fn test_find_char_with_count() {
        let buffer = make_buffer("aabaa");
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::FindChar {
                char: 'a',
                direction: Direction::Forward,
                till: false,
            },
            2,
        );
        assert_eq!(pos, Some(Position::new(0, 3))); // second 'a' after cursor
    }

    // === Coverage: match_bracket search forward on line ===

    #[test]
    fn test_match_bracket_search_forward() {
        let buffer = make_buffer("hello (world)");
        let cursor = make_cursor(0, 0); // Not on a bracket

        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
        assert_eq!(pos, Some(Position::new(0, 12))); // Matching )
    }

    #[test]
    fn test_match_bracket_multiline() {
        let buffer = make_buffer("(\n  content\n)");
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
        assert_eq!(pos, Some(Position::new(2, 0)));
    }

    #[test]
    fn test_match_bracket_multiline_backward() {
        let buffer = make_buffer("(\n  content\n)");
        let cursor = make_cursor(2, 0);

        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
        assert_eq!(pos, Some(Position::new(0, 0)));
    }

    #[test]
    fn test_match_bracket_square() {
        let buffer = make_buffer("[hello]");
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
        assert_eq!(pos, Some(Position::new(0, 6)));

        let cursor = make_cursor(0, 6);
        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
        assert_eq!(pos, Some(Position::new(0, 0)));
    }

    #[test]
    fn test_match_bracket_curly() {
        let buffer = make_buffer("{hello}");
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
        assert_eq!(pos, Some(Position::new(0, 6)));

        let cursor = make_cursor(0, 6);
        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
        assert_eq!(pos, Some(Position::new(0, 0)));
    }

    #[test]
    fn test_match_bracket_no_bracket() {
        let buffer = make_buffer("hello world");
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
        assert_eq!(pos, None);
    }

    // === Coverage: word_forward punctuation ===

    #[test]
    fn test_word_forward_punctuation() {
        let buffer = make_buffer("foo::bar baz");
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Forward,
                boundary: WordBoundary::Word,
                end: false,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 3))); // :: is punctuation
    }

    #[test]
    fn test_word_backward_punctuation() {
        let buffer = make_buffer("foo::bar");
        let cursor = make_cursor(0, 7);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Backward,
                boundary: WordBoundary::Word,
                end: false,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 5))); // 'b' of bar
    }

    // === Coverage: word_end punctuation ===

    #[test]
    fn test_word_end_punctuation() {
        let buffer = make_buffer("foo::bar");
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Forward,
                boundary: WordBoundary::Word,
                end: true,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 2))); // last 'o' of foo
    }

    // === Coverage: line_motion edge cases ===

    #[test]
    fn test_line_motion_backward() {
        let buffer = make_buffer("line1\nline2\nline3");
        let cursor = make_cursor(2, 0);

        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Line(Direction::Backward), 1);
        assert_eq!(pos, Some(Position::new(1, 0)));

        // Clamped to first line
        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Line(Direction::Backward), 10);
        assert_eq!(pos, Some(Position::new(0, 0)));
    }

    #[test]
    fn test_line_motion_empty_buffer() {
        let buffer = Buffer::new();
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Line(Direction::Forward), 1);
        assert_eq!(pos, None);
    }

    #[test]
    fn test_line_motion_empty_target_line() {
        let buffer = make_buffer("hello\n\nworld");
        let cursor = make_cursor(0, 3);

        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Line(Direction::Forward), 1);
        assert_eq!(pos, Some(Position::new(1, 0))); // empty line, col clamped to 0
    }

    // === Coverage: jump_line edge cases ===

    #[test]
    fn test_jump_line_with_indented_target() {
        let buffer = make_buffer("hello\n  world");
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::JumpLine(Some(1)), 1);
        assert_eq!(pos, Some(Position::new(1, 2))); // first non-blank is 'w'
    }

    #[test]
    fn test_jump_line_beyond_end() {
        let buffer = make_buffer("line1\nline2");
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::JumpLine(Some(100)), 1);
        assert_eq!(pos, Some(Position::new(1, 0))); // clamped to last line
    }

    #[test]
    fn test_jump_line_empty_buffer() {
        let buffer = Buffer::new();
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::JumpLine(None), 1);
        assert_eq!(pos, None);
    }

    // === Coverage: word_forward multiline ===

    #[test]
    fn test_word_forward_multiline() {
        let buffer = make_buffer("hello\nworld");
        let cursor = make_cursor(0, 4);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Forward,
                boundary: WordBoundary::Word,
                end: false,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(1, 0))); // 'w' of world
    }

    #[test]
    fn test_word_backward_multiline() {
        let buffer = make_buffer("hello\nworld");
        let cursor = make_cursor(1, 0);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Backward,
                boundary: WordBoundary::Word,
                end: false,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 0))); // 'h' of hello
    }

    // === Coverage: word_forward starting on whitespace ===

    #[test]
    fn test_word_forward_from_whitespace() {
        let buffer = make_buffer("  hello");
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Forward,
                boundary: WordBoundary::Word,
                end: false,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 2))); // 'h' of hello
    }

    // === Coverage: count 0 treated as 1 ===

    #[test]
    fn test_count_zero_treated_as_one() {
        let buffer = make_buffer("hello");
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Char(Direction::Forward), 0);
        assert_eq!(pos, Some(Position::new(0, 1))); // count 0 -> 1
    }

    // === Coverage: line_position edge cases ===

    #[test]
    fn test_line_position_all_whitespace() {
        let buffer = make_buffer("   ");
        let cursor = make_cursor(0, 1);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::LinePosition(LinePosition::FirstNonBlank),
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 0))); // no non-blank, defaults to 0

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::LinePosition(LinePosition::LastNonBlank),
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 0)));
    }

    // === Coverage: word_forward BigWord on whitespace start ===

    #[test]
    fn test_word_forward_bigword_from_whitespace() {
        let buffer = make_buffer("  hello.world");
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Forward,
                boundary: WordBoundary::BigWord,
                end: false,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 2)));
    }

    // === Coverage: word_forward punctuation skip ===

    #[test]
    fn test_word_forward_punctuation_skip() {
        let buffer = make_buffer("foo::bar baz");
        let cursor = make_cursor(0, 3); // on first ':'

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Forward,
                boundary: WordBoundary::Word,
                end: false,
            },
            1,
        );
        // Skip '::' punctuation, then 'bar', then ' ', land on 'baz'
        assert_eq!(pos, Some(Position::new(0, 5)));
    }

    // === Coverage: word_forward cross-line with whitespace-only lines ===

    #[test]
    fn test_word_forward_across_whitespace_only_line() {
        let buffer = make_buffer("hello\n   \nworld");
        let cursor = make_cursor(0, 4);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Forward,
                boundary: WordBoundary::Word,
                end: false,
            },
            1,
        );
        // Should skip whitespace-only line and land on 'world'
        assert_eq!(pos, Some(Position::new(2, 0)));
    }

    // === Coverage: word_forward at very end of buffer ===

    #[test]
    fn test_word_forward_at_end_of_buffer() {
        let buffer = make_buffer("hello");
        let cursor = make_cursor(0, 4);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Forward,
                boundary: WordBoundary::Word,
                end: false,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 4)));
    }

    // === Coverage: word_end BigWord ===

    // === Coverage: word_end cross-line ===

    #[test]
    fn test_word_end_cross_line() {
        let buffer = make_buffer("hello\nworld");
        let cursor = make_cursor(0, 4); // at end of 'hello'

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Forward,
                boundary: WordBoundary::Word,
                end: true,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(1, 4)));
    }

    #[test]
    fn test_word_end_empty_buffer() {
        let buffer = Buffer::new();
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Forward,
                boundary: WordBoundary::Word,
                end: true,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 0)));
    }

    #[test]
    fn test_word_end_across_whitespace_only_line() {
        let buffer = make_buffer("hello\n   \nworld");
        let cursor = make_cursor(0, 4);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Forward,
                boundary: WordBoundary::Word,
                end: true,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(2, 4)));
    }

    // === Coverage: word_end punctuation in Word mode ===

    #[test]
    fn test_word_end_on_punctuation() {
        let buffer = make_buffer("foo::bar");
        let cursor = make_cursor(0, 2); // 'o' of foo, next should be '::'

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Forward,
                boundary: WordBoundary::Word,
                end: true,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 4))); // end of '::'
    }

    // === Coverage: word_backward empty buffer ===

    #[test]
    fn test_word_backward_empty_buffer() {
        let buffer = Buffer::new();
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Backward,
                boundary: WordBoundary::Word,
                end: false,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 0)));
    }

    // === Coverage: word_backward cross-line with whitespace-only first line ===

    #[test]
    fn test_word_backward_across_whitespace_line() {
        let buffer = make_buffer("hello\n   \nworld");
        let cursor = make_cursor(2, 0);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Backward,
                boundary: WordBoundary::Word,
                end: false,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 0)));
    }

    // === Coverage: word_backward BigWord ===

    #[test]
    fn test_word_backward_bigword() {
        let buffer = make_buffer("hello foo.bar baz");
        let cursor = make_cursor(0, 14);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Backward,
                boundary: WordBoundary::BigWord,
                end: false,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 6)));
    }

    // === Coverage: word_backward punctuation ===

    #[test]
    fn test_word_backward_from_punct() {
        let buffer = make_buffer("foo::bar");
        let cursor = make_cursor(0, 4);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Backward,
                boundary: WordBoundary::Word,
                end: false,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 3)));
    }

    // === Coverage: word_end_backward ===

    #[test]
    fn test_word_end_backward_basic() {
        let buffer = make_buffer("hello world");
        let cursor = make_cursor(0, 10);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Backward,
                boundary: WordBoundary::Word,
                end: true,
            },
            1,
        );
        // ge from 'd' (col 10) moves backward
        assert!(pos.is_some());
    }

    #[test]
    fn test_word_end_backward_cross_line() {
        let buffer = make_buffer("hello\nworld");
        let cursor = make_cursor(1, 0);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Backward,
                boundary: WordBoundary::Word,
                end: true,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 4)));
    }

    #[test]
    fn test_word_end_backward_whitespace_only_line() {
        let buffer = make_buffer("hello\n   \nworld");
        let cursor = make_cursor(2, 0);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Backward,
                boundary: WordBoundary::Word,
                end: true,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 4)));
    }

    // === Coverage: paragraph backward with count ===

    #[test]
    fn test_paragraph_backward_count2() {
        let buffer = make_buffer("para1\n\npara2\n\npara3");
        let cursor = make_cursor(4, 0);

        let pos =
            MotionEngine::calculate(&buffer, &cursor, Motion::Paragraph(Direction::Backward), 2);
        assert!(pos.is_some());
        assert_eq!(pos.unwrap().line, 0);
    }

    #[test]
    fn test_paragraph_backward_from_empty_line() {
        let buffer = make_buffer("para1\n\npara2");
        let cursor = make_cursor(1, 0); // on empty line

        let pos =
            MotionEngine::calculate(&buffer, &cursor, Motion::Paragraph(Direction::Backward), 1);
        assert!(pos.is_some());
        assert_eq!(pos.unwrap().line, 0);
    }

    #[test]
    fn test_paragraph_forward_count2() {
        let buffer = make_buffer("para1\n\npara2\n\npara3");
        let cursor = make_cursor(0, 0);

        let pos =
            MotionEngine::calculate(&buffer, &cursor, Motion::Paragraph(Direction::Forward), 2);
        assert!(pos.is_some());
        assert_eq!(pos.unwrap().line, 4);
    }

    // === Coverage: find_char backward ===

    #[test]
    fn test_find_char_backward() {
        let buffer = make_buffer("abcabc");
        let cursor = make_cursor(0, 5);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::FindChar {
                char: 'a',
                direction: Direction::Backward,
                till: false,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 3)));
    }

    #[test]
    fn test_find_char_backward_till() {
        let buffer = make_buffer("abcabc");
        let cursor = make_cursor(0, 5);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::FindChar {
                char: 'a',
                direction: Direction::Backward,
                till: true,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 4)));
    }

    #[test]
    fn test_find_char_backward_count2() {
        let buffer = make_buffer("abcabc");
        let cursor = make_cursor(0, 5);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::FindChar {
                char: 'a',
                direction: Direction::Backward,
                till: false,
            },
            2,
        );
        assert_eq!(pos, Some(Position::new(0, 0)));
    }

    // === Coverage: match_bracket forward search on same line ===

    #[test]
    fn test_match_bracket_closing_search_forward() {
        let buffer = make_buffer("x ) ( y");
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
        // Forward search finds ')' at col 2 but no matching '(' before it
        assert_eq!(pos, None);
    }

    #[test]
    fn test_match_bracket_multiline_forward() {
        let buffer = make_buffer("(\nhello\n)");
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
        assert_eq!(pos, Some(Position::new(2, 0)));
    }

    #[test]
    fn test_match_bracket_multiline_backward_newline() {
        let buffer = make_buffer("(\nhello\n)");
        let cursor = make_cursor(2, 0);

        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
        assert_eq!(pos, Some(Position::new(0, 0)));
    }

    #[test]
    fn test_match_bracket_square_simple() {
        let buffer = make_buffer("[hello]");
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
        assert_eq!(pos, Some(Position::new(0, 6)));
    }

    #[test]
    fn test_match_bracket_curly_simple() {
        let buffer = make_buffer("{hello}");
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
        assert_eq!(pos, Some(Position::new(0, 6)));
    }

    // === Coverage: calculate_with_desired_col ===

    #[test]
    fn test_calculate_with_desired_col_line_motion() {
        let buffer = make_buffer("hello\nworld");
        let cursor = make_cursor(0, 3);

        let (pos, desired) = MotionEngine::calculate_with_desired_col(
            &buffer,
            &cursor,
            Motion::Line(Direction::Forward),
            1,
        );
        assert_eq!(pos, Some(Position::new(1, 3)));
        assert_eq!(desired, Some(3));
    }

    #[test]
    fn test_calculate_with_desired_col_char_motion() {
        let buffer = make_buffer("hello");
        let cursor = make_cursor(0, 0);

        let (pos, desired) = MotionEngine::calculate_with_desired_col(
            &buffer,
            &cursor,
            Motion::Char(Direction::Forward),
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 1)));
        assert_eq!(desired, None); // Horizontal motion clears desired col
    }

    // === Coverage: word_forward empty buffer ===

    #[test]
    fn test_word_forward_empty_buffer() {
        let buffer = Buffer::new();
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Forward,
                boundary: WordBoundary::Word,
                end: false,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 0)));
    }

    // === Coverage: word_end at last line last col ===

    #[test]
    fn test_word_end_at_last_position() {
        let buffer = make_buffer("hello");
        let cursor = make_cursor(0, 4);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Forward,
                boundary: WordBoundary::Word,
                end: true,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 4)));
    }

    // === Coverage: paragraph backward to line 0 with empty line at start ===

    #[test]
    fn test_paragraph_backward_starting_empty() {
        let buffer = make_buffer("\npara1\n\npara2");
        let cursor = make_cursor(3, 0);

        let pos =
            MotionEngine::calculate(&buffer, &cursor, Motion::Paragraph(Direction::Backward), 1);
        assert!(pos.is_some());
        assert!(pos.unwrap().line <= 1);
    }

    // === Coverage: backward bracket search spanning multiple lines ===

    #[test]
    fn test_backward_bracket_multiline_nested() {
        let buffer = make_buffer("(\n  inner\n  (\n    deep\n  )\n)");
        let cursor = make_cursor(5, 0); // on the closing ')'

        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
        assert_eq!(pos, Some(Position::new(0, 0)));
    }

    // === Coverage: word_forward pos.line >= line_count branch ===

    // === Coverage: word_end_backward whitespace at start of line ===

    #[test]
    fn test_word_end_backward_whitespace_at_line_start() {
        let buffer = make_buffer("  hello\nworld");
        let cursor = make_cursor(1, 0);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Backward,
                boundary: WordBoundary::Word,
                end: true,
            },
            1,
        );
        // Should go to 'o' of hello (line 0, col 6)
        assert_eq!(pos, Some(Position::new(0, 6)));
    }

    // === Coverage: paragraph backward at line 0 with empty first line ===

    #[test]
    fn test_paragraph_backward_line0_empty() {
        let buffer = make_buffer("\n\npara1");
        let cursor = make_cursor(2, 0);

        let pos =
            MotionEngine::calculate(&buffer, &cursor, Motion::Paragraph(Direction::Backward), 1);
        assert!(pos.is_some());
        // Paragraph backward from para1 skips empty lines 0,1 -> reaches line 0
        assert_eq!(pos.unwrap().line, 0);
    }

    // === Coverage: match_bracket forward search finding closing bracket ===

    #[test]
    fn test_match_bracket_forward_search_finds_close() {
        let buffer = make_buffer("abc ) xyz");
        let cursor = make_cursor(0, 0); // Not on a bracket

        let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
        // Forward search finds ')' at col 4, which is a closing bracket
        // find_backward_bracket should fail (no matching '('), returns None
        assert!(pos.is_none());
    }

    // === Coverage: word_end multiline at end of last line ===

    #[test]
    fn test_word_end_at_end_of_last_line() {
        let buffer = make_buffer("hello world");
        let cursor = make_cursor(0, 10); // at 'd' of "world" (last char)

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Forward,
                boundary: WordBoundary::Word,
                end: true,
            },
            1,
        );
        // At last position of last line, stays at end
        assert_eq!(pos, Some(Position::new(0, 10)));
    }

    // === Coverage: word_backward from whitespace-only start of line ===

    #[test]
    fn test_word_backward_whitespace_start() {
        let buffer = make_buffer("   hello");
        let cursor = make_cursor(0, 1); // on whitespace

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Backward,
                boundary: WordBoundary::Word,
                end: false,
            },
            1,
        );
        // At whitespace start, should go to col 0
        assert_eq!(pos, Some(Position::new(0, 0)));
    }

    // === Coverage: paragraph forward past end of buffer ===

    #[test]
    fn test_paragraph_forward_past_end() {
        let buffer = make_buffer("hello");
        let cursor = make_cursor(0, 0);

        let pos =
            MotionEngine::calculate(&buffer, &cursor, Motion::Paragraph(Direction::Forward), 1);
        // Single line, no empty line, forward should stop at end
        assert!(pos.is_some());
    }

    // === Coverage: paragraph backward at line 0 non-empty ===

    #[test]
    fn test_paragraph_backward_at_line0() {
        let buffer = make_buffer("hello\nworld");
        let cursor = make_cursor(0, 0);

        let pos =
            MotionEngine::calculate(&buffer, &cursor, Motion::Paragraph(Direction::Backward), 1);
        // Already at line 0 with non-empty content, should stay
        assert!(pos.is_some());
        assert_eq!(pos.unwrap().line, 0);
    }

    // === Coverage: word_end_backward at line 0 col 0 with whitespace ===

    #[test]
    fn test_word_end_backward_at_origin() {
        let buffer = make_buffer("  hello");
        let cursor = make_cursor(0, 1); // on whitespace

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Backward,
                boundary: WordBoundary::Word,
                end: true,
            },
            1,
        );
        // Back up one -> col 0 which is whitespace
        // chars.get(0) is whitespace -> stays on whitespace, col = 0
        assert!(pos.is_some());
        assert_eq!(pos.unwrap().column, 0);
    }

    // === Coverage: word_forward_empty_line between words ===

    #[test]
    fn test_word_forward_through_empty_line() {
        let buffer = make_buffer("hello\n\nworld");
        let cursor = make_cursor(0, 4);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Forward,
                boundary: WordBoundary::Word,
                end: false,
            },
            1,
        );
        // Should skip empty line and reach "world"
        assert!(pos.is_some());
    }

    // === Coverage: calculate_with_desired_col count 0 ===

    #[test]
    fn test_calculate_with_desired_col_count_zero() {
        let buffer = make_buffer("hello\nworld");
        let cursor = make_cursor(0, 0);

        let (pos, desired) = MotionEngine::calculate_with_desired_col(
            &buffer,
            &cursor,
            Motion::Line(Direction::Forward),
            0,
        );
        // count 0 should be clamped to 1
        assert_eq!(pos, Some(Position::new(1, 0)));
        assert_eq!(desired, Some(0));
    }

    // === Coverage: word_forward multiline BigWord ===

    #[test]
    fn test_word_forward_multiline_bigword() {
        let buffer = make_buffer("hello.world\n  foo");
        let cursor = make_cursor(0, 0);

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Forward,
                boundary: WordBoundary::BigWord,
                end: false,
            },
            1,
        );
        // BigWord "hello.world" -> skip to next line -> "foo"
        assert_eq!(pos, Some(Position::new(1, 2)));
    }

    // === Coverage: word_backward from col 0 wrapping to previous line ===

    #[test]
    fn test_word_backward_wrap_previous_line() {
        let buffer = make_buffer("hello world\nfoo");
        let cursor = make_cursor(1, 0); // at start of "foo"

        let pos = MotionEngine::calculate(
            &buffer,
            &cursor,
            Motion::Word {
                direction: Direction::Backward,
                boundary: WordBoundary::Word,
                end: false,
            },
            1,
        );
        assert_eq!(pos, Some(Position::new(0, 6))); // "world"
    }
}
