#![no_std]
#![forbid(unsafe_code)]
//! Vim: keys in, effects and a frame out, over the text and the cursor
//! it holds. It sees no Client, no rd and no terminal; the tui turns
//! an `Effect` into a call and a `Frame` into a screen.
//!
//! The text is the Buffer's bytes. A `\n` ends a line; the last one
//! ends the last line rather than starting an empty one, as Vim has
//! it, and a text without one is a last line without an end.

extern crate alloc;

mod command;
mod motion;
pub mod notation;
mod word;

use alloc::string::String;
use alloc::vec::Vec;

pub use reovim_client::Key;

/// What a key did.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Effect {
    Nothing,
    /// The cursor or the mode moved; the text did not.
    Moved,
    /// `len` bytes at `at` became `text`, already applied here. With
    /// `join` the edit belongs to the head revision, not a new one:
    /// one insert is one undo step.
    Edit {
        at: usize,
        len: usize,
        text: String,
        join: bool,
    },
    Undo,
    Redo,
    Save,
    Quit(Quit),
}

/// How a quit was asked for; what it does with unsaved changes is
/// the tui's to decide, since only it knows whether there are any.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Quit {
    /// `:q`: refused with E37 while there are unsaved changes.
    Ask,
    /// `:q!`, `ZQ`: whatever the Buffer holds, exit 0.
    Force,
    /// `:wq`, `:x`, `ZZ`: written, then exit 0.
    Write,
    /// `:cq`: exit 1, so the caller uses nothing.
    Abort,
}

/// What to draw: `rows - 1` text rows, the cursor as (row, column) on
/// them, and Vim's own word for the status row: a message, the command
/// line, the mode, or nothing.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Frame {
    pub lines: Vec<String>,
    pub cursor: (u16, u16),
    pub status: String,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Mode {
    #[default]
    Normal,
    Insert,
    Command,
}

/// The text, the cursor as a byte index on a char boundary, the first
/// line shown, and the mode. In normal mode the cursor sits on a
/// char, never past the last one of its line; insert may sit past it.
#[derive(Debug, Default)]
pub struct Vim {
    text: String,
    cursor: usize,
    top: usize,
    mode: Mode,
    /// Digits typed before a command; 0 is none.
    count: usize,
    /// A `g`, `d` or `Z` waiting for its second key.
    pending: Option<char>,
    /// The next insert edit joins the head revision.
    joining: bool,
    /// The command line being typed.
    line: String,
    /// Shown until the next key.
    message: String,
}

impl Vim {
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub const fn mode(&self) -> Mode {
        self.mode
    }

    /// `bytes` as the text, lossily; the cursor stays where it can.
    /// True when the text differs from before.
    pub fn loaded(&mut self, bytes: &[u8]) -> bool {
        let text = String::from_utf8_lossy(bytes);
        if text == self.text {
            return false;
        }
        self.text = text.into_owned();
        self.cursor = self.cursor.min(self.bound());
        while !self.text.is_char_boundary(self.cursor) {
            self.cursor -= 1;
        }
        if self.mode == Mode::Normal {
            self.clamp();
        }
        true
    }

    /// One key in the current mode; a message showing goes with it.
    pub fn key(&mut self, key: Key) -> Effect {
        let shown = !self.message.is_empty();
        self.message.clear();
        let effect = match self.mode {
            Mode::Normal => self.normal(key),
            Mode::Insert => self.insert(key),
            Mode::Command => self.command(key),
        };
        if shown && effect == Effect::Nothing { Effect::Moved } else { effect }
    }

    /// The cursor to line `n`, counted from 1 and clamped, at its first
    /// non-blank: `+N` on the command line.
    pub fn goto_line(&mut self, n: usize) {
        let last = self.line_count() - 1;
        let start = self.line_at(n.saturating_sub(1).min(last)).unwrap_or(0);
        self.cursor = self.first_non_blank(start);
        self.clamp();
    }

    /// The `\n` Vim writes at the end of a last line that has none, as
    /// `fixendofline` does; `None` when there is nothing to fix.
    pub fn fix_eol(&mut self) -> Option<Effect> {
        if self.text.is_empty() || self.text.ends_with('\n') {
            return None;
        }
        let at = self.text.len();
        self.text.push('\n');
        Some(Effect::Edit { at, len: 0, text: String::from("\n"), join: false })
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
            .take(self.line_count())
            .skip(self.top)
            .take(height)
            .map(|l| l.chars().take(width).collect())
            .collect();
        let row = u16::try_from(line - self.top).unwrap_or(u16::MAX);
        let col = u16::try_from(column.min(width - 1)).unwrap_or(u16::MAX);
        let status = match self.mode {
            _ if !self.message.is_empty() => self.message.clone(),
            Mode::Normal => String::new(),
            Mode::Insert => String::from("-- INSERT --"),
            Mode::Command => alloc::format!(":{}", self.line),
        };
        Frame { lines, cursor: (row, col), status }
    }

