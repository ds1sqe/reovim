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

    #[cfg_attr(coverage_nightly, coverage(off))]
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

    #[cfg_attr(coverage_nightly, coverage(off))]
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

    #[cfg_attr(coverage_nightly, coverage(off))]
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
        boundary: WordBoundary,
    ) -> Option<Position> {
        // ge/gE: Move backward to end of previous word/WORD.
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

            // Phase 1: Skip whitespace backward
            while x > 0 && chars.get(x).is_some_and(|c| c.is_whitespace()) {
                x -= 1;
            }

            if chars.get(x).is_some_and(|c| c.is_whitespace()) {
                if pos.line > 0 {
                    pos.line -= 1;
                    pos.column = buffer.line_len(pos.line).unwrap_or(0).saturating_sub(1);
                    continue;
                }
                pos.column = 0;
                break;
            }

            // Phase 2: Check if x is at a word boundary (end of a word/WORD).
            // If the next char is whitespace, different word class, or EOL, we're
            // already at a word end — return immediately.
            let at_word_end = if x + 1 >= chars.len() {
                true
            } else {
                let next = chars[x + 1];
                if next.is_whitespace() {
                    true
                } else if boundary == WordBoundary::Word {
                    let x_is_word = chars[x].is_alphanumeric() || chars[x] == '_';
                    let next_is_word = next.is_alphanumeric() || next == '_';
                    x_is_word != next_is_word
                } else {
                    false // BigWord: only whitespace ends a WORD
                }
            };

            if at_word_end {
                pos.column = x;
                break;
            }

            // Phase 3: Inside a word/WORD — skip backward through current word class
            // to find the start, then find end of previous word.
            if boundary == WordBoundary::Word {
                let is_word = chars[x].is_alphanumeric() || chars[x] == '_';
                if is_word {
                    while x > 0 && (chars[x - 1].is_alphanumeric() || chars[x - 1] == '_') {
                        x -= 1;
                    }
                } else {
                    while x > 0
                        && !chars[x - 1].is_whitespace()
                        && !(chars[x - 1].is_alphanumeric() || chars[x - 1] == '_')
                    {
                        x -= 1;
                    }
                }
            } else {
                while x > 0 && !chars[x - 1].is_whitespace() {
                    x -= 1;
                }
            }

            // x is at start of current word — go one back to find previous word end
            if x == 0 {
                if pos.line > 0 {
                    pos.line -= 1;
                    pos.column = buffer.line_len(pos.line).unwrap_or(0).saturating_sub(1);
                    continue;
                }
                pos.column = 0;
                break;
            }

            x -= 1;

            // Skip whitespace backward to find end of previous word
            while x > 0 && chars[x].is_whitespace() {
                x -= 1;
            }

            if chars[x].is_whitespace() {
                if pos.line > 0 {
                    pos.line -= 1;
                    pos.column = buffer.line_len(pos.line).unwrap_or(0).saturating_sub(1);
                    continue;
                }
                pos.column = 0;
                break;
            }

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

    #[cfg_attr(coverage_nightly, coverage(off))]
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

    #[cfg_attr(coverage_nightly, coverage(off))]
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

    #[cfg_attr(coverage_nightly, coverage(off))]
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

// =========================================================================
// #720 repro: word_end_backward ignores WordBoundary parameter.
// ge and gE behave identically because _boundary is unused.
// =========================================================================

#[cfg(test)]
mod b9_repro {
    use {super::*, crate::mm::Cursor};

    // "foo::bar baz" — '::' is punctuation, separating two words but one WORD.
    // ge from 'b' in "baz" (col 9): should stop at 'r' in "bar" (col 7)
    // gE from 'b' in "baz" (col 9): should also stop at 'r' (col 7) — end of WORD "foo::bar"
    //
    // Second ge from col 7: should stop at ':' (col 4) — end of punctuation word "::"
    // Second gE from col 7: should stop at 'o' (col 2) — end of WORD "foo" is col 2?
    // Actually gE skips the entire "foo::bar" as one WORD, so from inside it goes
    // to previous WORD boundary. Let's use a clearer example.

    #[test]
    fn b9_ge_and_ge_identical_on_punctuation() {
        // "hello.world test"
        //  0123456789...
        // ge from 't' in "test" (col 12): backward word-end lands on 'd' in "world" (col 10)
        // gE from 't' in "test" (col 12): backward WORD-end should also land on 'd' (col 10)
        //   because "hello.world" is one WORD, its end is 'd' at col 10.
        //
        // Now from col 10:
        // ge: backward word-end should land on '.' (col 5) — end of punctuation "."
        // gE: backward WORD-end should land at... no previous WORD, so col 0 or stays.
        //   Actually gE treats "hello.world" as one WORD, so going backward from col 10
        //   (which is inside that WORD) would go to before this WORD to the previous one.
        //   There's no previous WORD, so it stays or goes to col 0.

        let buf = Buffer::from_string("hello.world test");
        let cursor = Cursor::new(Position::new(0, 10)); // 'd' in "world"

        let ge = MotionEngine::calculate(
            &buf,
            &cursor,
            Motion::Word {
                direction: Direction::Backward,
                boundary: WordBoundary::Word,
                end: true,
            },
            1,
        );

        let g_big_e = MotionEngine::calculate(
            &buf,
            &cursor,
            Motion::Word {
                direction: Direction::Backward,
                boundary: WordBoundary::BigWord,
                end: true,
            },
            1,
        );

        // ge lands on '.' (col 5), gE skips entire WORD to col 0
        assert_ne!(ge, g_big_e, "ge and gE should return different positions");
        assert_eq!(ge.unwrap().column, 5, "ge: end of punctuation '.'");
        assert_eq!(g_big_e.unwrap().column, 0, "gE: no previous WORD, lands at start");
    }

    #[test]
    fn b9_word_end_forward_does_use_boundary() {
        // Contrast: word_end (forward) correctly uses boundary.
        // "foo.bar" — e lands on 'o' (col 2), E lands on 'r' (col 6)
        let buf = Buffer::from_string("foo.bar");
        let cursor = Cursor::new(Position::new(0, 0));

        let e_word = MotionEngine::calculate(
            &buf,
            &cursor,
            Motion::Word {
                direction: Direction::Forward,
                boundary: WordBoundary::Word,
                end: true,
            },
            1,
        );

        let e_big_word = MotionEngine::calculate(
            &buf,
            &cursor,
            Motion::Word {
                direction: Direction::Forward,
                boundary: WordBoundary::BigWord,
                end: true,
            },
            1,
        );

        // Forward word_end correctly distinguishes Word vs BigWord
        assert_ne!(
            e_word, e_big_word,
            "Forward e vs E: Word stops at 'o' (col 2), BigWord at 'r' (col 6)"
        );
    }

    fn ge(buf: &Buffer, line: usize, col: usize) -> Option<Position> {
        MotionEngine::calculate(
            buf,
            &Cursor::new(Position::new(line, col)),
            Motion::Word {
                direction: Direction::Backward,
                boundary: WordBoundary::Word,
                end: true,
            },
            1,
        )
    }

    fn g_big_e(buf: &Buffer, line: usize, col: usize) -> Option<Position> {
        MotionEngine::calculate(
            buf,
            &Cursor::new(Position::new(line, col)),
            Motion::Word {
                direction: Direction::Backward,
                boundary: WordBoundary::BigWord,
                end: true,
            },
            1,
        )
    }

    #[test]
    fn b9_ge_from_whitespace() {
        // "hello world" col 5 (space) → ge lands on 'o' (col 4)
        let buf = Buffer::from_string("hello world");
        assert_eq!(ge(&buf, 0, 5).unwrap().column, 4);
    }

    #[test]
    fn b9_ge_from_word_boundary() {
        // "hello.world" col 6 ('w') → ge lands on '.' (col 5)
        let buf = Buffer::from_string("hello.world");
        assert_eq!(ge(&buf, 0, 6).unwrap().column, 5);
    }

    #[test]
    fn b9_ge_across_line() {
        // "foo\nbar" line 1 col 0 ('b') → ge lands on line 0 col 2 ('o')
        let buf = Buffer::from_string("foo\nbar");
        let pos = ge(&buf, 1, 0).unwrap();
        assert_eq!(pos.line, 0);
        assert_eq!(pos.column, 2);
    }

    #[test]
    fn b9_ge_underscore_is_word_char() {
        // "foo_bar.baz" col 8 ('a' in baz) → ge lands on '.' (col 7)
        let buf = Buffer::from_string("foo_bar.baz");
        assert_eq!(ge(&buf, 0, 8).unwrap().column, 7);
    }

    #[test]
    fn b9_ge_big_word_skips_entire_word() {
        // "foo.bar baz" col 10 ('z') → gE lands on 'r' (col 6, end of WORD "foo.bar")
        let buf = Buffer::from_string("foo.bar baz");
        assert_eq!(g_big_e(&buf, 0, 10).unwrap().column, 6);
    }

    #[test]
    fn b9_ge_at_buffer_start() {
        // col 0 → stays at col 0
        let buf = Buffer::from_string("hello");
        assert_eq!(ge(&buf, 0, 0).unwrap().column, 0);
    }

    #[test]
    fn b9_ge_from_whitespace_after_line_break() {
        // "first line\n  second" from line 1 col 2 ('s')
        // ge → line 0 col 9 ('e' in "line")
        let buf = Buffer::from_string("first line\n  second");
        let pos = ge(&buf, 1, 2).unwrap();
        assert_eq!(pos.line, 0);
        assert_eq!(pos.column, 9);
    }

    // === Coverage: lines 504-509 — backward scan over punctuation chars ===

    #[test]
    fn ge_backward_through_punctuation_run() {
        // "abc::def" — cursor on 'd' (col 5), which is a word char.
        // Phase 2 check: chars[5]='d' is word, chars[6]='e' is word, same class → not word end.
        // Phase 3: x is word char, skip backward through word chars: d → stop at x=5
        //   (chars[4]=':' is not word). x=5 is start of "def" word.
        // x==5 > 0, so x -= 1 → x=4. Skip whitespace? ':' is not ws. chars[4]=':'
        // pos.column = 4 → that's the ':' punctuation.
        //
        // Now call ge again from col 4 (':'):
        // After initial move back: x=3, chars[3]=':' — punctuation.
        // Phase 2: chars[3]=':', chars[4]='d' word → different class → at word end.
        // So it returns col 3. But we want to test the *punctuation skip loop*.
        //
        // Use "abc:::def" cursor on 'd' (col 6).
        // Phase 2: chars[6]='d', chars[7]='e' same → not word end.
        // Phase 3: word char, skip back to x=6 (start of "def"), x -= 1 → x=5 (':').
        // chars[5]=':' not ws → pos.column = 5.
        //
        // Now ge from col 5 (':'):
        // Move back → x=4. chars[4]=':'.
        // Phase 1: not ws, skip.
        // Phase 2: chars[4]=':', chars[5]=':' — same class (both punct) → not word end.
        // Phase 3 (lines 504-509): not word char → skip backward through punctuation:
        //   x=4, chars[3]=':' punct → x=3; chars[2]='c' is word → stop. x=3.
        // x==3 > 0, x -= 1 → x=2. chars[2]='c' not ws. pos.column = 2.
        let buf = Buffer::from_string("abc:::def");
        // ge from ':' at col 5 (after backing to col 4)
        let pos = ge(&buf, 0, 5).unwrap();
        assert_eq!(pos.column, 2, "ge from middle of punctuation run should land on end of 'abc'");
    }

    // === Coverage: lines 520-522 — cursor at word start, wraps to previous line ===

    #[test]
    fn ge_word_start_wraps_to_previous_line() {
        // "hello\nworld" — ge from 'w' (line 1, col 0).
        // Move back → line 0, col 4 ('o').
        // That is at_word_end (x+1 >= chars.len() since col 4 is last of "hello").
        // Returns (0, 4). But that doesn't reach lines 520-522.
        //
        // We need: cursor lands inside a word at col 0 after phase 3 skip.
        // "abc\ndef" from 'd' (line 1, col 0).
        // Move back → line 0, col 2 ('c').
        // Phase 2: x=2, x+1=3 >= len(3) → at_word_end → returns col 2.
        //
        // To reach 520-522 we need x==0 after phase 3, with pos.line > 0.
        // "a\ndef" from 'e' (line 1, col 1).
        // Move back: col 1 > 0 → col 0. x = 0, chars = ['d','e','f'].
        // Phase 1: chars[0]='d' not ws → skip.
        // Phase 2: chars[0]='d', chars[1]='e' → same class → not word end.
        // Phase 3: word char, skip back: x=0, loop (x>0?) no.
        // x == 0, pos.line == 1 > 0 → lines 520-522: wrap to line 0.
        // Line 0 = "a", pos.column = 0. Next iteration: chars=['a'], x=0.
        // Phase 1: 'a' not ws. Phase 2: x+1 >= len → at_word_end → returns (0,0).
        let buf = Buffer::from_string("a\ndef");
        let pos = ge(&buf, 1, 1).unwrap();
        assert_eq!(pos.line, 0, "should wrap to previous line");
        assert_eq!(pos.column, 0, "should land on 'a'");
    }

    // === Coverage: lines 536-542 — all whitespace on left, wraps to previous line ===

    #[test]
    fn ge_all_whitespace_left_wraps_to_previous_line() {
        // Need: after phase 3 skip, x > 0, x -= 1, then skip whitespace backward
        // until chars[x].is_whitespace() is true (all whitespace on left).
        // Then lines 536-542 fire.
        //
        // "hello\n  a" from 'a' (line 1, col 2).
        // Move back: col 2 > 0 → col 1. chars = [' ',' ','a'].
        // Phase 1: chars[1]=' ' ws, skip → x=0. chars[0]=' ' ws.
        // chars[x].is_whitespace() → true, pos.line==1>0 → wrap to line 0.
        //
        // But that's lines 462-466, not 536-542. Lines 536-542 are *after* phase 3.
        // We need to reach phase 3 (not at word end), skip back to start, x>0,
        // x-=1, then skip whitespace, but land on whitespace.
        //
        // "abc  \n  def" from 'e' (line 1, col 3).
        // Move back: col 3>0 → col 2. chars = [' ',' ','d','e','f']. x=2.
        // Phase 1: chars[2]='d' not ws → skip.
        // Phase 2: chars[2]='d', chars[3]='e' → same class → not word end.
        // Phase 3: 'd' is word, skip back: x=2, chars[1]=' ' not word → stop. x=2.
        // x==2 > 0, x -= 1 → x=1. chars[1]=' ' is ws.
        // Skip ws backward: x=1>0, chars[1]=' ' ws → x=0. chars[0]=' ' ws.
        // chars[x].is_whitespace() → true! pos.line==1>0 → lines 536-542: wrap.
        // Line 0 = "abc  ", col = 4. Next iter: chars = ['a','b','c',' ',' '], x=4.
        // Phase 1: chars[4]=' ' ws → x=3, chars[3]=' ' ws → x=2, chars[2]='c' not ws.
        // Phase 2: chars[2]='c', chars[3]=' ' → different → at_word_end → returns (0,2).
        let buf = Buffer::from_string("abc  \n  def");
        let pos = ge(&buf, 1, 3).unwrap();
        assert_eq!(pos.line, 0, "should wrap to previous line through whitespace");
        assert_eq!(pos.column, 2, "should land on 'c' in 'abc'");
    }

    // === Coverage: lines 540-542 — all whitespace left on line 0 ===

    #[test]
    fn ge_all_whitespace_left_on_line_zero() {
        // Need: after phase 3, x > 0, x -= 1, skip whitespace, land on whitespace,
        // but pos.line == 0 → lines 540-542: pos.column = 0; break.
        //
        // " def" from 'e' (col 2).
        // Move back: col 2 > 0 → col 1. chars = [' ','d','e','f']. x = 1.
        // Phase 1: chars[1]='d' not ws → skip.
        // Phase 2: chars[1]='d', chars[2]='e' same class → not at word end.
        // Phase 3: 'd' is word, skip backward: x=1, chars[0]=' ' not word → stop. x=1.
        // x == 1 > 0, x -= 1 → x=0. chars[0]=' ' is ws.
        // Skip ws backward: x > 0? No (x=0). Loop skipped.
        // chars[x].is_whitespace() → true, pos.line==0 → lines 540-542.
        let buf = Buffer::from_string(" def");
        let pos = ge(&buf, 0, 2).unwrap();
        assert_eq!(pos.line, 0);
        assert_eq!(pos.column, 0, "should land at col 0 when all whitespace left on line 0");
    }

    // === Coverage: branch 482:2 — BigWord path in phase 2 ===

    #[test]
    fn ge_big_word_not_at_word_end_adjacent_non_ws() {
        // BigWord phase 2: next char is not whitespace → returns false (line 486).
        // "abc.def" from 'e' (col 5). After backing to col 4.
        // Phase 2: chars[4]='d', chars[5]='e' — both non-ws.
        // BigWord: only whitespace ends a WORD → false → phase 3.
        let buf = Buffer::from_string("abc.def");
        let pos = g_big_e(&buf, 0, 5).unwrap();
        // Phase 3 (BigWord): skip back while non-ws: x=4,3,2,1,0. x==0.
        // x==0, pos.line==0 → pos.column=0; break.
        assert_eq!(pos.column, 0, "gE should skip entire WORD to start");
    }

    // === Coverage: branch 483:2 — Word same class (not at word end) ===

    #[test]
    fn ge_word_same_class_not_at_word_end() {
        // Word boundary, phase 2: chars[x] and chars[x+1] are same word class → not at end.
        // "abcdef" from 'e' (col 4). After backing to col 3.
        // Phase 2: chars[3]='d', chars[4]='e' — both word chars, same class → not word end.
        // Phase 3: skip backward through word chars: x=3,2,1,0. x==0.
        // pos.line==0 → pos.column=0; break.
        let buf = Buffer::from_string("abcdef");
        let pos = ge(&buf, 0, 4).unwrap();
        assert_eq!(pos.column, 0, "ge in middle of word with no previous word → col 0");
    }

    // === Coverage: branch 498:2 — BigWord in phase 3 ===

    #[test]
    fn ge_big_word_phase3_skip_to_start() {
        // BigWord phase 3: skip backward through all non-whitespace.
        // "   abc.def" from '.' (col 6). After backing to col 5.
        // Phase 1: chars[5]='c' not ws → skip.
        // Phase 2: chars[5]='c', chars[6]='.' — BigWord: not ws → false → phase 3.
        // Phase 3 BigWord: skip back while non-ws: x=5,4,3. chars[2]=' ' → stop. x=3.
        // x==3 > 0, x -= 1 → x=2. chars[2]=' ' is ws.
        // Skip ws: x=2>0, chars[2]=' '→x=1, chars[1]=' '→x=0, chars[0]=' '.
        // chars[x].is_whitespace() → true, line 0 → pos.column=0; break.
        let buf = Buffer::from_string("   abc.def");
        let pos = g_big_e(&buf, 0, 6).unwrap();
        assert_eq!(pos.column, 0);
    }

    // === Coverage: branch 500:4 — word skip loop exits immediately ===

    #[test]
    fn ge_word_char_at_x_but_prev_is_punct() {
        // Phase 3, Word boundary: chars[x] is word char but chars[x-1] is not word.
        // Loop exits immediately (branch 500:4 = false on first check).
        // "!a.bc" from 'b' (col 3). After backing to col 2.
        // Phase 1: chars[2]='.' not ws → skip.
        // Phase 2: chars[2]='.', chars[3]='b' → different class → at_word_end → returns col 2.
        // That hits the at_word_end path, not phase 3.
        //
        // Try: ".ab" from 'b' (col 2). After backing to col 1.
        // Phase 1: chars[1]='a' not ws. Phase 2: chars[1]='a', chars[2]='b' → same → not end.
        // Phase 3: 'a' is word. while x>0 && chars[x-1] is word: chars[0]='.' → false. x stays 1.
        // x==1>0, x-=1→x=0. chars[0]='.' not ws. pos.column=0. break.
        let buf = Buffer::from_string(".ab");
        let pos = ge(&buf, 0, 2).unwrap();
        assert_eq!(pos.column, 0, "ge should land on '.' (end of punct run at col 0)");
    }

    // === Coverage: branch 504:1, 505:1, 506:2 — punctuation skip in phase 3 ===

    #[test]
    fn ge_punct_skip_single_char() {
        // Phase 3 with punctuation char where skip only moves one position.
        // "a.!b" from '!' (col 2). After backing to col 1.
        // Phase 1: chars[1]='.' not ws → skip.
        // Phase 2: chars[1]='.', chars[2]='!' — both punct → same class → not word end.
        // Phase 3: '.' is not word char → punct skip:
        //   x=1, chars[0]='a' → is word → stop. x stays 1.
        // x==1>0, x-=1→0. chars[0]='a' not ws → pos.column=0. break.
        let buf = Buffer::from_string("a.!b");
        let pos = ge(&buf, 0, 2).unwrap();
        assert_eq!(pos.column, 0, "ge from inside punct run with word char to left");
    }

    #[test]
    fn ge_punct_skip_multiple_chars() {
        // Ensure the punctuation skip loop iterates more than once.
        // "a...b" from last '.' (col 3). After backing to col 2.
        // Phase 1: chars[2]='.' not ws. Phase 2: '.', '.' same → not word end.
        // Phase 3 punct skip: x=2, chars[1]='.' not ws, not word → x=1.
        //   chars[0]='a' is word → stop. x=1.
        // x==1>0, x-=1→0. chars[0]='a' not ws → pos.column=0.
        let buf = Buffer::from_string("a...b");
        let pos = ge(&buf, 0, 3).unwrap();
        assert_eq!(pos.column, 0);
    }

    // === Coverage: branch 536:1 — whitespace at x, line > 0, wraps ===
    // Already covered by ge_all_whitespace_left_wraps_to_previous_line above.
    // Adding a BigWord variant for branch coverage of the BigWord path in phase 3.

    #[test]
    fn ge_big_word_whitespace_left_wraps() {
        // BigWord variant: after phase 3, all whitespace left, wraps to prev line.
        // "abc\n  def" from 'e' (line 1, col 3). After backing to col 2.
        // chars = [' ',' ','d','e','f']. x=2.
        // Phase 1: chars[2]='d' not ws. Phase 2: BigWord, chars[3]='e' not ws → false.
        // Phase 3 BigWord: skip back: x=2, chars[1]=' ' ws → stop. x=2.
        // x==2>0, x-=1→1. chars[1]=' ' ws. Skip ws: x=1>0, chars[1]=' '→x=0.
        // chars[0]=' ' ws. pos.line==1>0 → wrap to line 0.
        let buf = Buffer::from_string("abc\n  def");
        let pos = g_big_e(&buf, 1, 3).unwrap();
        assert_eq!(pos.line, 0, "gE should wrap to previous line");
        assert_eq!(pos.column, 2, "gE should land on 'c'");
    }

    // === Coverage: empty buffer ===

    #[test]
    fn ge_empty_buffer() {
        let buf = Buffer::from_string("");
        let pos = ge(&buf, 0, 0).unwrap();
        assert_eq!(pos.line, 0);
        assert_eq!(pos.column, 0);
    }

    // === Coverage: empty line wraps backward ===

    #[test]
    fn ge_empty_line_wraps() {
        // "hello\n\nworld" from empty line 1 col 0.
        // Move back: col 0, line 1 > 0 → line 0, col 4.
        // Phase 2: chars[4]='o', x+1>=5=len → at_word_end → return (0,4).
        let buf = Buffer::from_string("hello\n\nworld");
        let pos = ge(&buf, 1, 0).unwrap();
        assert_eq!(pos.line, 0);
        assert_eq!(pos.column, 4, "ge from empty line should land on 'o' in 'hello'");
    }

    // === Coverage: empty line at line 0 (break from empty line check) ===

    #[test]
    fn ge_empty_line_at_start() {
        // "\nhello" from line 0 col 0. Line 0 is empty.
        // Move back: col 0, line 0 → can't move back further.
        // Loop: line 0 = "", chars empty. pos.line == 0 → break. Returns (0,0).
        let buf = Buffer::from_string("\nhello");
        let pos = ge(&buf, 0, 0).unwrap();
        assert_eq!(pos.line, 0);
        assert_eq!(pos.column, 0);
    }

    // === Coverage: L482:br2, L483:br2 — underscore as word char ===

    #[test]
    fn ge_word_boundary_with_underscores() {
        // Exercise `chars[x] == '_'` (L482:br2) and `next == '_'` (L483:br2).
        // "a__b" from 'b' (col 3). After backing to col 2.
        // Phase 1: chars[2]='_' not ws → skip.
        // Phase 2: chars[2]='_', chars[3]='b'. Word boundary check:
        //   x_is_word: '_' is not alphanumeric, but '_' == '_' → true (L482:br2)
        //   next_is_word: 'b'.is_alphanumeric() → true
        //   x_is_word != next_is_word → false → not at word end.
        // Phase 3: is_word('_') → true. Skip back: chars[1]='_'=='_' → true (L500 br4).
        //   x=2→1. chars[0]='a' is alphanumeric → true. x=1→0. Loop exits.
        // x==0, line 0 → pos.column=0.
        let buf = Buffer::from_string("a__b");
        let pos = ge(&buf, 0, 3).unwrap();
        assert_eq!(pos.column, 0, "ge should treat underscores as word chars");
    }

    #[test]
    fn ge_word_boundary_underscore_next_is_underscore() {
        // Exercise `next == '_'` specifically (L483:br2).
        // "ab_c" from 'b' (col 1). After backing to col 0.
        // Phase 1: chars[0]='a' not ws.
        // Phase 2: chars[0]='a', chars[1]='b'. Both word → same class → not word end.
        // Phase 3: 'a' is word. Skip: x=0, loop exits (x>0 false).
        // x==0, line 0 → col 0.
        //
        // Better: "ab__cd" from 'd' (col 5). After backing to col 4.
        // Phase 1: chars[4]='c' not ws.
        // Phase 2: chars[4]='c', chars[5]='d'. Both word (alphanumeric) → same → not end.
        // Phase 3: 'c' is word. Skip: chars[3]='_'=='_'→true. x=4→3.
        //   chars[2]='_'=='_'→true. x=3→2.
        //   chars[1]='b'.is_alphanumeric()→true. x=2→1.
        //   chars[0]='a'.is_alphanumeric()→true. x=1→0. Loop exits.
        // x==0, line 0 → col 0.
        let buf = Buffer::from_string("ab__cd");
        let pos = ge(&buf, 0, 5).unwrap();
        assert_eq!(pos.column, 0, "ge skips backward through underscores");
    }

    // === Coverage: L498:br2 — BigWord in phase 3 hitting specific branches ===

    #[test]
    fn ge_big_word_phase3_from_deep_inside_word() {
        // "foo.bar_baz test" from 'a' in "baz" (col 9). After backing to col 8.
        // Phase 1: chars[8]='a' not ws.
        // Phase 2: BigWord, chars[9]='z' not ws → false → phase 3.
        // Phase 3 BigWord (L498:br2 false): skip non-ws backward:
        //   x=8,7('_'),6('r'),5('a'),4('b'),3('.'),2('o'),1('o'),0('f').
        //   x=0, loop exits.
        // x==0, line 0 → col 0.
        let buf = Buffer::from_string("foo.bar_baz test");
        let pos = g_big_e(&buf, 0, 9).unwrap();
        assert_eq!(pos.column, 0, "gE skips entire WORD backward to col 0");
    }

    // === Coverage: L504:br1, L505:br1, L506:br2 — punct skip with mixed chars ===

    #[test]
    fn ge_punct_skip_stops_at_whitespace() {
        // Exercise the punctuation skip loop where chars[x-1] is whitespace.
        // "a .!b" from '!' (col 3). After backing to col 2.
        // Phase 1: chars[2]='.' not ws.
        // Phase 2: chars[2]='.', chars[3]='!' both punct → same → not end.
        // Phase 3: '.' not word → punct skip (L504-509):
        //   x=2, chars[1]=' ' is whitespace → loop condition false (L505:br1).
        //   Loop exits. x=2.
        // x==2>0, x-=1→1. chars[1]=' ' ws.
        // Skip ws: x=1>0, chars[1]=' '→x=0. chars[0]='a' not ws? No wait,
        // after x-=1 we have x=1. The ws skip is `while x > 0 && chars[x].is_whitespace()`.
        // chars[1]=' ' → x=0. chars[0]='a' not ws → loop exits.
        // chars[0].is_whitespace() → false → pos.column=0.
        let buf = Buffer::from_string("a .!b");
        let pos = ge(&buf, 0, 3).unwrap();
        assert_eq!(pos.column, 0, "ge from punct run with ws to left should reach col 0");
    }

    #[test]
    fn ge_punct_skip_stops_at_word_char() {
        // Exercise the punctuation skip where chars[x-1] is a word char (L506:br2 true).
        // "abc..def" from second '.' (col 4). After backing to col 3.
        // Phase 1: chars[3]='.' not ws.
        // Phase 2: chars[3]='.', chars[4]='.' both punct → same → not end.
        // Phase 3: '.' not word → punct skip:
        //   x=3, chars[2]='c' → is alphanumeric (word) → L506:br2 true → loop exits.
        //   x stays 3.
        // x==3>0, x-=1→2. chars[2]='c' not ws → pos.column=2.
        let buf = Buffer::from_string("abc..def");
        let pos = ge(&buf, 0, 4).unwrap();
        assert_eq!(pos.column, 2, "ge from punct should land on end of word 'abc'");
    }

    // === Coverage: L500:br4 — word skip with underscore in chars[x-1] ===

    #[test]
    fn ge_word_skip_through_underscores() {
        // Phase 3 word skip where chars[x-1] == '_' → the `chars[x-1] == '_'` branch
        // in line 500 evaluates to true (br4).
        // ".a_b" from 'b' (col 3). After backing to col 2.
        // Phase 1: chars[2]='_' not ws.
        // Phase 2: chars[2]='_', chars[3]='b'. x_is_word('_')=true, next_is_word('b')=true.
        //   Same class → not word end.
        // Phase 3: is_word('_')=true. while x>0 && chars[x-1] is word:
        //   x=2, chars[1]='a' alphanumeric → true. x=1.
        //   x=1, chars[0]='.' → not alphanumeric, not '_' → false. Loop exits.
        // x==1>0, x-=1→0. chars[0]='.' not ws → pos.column=0.
        let buf = Buffer::from_string(".a_b");
        let pos = ge(&buf, 0, 3).unwrap();
        assert_eq!(
            pos.column, 0,
            "ge should skip backward through word chars including underscores"
        );
    }
}
