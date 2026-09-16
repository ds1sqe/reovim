//! One text and a cursor, keys in, a frame out. No rd and no wire.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use reovim_client::Key;

/// What a key did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Effect {
    Nothing,
    /// The cursor moved; the text did not.
    Moved,
    /// The whole text is to be written.
    Changed,
    Save,
    Quit,
}

/// What to draw: `rows - 1` text rows, the cursor as (row, column) on
/// them, and the status row's editor part, `line:col`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Frame {
    pub lines: Vec<String>,
    pub cursor: (u16, u16),
    pub status: String,
}

/// The text, the cursor as a byte index on a char boundary, and the
/// first line shown.
#[derive(Debug, Default)]
pub struct Editor {
    text: String,
    cursor: usize,
    top: usize,
}

impl Editor {
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// `bytes` as the text, lossily; the cursor stays where it can.
    /// True when the text differs from before.
    pub fn loaded(&mut self, bytes: &[u8]) -> bool {
        let text = String::from_utf8_lossy(bytes);
        if text == self.text {
            return false;
        }
        self.text = text.into_owned();
        self.cursor = self.cursor.min(self.text.len());
        while !self.text.is_char_boundary(self.cursor) {
            self.cursor -= 1;
        }
        true
    }

    /// One key.
    pub fn key(&mut self, key: Key) -> Effect {
        match key {
            Key::Char(c) => self.insert(c),
            Key::Enter => self.insert('\n'),
            Key::Backspace => self.backspace(),
            Key::Left => self.left(),
            Key::Right => self.right(),
            Key::Up => self.vertical(false),
            Key::Down => self.vertical(true),
            Key::Home => self.go(self.line_start(self.position().0)),
            Key::End => self.go(self.line_end(self.cursor)),
            Key::Ctrl('s') => Effect::Save,
            Key::Ctrl('q') | Key::Esc => Effect::Quit,
            Key::Tab | Key::Ctrl(_) => Effect::Nothing,
        }
    }

    /// The cursor's line and column, in chars from 0.
    #[must_use]
    pub fn position(&self) -> (usize, usize) {
        let before = &self.text[..self.cursor];
        let line = before.matches('\n').count();
        let column = before.rsplit('\n').next().unwrap_or("").chars().count();
        (line, column)
    }

    /// The frame for `cols` by `rows`; `top` moves to keep the cursor
    /// on screen.
    pub fn frame(&mut self, cols: u16, rows: u16) -> Frame {
        let width = usize::from(cols.max(1));
        let height = usize::from(rows.saturating_sub(1).max(1));
        let (line, column) = self.position();
        if line < self.top {
            self.top = line;
        } else if line >= self.top + height {
            self.top = line + 1 - height;
        }
        let lines = self
            .text
            .split('\n')
            .skip(self.top)
            .take(height)
            .map(|l| l.chars().take(width).collect())
            .collect();
        let row = u16::try_from(line - self.top).unwrap_or(u16::MAX);
        let col = u16::try_from(column.min(width - 1)).unwrap_or(u16::MAX);
        let status = format!("{}:{}", line + 1, column + 1);
        Frame { lines, cursor: (row, col), status }
    }

    fn insert(&mut self, c: char) -> Effect {
        self.text.insert(self.cursor, c);
        self.cursor += c.len_utf8();
        Effect::Changed
    }

    fn backspace(&mut self) -> Effect {
        let Some(c) = self.text[..self.cursor].chars().next_back() else {
            return Effect::Nothing;
        };
        self.cursor -= c.len_utf8();
        self.text.remove(self.cursor);
        Effect::Changed
    }

    fn left(&mut self) -> Effect {
        match self.text[..self.cursor].chars().next_back() {
            Some(c) => self.go(self.cursor - c.len_utf8()),
            None => Effect::Nothing,
        }
    }

    fn right(&mut self) -> Effect {
        match self.text[self.cursor..].chars().next() {
            Some(c) => self.go(self.cursor + c.len_utf8()),
            None => Effect::Nothing,
        }
    }

    /// The line above or below, at the same column or the line's end.
    fn vertical(&mut self, down: bool) -> Effect {
        let (line, column) = self.position();
        let target = if down { line + 1 } else { line.wrapping_sub(1) };
        let Some(start) = self.line_at(target) else { return Effect::Nothing };
        let end = self.line_end(start);
        let offset: usize = self.text[start..end]
            .chars()
            .take(column)
            .map(char::len_utf8)
            .sum();
        self.go(start + offset)
    }

    /// Byte index where line `n` starts, if there is one.
    fn line_at(&self, n: usize) -> Option<usize> {
        let mut at = 0;
        for _ in 0..n {
            at += self.text[at..].find('\n')? + 1;
        }
        Some(at)
    }

    fn line_start(&self, n: usize) -> usize {
        self.line_at(n).unwrap_or(0)
    }

    /// Byte index of the newline ending the line `at` is on, or the end.
    fn line_end(&self, at: usize) -> usize {
        self.text[at..].find('\n').map_or(self.text.len(), |i| at + i)
    }

    const fn go(&mut self, to: usize) -> Effect {
        if to == self.cursor {
            return Effect::Nothing;
        }
        self.cursor = to;
        Effect::Moved
    }
}