    fn normal(&mut self, key: Key) -> Effect {
        if let Key::Char(digit @ '0'..='9') = key
            && (digit != '0' || self.count > 0)
        {
            let digit = digit as usize - '0' as usize;
            self.count = self.count.saturating_mul(10).saturating_add(digit);
            return Effect::Nothing;
        }
        let count = core::mem::take(&mut self.count);
        let n = count.max(1);
        if let Some(first) = self.pending.take() {
            return match (first, key) {
                ('g', Key::Char('g')) => self.jump(n - 1),
                ('d', Key::Char('d')) => self.delete_lines(n),
                ('Z', Key::Char('Z')) => Effect::Quit(Quit::Write),
                ('Z', Key::Char('Q')) => Effect::Quit(Quit::Force),
                _ => Effect::Nothing,
            };
        }
        match key {
            Key::Char('h') | Key::Left => self.repeat(n, Self::left),
            Key::Char('l') | Key::Right => self.repeat(n, Self::right),
            Key::Char('j') | Key::Down => self.repeat(n, |v| v.vertical(true)),
            Key::Char('k') | Key::Up => self.repeat(n, |v| v.vertical(false)),
            Key::Char('0') | Key::Home => self.go(self.line_start(self.cursor)),
            Key::Char('^') => self.go(self.first_non_blank(self.cursor)),
            Key::Char('$') | Key::End => self.line_end_motion(n),
            Key::Char('w') => self.repeat(n, Self::word_forward),
            Key::Char('b') => self.repeat(n, Self::word_back),
            Key::Char('e') => self.repeat(n, Self::word_end),
            Key::Char(first @ ('g' | 'd' | 'Z')) => {
                self.pending = Some(first);
                self.count = count;
                Effect::Nothing
            }
            Key::Char('G') => {
                self.jump(count.checked_sub(1).unwrap_or(usize::MAX))
            }
            Key::Char('i') => self.enter_insert(),
            Key::Char('a') => {
                self.cursor = self.append_at();
                self.enter_insert()
            }
            Key::Char('I') => {
                self.cursor = self.first_non_blank(self.cursor);
                self.enter_insert()
            }
            Key::Char('A') => {
                self.cursor = self.line_end(self.cursor);
                self.enter_insert()
            }
            Key::Char('o') => {
                self.cursor = self.line_end(self.cursor);
                self.enter_insert();
                self.put('\n')
            }
            Key::Char('O') => {
                let at = self.line_start(self.cursor);
                self.cursor = at;
                self.enter_insert();
                let effect = self.put('\n');
                self.cursor = at;
                effect
            }
            Key::Char('x') => self.delete_chars(n),
            Key::Char('X') => self.delete_before(n),
            Key::Char('D') => self.delete_to_end(),
            Key::Char('u') => Effect::Undo,
            Key::Ctrl('r') => Effect::Redo,
            Key::Char(':') => {
                self.mode = Mode::Command;
                Effect::Moved
            }
            _ => Effect::Nothing,
        }
    }

    fn insert(&mut self, key: Key) -> Effect {
        match key {
            Key::Char(c) => self.put(c),
            Key::Enter => self.put('\n'),
            Key::Tab => self.put('\t'),
            Key::Backspace => self.backspace(),
            Key::Left => self.arrow(Self::left),
            Key::Right => self.arrow(Self::right),
            Key::Up => self.arrow(|v| v.vertical(false)),
            Key::Down => self.arrow(|v| v.vertical(true)),
            Key::Home => self.arrow(|v| v.go(v.line_start(v.cursor))),
            Key::End => self.arrow(|v| v.go(v.line_end(v.cursor))),
            Key::Esc => {
                self.mode = Mode::Normal;
                self.cursor = self.stepped_back();
                Effect::Moved
            }
            Key::Ctrl(_) => Effect::Nothing,
        }
    }

    /// `$` with a count: `n - 1` lines down, then the last char.
    fn line_end_motion(&mut self, n: usize) -> Effect {
        let down = self.repeat(n - 1, |v| v.vertical(true));
        let end = self.go(self.last_char(self.cursor));
        if down == Effect::Moved { down } else { end }
    }

