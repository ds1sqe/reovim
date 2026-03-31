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

        // BUG: both return the same position because _boundary is ignored
        assert_eq!(ge, g_big_e, "#720: ge and gE return identical result — WordBoundary ignored");
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
}
