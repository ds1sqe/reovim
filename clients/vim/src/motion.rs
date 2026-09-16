//! Where things are in the text: lines, columns, chars, and the
//! moves that keep the cursor where Vim keeps it.

use crate::{Effect, Mode, Vim};

impl Vim {
    /// Byte index where line `n` starts, if there is one.
    pub(crate) fn line_at(&self, n: usize) -> Option<usize> {
        let mut at = 0;
        for _ in 0..n {
            at += self.text[at..].find('\n')? + 1;
        }
        Some(at)
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

    /// The last char of the line `at` is on, or its start when empty.
    pub(crate) fn last_char(&self, at: usize) -> usize {
        let (start, end) = (self.line_start(at), self.line_end(at));
        if end == start { start } else { self.prev_char(end) }
    }

    /// The byte index of the char before `at`.
    pub(crate) fn prev_char(&self, at: usize) -> usize {
        self.text[..at].chars().next_back().map_or(at, |c| at - c.len_utf8())
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

    /// Right by one char; in normal mode not onto the line's end.
    pub(crate) fn right(&mut self) -> Effect {
        let limit = match self.mode {
            Mode::Normal => self.last_char(self.cursor),
            Mode::Insert => self.line_end(self.cursor),
        };
        match self.text[self.cursor..].chars().next() {
            Some(c) if self.cursor < limit => {
                self.go(self.cursor + c.len_utf8())
            }
            _ => Effect::Nothing,
        }
    }

    /// The line above or below, at the same column or as far as the
    /// line goes.
    pub(crate) fn vertical(&mut self, down: bool) -> Effect {
        let (line, column) = self.position();
        let target = if down { line + 1 } else { line.wrapping_sub(1) };
        let Some(start) = self.line_at(target) else { return Effect::Nothing };
        let end = self.line_end(start);
        let offset: usize = self.text[start..end]
            .chars()
            .take(column)
            .map(char::len_utf8)
            .sum();
        let effect = self.go(start + offset);
        if self.mode == Mode::Normal {
            self.clamp();
        }
        effect
    }

    pub(crate) const fn go(&mut self, to: usize) -> Effect {
        if to == self.cursor {
            return Effect::Nothing;
        }
        self.cursor = to;
        Effect::Moved
    }
}