    /// Line `line` from 0, clamped to the last, at the same column.
    fn jump(&mut self, line: usize) -> Effect {
        let (from, column) = self.position();
        let line = line.min(self.line_count() - 1);
        if line == from {
            return Effect::Nothing;
        }
        let start = self.line_at(line).unwrap_or(0);
        self.go_clamped(self.at_column(start, column))
    }

    /// Insert mode at the cursor; the first edit starts a revision.
    const fn enter_insert(&mut self) -> Effect {
        self.mode = Mode::Insert;
        self.joining = false;
        Effect::Moved
    }

    /// A move inside an insert ends its undo step, as in Vim.
    fn arrow(&mut self, motion: impl FnOnce(&mut Self) -> Effect) -> Effect {
        self.joining = false;
        motion(self)
    }

    /// `c` at the cursor.
    fn put(&mut self, c: char) -> Effect {
        let at = self.cursor;
        self.text.insert(at, c);
        self.cursor += c.len_utf8();
        self.edit(at, 0, c.into())
    }

    fn backspace(&mut self) -> Effect {
        let Some(c) = self.text[..self.cursor].chars().next_back() else {
            return Effect::Nothing;
        };
        self.cursor -= c.len_utf8();
        self.text.remove(self.cursor);
        self.edit(self.cursor, c.len_utf8(), String::new())
    }

    /// An edit already applied to the text, joined to the head when
    /// an insert is under way.
    const fn edit(&mut self, at: usize, len: usize, text: String) -> Effect {
        let join = core::mem::replace(&mut self.joining, true);
        Effect::Edit { at, len, text, join }
    }

    /// `x`: up to `n` chars from the cursor on its line, a revision.
    fn delete_chars(&mut self, n: usize) -> Effect {
        let end = self.line_end(self.cursor);
        let len: usize = self.text[self.cursor..end]
            .chars()
            .take(n)
            .map(char::len_utf8)
            .sum();
        self.remove(self.cursor, len)
    }

    /// `X`: up to `n` chars before the cursor on its line.
    fn delete_before(&mut self, n: usize) -> Effect {
        let start = self.line_start(self.cursor);
        let len: usize = self.text[start..self.cursor]
            .chars()
            .rev()
            .take(n)
            .map(char::len_utf8)
            .sum();
        self.cursor -= len;
        self.remove(self.cursor, len)
    }

    /// `dd`: `n` lines from the cursor's, the last line taking the
    /// newline before it; the cursor lands on the first non-blank.
    fn delete_lines(&mut self, n: usize) -> Effect {
        let start = self.line_start(self.cursor);
        let mut end = self.line_end(self.cursor);
        for _ in 1..n {
            end = self.next_line_end(end);
        }
        let (at, len) = if end < self.text.len() {
            (start, end + 1 - start)
        } else {
            let at = start.saturating_sub(1);
            (at, self.text.len() - at)
        };
        let effect = self.remove(at, len);
        let at = at.min(self.bound());
        self.cursor = self.first_non_blank(self.line_start(at));
        self.clamp();
        effect
    }

    /// The end of the line after the one ending at `end`, or `end` at
    /// the text's end.
    fn next_line_end(&self, end: usize) -> usize {
        if end >= self.text.len() { end } else { self.line_end(end + 1) }
    }

    /// `D`: from the cursor to the end of its line.
    fn delete_to_end(&mut self) -> Effect {
        let len = self.line_end(self.cursor) - self.cursor;
        self.remove(self.cursor, len)
    }

    /// `len` bytes at `at` gone as a revision of their own; the cursor
    /// clamped after. Nothing when there is nothing to remove.
    fn remove(&mut self, at: usize, len: usize) -> Effect {
        if len == 0 {
            return Effect::Nothing;
        }
        self.text.replace_range(at..at + len, "");
        self.cursor = self.cursor.min(at);
        self.clamp();
        self.joining = false;
        Effect::Edit { at, len, text: String::new(), join: false }
    }

    /// `motion` `n` times, or until one goes nowhere; `Moved` if any
    /// of them moved.
    fn repeat(&mut self, n: usize, motion: fn(&mut Self) -> Effect) -> Effect {
        let moved =
            (0..n).take_while(|_| motion(self) == Effect::Moved).count();
        if moved > 0 { Effect::Moved } else { Effect::Nothing }
    }
}
