//! Where things are in the text: lines, columns, chars, and the
//! moves that keep the cursor where Vim keeps it.

use reovim_client::width;

use crate::{Effect, Mode, Vim, moved};

impl Vim {
    /// The line and column of `at`, in chars from 0.
    pub(crate) fn position_of(&self, at: usize) -> (usize, usize) {
        let before = &self.text[..at];
        let line = before.matches('\n').count();
        let column = before.rsplit('\n').next().unwrap_or("").chars().count();
        (line, column)
    }

    /// The char at `at`, if the text reaches it.
    pub(crate) fn char_at(&self, at: usize) -> Option<char> {
        self.text[at.min(self.text.len())..].chars().next()
    }

    /// The last byte index the cursor may take: before a final `\n`,
    /// else the end of the text.
    pub(crate) fn bound(&self) -> usize {
        self.text.len() - usize::from(self.text.ends_with('\n'))
    }

    /// How many lines: one more than the `\n`s, except that a final
    /// `\n` ends a line instead of starting one; an empty text is one
    /// empty line.
    pub(crate) fn line_count(&self) -> usize {
        self.text.matches('\n').count() + 1
            - usize::from(self.text.ends_with('\n'))
    }

    /// Byte index where line `n` starts, if there is one.
    pub(crate) fn line_at(&self, n: usize) -> Option<usize> {
        let mut at = 0;
        for _ in 0..n {
            at += self.text[at..].find('\n')? + 1;
        }
        (at <= self.bound()).then_some(at)
    }

    /// Byte index where the line `at` is on starts.
    pub(crate) fn line_start(&self, at: usize) -> usize {
        self.text[..at].rfind('\n').map_or(0, |i| i + 1)
    }

    /// Byte index of the newline ending the line `at` is on, or the
    /// end of the text.
    pub(crate) fn line_end(&self, at: usize) -> usize {
        self.text[at..].find('\n').map_or(self.text.len(), |i| at + i)
    }

    /// The first char of the line `at` is on that is not a blank, or
    /// the line's end.
    pub(crate) fn first_non_blank(&self, at: usize) -> usize {
        let start = self.line_start(at);
        let end = self.line_end(at);
        let blanks = self.text[start..end]
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .map(char::len_utf8)
            .sum::<usize>();
        start + blanks
    }

    /// The blanks a line starts with: what `autoindent` copies.
    pub(crate) fn indent_of(&self, at: usize) -> alloc::string::String {
        let start = self.line_start(at);
        self.text[start..self.first_non_blank(at)].into()
    }

    /// The last char of the line `at` is on, or its start when empty.
    pub(crate) fn last_char(&self, at: usize) -> usize {
        let (start, end) = (self.line_start(at), self.line_end(at));
        if end == start { start } else { self.prev_char(end) }
    }

    /// The byte index of the char before `at`.
    pub(crate) fn prev_char(&self, at: usize) -> usize {
        self.text[..at].chars().next_back().map_or(at, |c| at - c.len_utf8())
    }

    /// `column` chars into the line starting at `start`, or as far as
    /// the line goes.
    pub(crate) fn at_column(&self, start: usize, column: usize) -> usize {
        let end = self.line_end(start);
        let offset: usize = self.text[start..end]
            .chars()
            .take(column)
            .map(char::len_utf8)
            .sum();
        start + offset
    }

    /// The char whose screen cells reach `column` on the line
    /// starting at `start`, tabs eight wide, or as far as the line
    /// goes: where a wanted column lands.
    pub(crate) fn at_screen_column(
        &self,
        start: usize,
        column: usize,
    ) -> usize {
        let end = self.line_end(start);
        let mut cell = 0;
        let reached = |(at, c): (usize, char)| {
            cell = next_cell(cell, c);
            (cell > column).then_some(start + at)
        };
        self.text[start..end].char_indices().find_map(reached).unwrap_or(end)
    }

    /// Screen cells from `start` to `at` on one line.
    pub(crate) fn cells(&self, start: usize, at: usize) -> usize {
        self.text[start..at].chars().fold(0, next_cell)
    }

    /// The screen cell the cursor wants and is shown at: its char's
    /// first cell, but a tab's last in normal mode, as nvim's
    /// `curswant` and cursor have it.
    pub(crate) fn wanted(&self) -> usize {
        let start = self.line_start(self.cursor);
        let cells = self.cells(start, self.cursor);
        let here = self.text[self.cursor..].chars().next();
        match here {
            Some('\t') if self.mode != Mode::Insert => {
                next_cell(cells, '\t') - 1
            }
            _ => cells,
        }
    }

    /// Where `a` inserts: after the cursor's char, unless the line is
    /// empty.
    pub(crate) fn append_at(&self) -> usize {
        let end = self.line_end(self.cursor);
        let next = self.text[self.cursor..end].chars().next();
        next.map_or(self.cursor, |c| self.cursor + c.len_utf8())
    }

    /// Where Esc leaves the cursor: one char back unless at the
    /// line's start, as Vim does.
    pub(crate) fn stepped_back(&self) -> usize {
        if self.cursor == self.line_start(self.cursor) {
            self.cursor
        } else {
            self.prev_char(self.cursor)
        }
    }

    /// Normal mode's cursor: on the last char, not past it.
    pub(crate) fn clamp(&mut self) {
        self.cursor = self.cursor.min(self.bound());
        let end = self.line_end(self.cursor);
        if self.cursor == end {
            self.cursor = self.last_char(self.cursor);
        }
    }

    pub(crate) fn left(&mut self) -> Effect {
        if self.cursor == self.line_start(self.cursor) {
            return Effect::Nothing;
        }
        self.go(self.prev_char(self.cursor))
    }

    /// The next char boundary after `at`, or `at` at the text's end.
    pub(crate) fn forward_char(&self, at: usize) -> usize {
        let c = self.text[at.min(self.text.len())..].chars().next();
        c.map_or(at, |c| at + c.len_utf8())
    }

    /// Right by one char; in normal mode not onto the line's end,
    /// unless an operator is asking.
    pub(crate) fn right(&mut self) -> Effect {
        let past = self.mode == Mode::Insert || self.pending.running;
        let limit = if past {
            self.line_end(self.cursor)
        } else {
            self.last_char(self.cursor)
        };
        match self.text[self.cursor..].chars().next() {
            Some(c) if self.cursor < limit => {
                self.go(self.cursor + c.len_utf8())
            }
            _ => Effect::Nothing,
        }
    }

    /// The line above or below, at the column wanted or as far as the
    /// line goes.
    pub(crate) fn vertical(&mut self, down: bool) -> Effect {
        let line = self.position().0;
        let target = if down { line + 1 } else { line.wrapping_sub(1) };
        self.keep_want = true;
        let Some(start) = self.line_at(target) else { return Effect::Nothing };
        let to = self.at_screen_column(start, self.want);
        match self.mode {
            Mode::Insert => self.go(to),
            Mode::Normal | Mode::Command | Mode::Visual(_) => {
                self.go_clamped(to)
            }
        }
    }

    /// `to`, then normal mode's clamp; `Moved` if the cursor ended
    /// elsewhere than it began.
    pub(crate) fn go_clamped(&mut self, to: usize) -> Effect {
        let from = self.cursor;
        self.cursor = to;
        self.clamp();
        moved(self.cursor != from)
    }

    pub(crate) const fn go(&mut self, to: usize) -> Effect {
        let from = self.cursor;
        self.cursor = to;
        moved(to != from)
    }
}

/// The screen cell after `c` drawn at `cell`: a tab reaches the next
/// stop of eight, a wide char two cells on.
///
/// Vim counts in cells where the eye does: `curswant` is a cell, so
/// `j` from a Hangul syllable lands where it looks like it should.
pub const fn next_cell(cell: usize, c: char) -> usize {
    if c == '\t' { cell / 8 * 8 + 8 } else { cell + width(c) }
}
