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
mod object;
mod operator;
mod pattern;
mod search;
mod visual;
mod word;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

pub use operator::Motion;
pub use reovim_client::Key;
pub use visual::Visual;

use operator::{Find, Operator};
use search::Search;
use visual::{BlockInsert, Selection};

/// One change to the text, already applied here: `len` bytes at
/// `at` became `text`. With `join` the edit belongs to the head
/// revision, not a new one: one insert is one undo step.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Edit {
    pub at: usize,
    pub len: usize,
    pub text: String,
    pub join: bool,
}

/// What a key did.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Effect {
    Nothing,
    /// The cursor or the mode moved; the text did not.
    Moved,
    Edit(Edit),
    /// Several edits in order, as `.` replays them.
    Edits(Vec<Edit>),
    Undo,
    Redo,
    Save,
    Quit(Quit),
    /// A put wants the clipboard's text: `pasted` finishes it.
    Clipboard,
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

/// What to draw.
///
/// `rows - 1` text rows, the cursor as (row, column) on them, the
/// selected cells as `(row, from, to)` with `to` exclusive, and Vim's
/// own word for the status row: a message, the command line, the
/// mode, or nothing.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Frame {
    pub lines: Vec<String>,
    pub cursor: (u16, u16),
    pub selection: Vec<(u16, u16, u16)>,
    /// `hlsearch`'s matches, in the selection's shape.
    pub marks: Vec<(u16, u16, u16)>,
    pub status: String,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Mode {
    #[default]
    Normal,
    Insert,
    Command,
    Visual(Visual),
}

/// What the unnamed register holds: text, and whether it came from
/// whole lines.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Register {
    pub text: String,
    pub linewise: bool,
}

/// What a normal-mode command has typed so far.
#[derive(Debug, Default)]
struct Pending {
    /// Digits typed before a command; 0 is none.
    count: usize,
    /// A `g`, `d`, `Z`, `f` or the like waiting for its second key.
    key: Option<char>,
    /// An operator waiting for its motion, with its own count.
    operator: Option<(Operator, usize)>,
    /// `"x` typed: the register the next operator or put uses.
    register: Option<char>,
    /// A motion is being run for an operator, not for the cursor.
    running: bool,
}

/// What `.` needs: the keys of the command under way and of the last
/// change.
#[derive(Debug, Default)]
struct Repeat {
    record: Vec<Key>,
    last: Vec<Key>,
    /// `.` is feeding keys back in.
    replaying: bool,
}

/// Where the cursor was before each change, for undo to put it back
/// as nvim does, and how many of those changes stand undone.
#[derive(Debug, Default)]
struct Undone {
    before: Vec<(usize, usize)>,
    count: usize,
    /// An undo (`false`) or redo (`true`) is waiting for its reload.
    walking: Option<bool>,
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
    pending: Pending,
    /// The next insert edit joins the head revision.
    joining: bool,
    /// The screen column a vertical motion wants, `usize::MAX` after
    /// `$`.
    want: usize,
    /// The last key kept `want` rather than setting it.
    keep_want: bool,
    /// Bytes of indent `o`, `O` or Enter just copied, dropped again if
    /// nothing is typed on the line.
    indent: usize,
    register: Register,
    /// `"a` to `"z`.
    registers: BTreeMap<char, Register>,
    /// Text for the clipboard, until the tui takes it.
    clipboard: Option<String>,
    /// A put waiting for the clipboard: after the cursor, its count,
    /// and the selection it goes over.
    put: Option<(bool, usize, Option<Visual>)>,
    /// What the clipboard last gave, for `.` to put again.
    last_clip: Register,
    /// The last `f`, `t`, `F` or `T`, for `;` and `,`.
    find: Option<(Find, char)>,
    repeat: Repeat,
    undone: Undone,
    /// The selection's other end in visual mode.
    anchor: usize,
    /// `$` was the last motion of a selection: it reaches every line's
    /// end.
    to_end: bool,
    last_visual: Option<Selection>,
    /// A block insert under way.
    block: Option<BlockInsert>,
    search: Search,
    /// `:`, `/` or `?`: what the command line is for.
    prompt: char,
    /// The mode the command line returns to.
    before: Mode,
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

    #[must_use]
    pub const fn register(&self) -> &Register {
        &self.register
    }

    /// `bytes` as the text, lossily; the cursor stays where it can.
    /// True when the text differs from before.
    pub fn loaded(&mut self, bytes: &[u8]) -> bool {
        let text = String::from_utf8_lossy(bytes);
        let walking = self.undone.walking.take();
        if text == self.text {
            return false;
        }
        let common =
            self.text.bytes().zip(text.bytes()).take_while(|(a, b)| a == b);
        let top = self.text[..common.count()].matches('\n').count();
        self.text = text.into_owned();
        match walking {
            Some(redo) => self.undone_cursor(top, redo),
            None => self.cursor = self.cursor.min(self.bound()),
        }
        while !self.text.is_char_boundary(self.cursor) {
            self.cursor -= 1;
        }
        if self.mode == Mode::Normal {
            self.clamp();
        }
        true
    }

    /// After an undo or redo, as nvim places it: on the first changed
    /// line (`top`), at the column the cursor had before that change
    /// when this was its line, one line up for a change made by `o`,
    /// else at the wanted column.
    fn undone_cursor(&mut self, top: usize, redo: bool) {
        let made = self.undone.before.len() - self.undone.count;
        let before = if redo {
            self.undone.count = self.undone.count.saturating_sub(1);
            self.undone.before.get(made)
        } else {
            self.undone.count = (self.undone.count + 1).min(made.max(1));
            made.checked_sub(1).and_then(|i| self.undone.before.get(i))
        };
        let line = match before {
            Some(&(was, _)) if was + 1 == top && top > 0 => was,
            _ => top,
        };
        let start = self.line_at(line).unwrap_or_else(|| {
            self.line_at(self.line_count() - 1).unwrap_or(0)
        });
        self.cursor = match before {
            Some(&(was, column)) if was == line => {
                self.at_column(start, column)
            }
            _ => self.at_screen_column(start, self.want),
        };
        self.want = self.wanted();
    }

    /// A change is being made: the cursor `before` it kept for undo;
    /// what stood undone is forgotten, as a new change ends redo.
    fn changing(&mut self, before: (usize, usize), effect: &Effect) {
        let change = match effect {
            Effect::Edit(edit) => !edit.join,
            Effect::Edits(_) => true,
            Effect::Undo => return self.undone.walking = Some(false),
            Effect::Redo => return self.undone.walking = Some(true),
            _ => false,
        };
        if change && !self.repeat.replaying {
            let made = self.undone.before.len() - self.undone.count;
            self.undone.before.truncate(made);
            self.undone.before.push(before);
            self.undone.count = 0;
        }
    }

    /// One key in the current mode; a message showing goes with it.
    pub fn key(&mut self, key: Key) -> Effect {
        if !self.repeat.replaying {
            self.repeat.record.push(key);
        }
        let shown = !self.message.is_empty();
        self.message.clear();
        self.undone.walking = None;
        let before = self.position();
        let effect = match self.mode {
            Mode::Normal => self.normal(key),
            Mode::Insert => self.insert(key),
            Mode::Command => self.command(key),
            Mode::Visual(kind) => {
                let effect = self.visual(kind, key);
                self.settle(&effect);
                effect
            }
        };
        self.changing(before, &effect);
        if !core::mem::take(&mut self.keep_want) {
            self.want = self.wanted();
        }
        if matches!(self.mode, Mode::Visual(_)) {
            self.to_end = self.want == usize::MAX;
        }
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
        let text = String::from("\n");
        Some(Effect::Edit(Edit { at, len: 0, text, join: false }))
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
            Mode::Command => alloc::format!("{}{}", self.prompt, self.line),
            Mode::Visual(Visual::Char) => String::from("-- VISUAL --"),
            Mode::Visual(Visual::Line) => String::from("-- VISUAL LINE --"),
            Mode::Visual(Visual::Block) => String::from("-- VISUAL BLOCK --"),
        };
        let selection = match self.mode {
            Mode::Visual(kind) => self.selection_cells(kind, self.top, height),
            _ => Vec::new(),
        };
        let marks = self.search_marks(self.top, height);
        Frame { lines, cursor: (row, col), selection, marks, status }
    }

    fn normal(&mut self, key: Key) -> Effect {
        if let Key::Char(digit @ '0'..='9') = key
            && (digit != '0' || self.pending.count > 0)
        {
            let digit = digit as usize - '0' as usize;
            self.pending.count =
                self.pending.count.saturating_mul(10).saturating_add(digit);
            self.keep_want = true;
            return Effect::Nothing;
        }
        let count = core::mem::take(&mut self.pending.count);
        let n = count.max(1);
        if let Some(first) = self.pending.key.take() {
            return self.second(first, key, count);
        }
        if let Some((op, first)) = self.pending.operator
            && key == Key::Char(op.key())
        {
            self.pending.operator = None;
            return self.lines(op, first * n);
        }
        if let Some(motion) = Motion::of(key, count) {
            return self.motion(motion, n);
        }
        let effect = match key {
            Key::Char(first @ ('g' | 'f' | 't' | 'F' | 'T' | '"')) => {
                return self.wait(first, count);
            }
            Key::Char(first @ ('i' | 'a'))
                if self.pending.operator.is_some() =>
            {
                return self.wait(first, count);
            }
            Key::Char('Z') if self.pending.operator.is_none() => {
                return self.wait('Z', count);
            }
            Key::Char('d') => return self.operator(Operator::Delete, n),
            Key::Char('c') => return self.operator(Operator::Change, n),
            Key::Char('y') => return self.operator(Operator::Yank, n),
            Key::Char('>') => return self.operator(Operator::ShiftRight, n),
            Key::Char('<') => return self.operator(Operator::ShiftLeft, n),
            Key::Char(c @ ('/' | '?')) => return self.prompt(c, count),
            _ if self.pending.operator.take().is_some() => Effect::Nothing,
            Key::Char('~') => self.tilde(n),
            Key::Char('v') => self.enter_visual(Visual::Char),
            Key::Char('V') => self.enter_visual(Visual::Line),
            Key::Ctrl('v') => self.enter_visual(Visual::Block),
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
                let indent = self.indent_of(self.cursor);
                self.cursor = self.line_end(self.cursor);
                self.enter_insert();
                self.open_line(&indent)
            }
            Key::Char('O') => {
                let indent = self.indent_of(self.cursor);
                self.cursor = self.line_start(self.cursor);
                self.enter_insert();
                let effect = self.put_str(&alloc::format!("{indent}\n"));
                self.cursor -= 1;
                self.indent = indent.len();
                effect
            }
            Key::Char('x') => self.operate(Operator::Delete, n, Motion::Right),
            Key::Char('X') => self.operate(Operator::Delete, n, Motion::Left),
            Key::Char('D') => {
                self.operate(Operator::Delete, n, Motion::LineEnd)
            }
            Key::Char('C') => {
                self.operate(Operator::Change, n, Motion::LineEnd)
            }
            // nvim maps `Y` to `y$`.
            Key::Char('Y') => self.operate(Operator::Yank, n, Motion::LineEnd),
            Key::Char('p') => self.paste(true, n),
            Key::Char('P') => self.paste(false, n),
            Key::Char('J') => self.join(n),
            Key::Char('.') => return self.replay(count),
            Key::Char('u') => Effect::Undo,
            Key::Ctrl('r') => Effect::Redo,
            Key::Char(':') => self.prompt(':', count),
            _ => Effect::Nothing,
        };
        self.settle(&effect);
        effect
    }

    /// The second key of a pair: `gg`, `dd`, `ZZ`, `fx` and the like.
    fn second(&mut self, first: char, key: Key, count: usize) -> Effect {
        let n = count.max(1);
        let motion = match (first, key) {
            ('g', Key::Char('g')) => Motion::Line(count.saturating_sub(1)),
            ('g', Key::Char('u')) => return self.operator(Operator::Lower, n),
            ('g', Key::Char('U')) => return self.operator(Operator::Upper, n),
            ('g', Key::Char('~')) => return self.operator(Operator::Toggle, n),
            ('g', Key::Char('v')) => return self.reselect(),
            ('"', Key::Char(name)) => return self.named(name, count),
            ('i' | 'a', Key::Char(kind)) => {
                Motion::Object { inner: first == 'i', kind }
            }
            ('f', Key::Char(c)) => Motion::Find(Find::Forward, c),
            ('t', Key::Char(c)) => Motion::Find(Find::ForwardTo, c),
            ('F', Key::Char(c)) => Motion::Find(Find::Backward, c),
            ('T', Key::Char(c)) => Motion::Find(Find::BackwardTo, c),
            ('Z', Key::Char('Z')) => return Effect::Quit(Quit::Write),
            ('Z', Key::Char('Q')) => return Effect::Quit(Quit::Force),
            _ => {
                self.pending.operator = None;
                self.repeat.record.clear();
                return Effect::Nothing;
            }
        };
        if let Motion::Find(kind, c) = motion {
            self.find = Some((kind, c));
        }
        self.motion(motion, n)
    }

    /// `"x`: the register `name` for what comes next; the count waits.
    pub(crate) const fn named(&mut self, name: char, count: usize) -> Effect {
        self.pending.register = Some(name);
        self.pending.count = count;
        self.keep_want = true;
        Effect::Nothing
    }

    /// The text a `"+` or `"*` yank left for the clipboard.
    pub const fn take_clipboard(&mut self) -> Option<String> {
        self.clipboard.take()
    }

    /// The clipboard's answer to `Effect::Clipboard`: the put goes on
    /// with `text`, or E353 when there is none.
    pub fn pasted(&mut self, text: Option<String>) -> Effect {
        let Some((after, n, over)) = self.put.take() else {
            return Effect::Nothing;
        };
        let before = self.position();
        let effect = if let Some(text) = text {
            self.put_text(&text, after, n, over)
        } else {
            self.message = String::from("E353: Nothing in register +");
            Effect::Moved
        };
        self.changing(before, &effect);
        self.settle(&effect);
        effect
    }

    /// The clipboard's `text` put: lines of its own when it ends in a
    /// newline, as nvim reads it.
    fn put_text(
        &mut self,
        text: &str,
        after: bool,
        n: usize,
        over: Option<Visual>,
    ) -> Effect {
        let linewise = text.ends_with('\n');
        let text = String::from(text.strip_suffix('\n').unwrap_or(text));
        let register = Register { text, linewise };
        self.last_clip = register.clone();
        match over {
            Some(kind) => self.paste_over(kind, register),
            None => self.paste_register(register, after, n),
        }
    }

    /// `first` waits for its second key; the count waits with it.
    const fn wait(&mut self, first: char, count: usize) -> Effect {
        self.pending.key = Some(first);
        self.pending.count = count;
        self.keep_want = true;
        Effect::Nothing
    }

    /// An operator key: the operator waits for a motion, or doubled
    /// (`dd`, `cc`, `yy`, `gUgU`) takes `n` lines.
    fn operator(&mut self, op: Operator, n: usize) -> Effect {
        match self.pending.operator.take() {
            Some((waiting, count)) if waiting == op => {
                self.lines(op, count * n)
            }
            Some(_) => {
                self.repeat.record.clear();
                Effect::Nothing
            }
            None => {
                self.pending.operator = Some((op, n));
                self.keep_want = true;
                Effect::Nothing
            }
        }
    }

    /// `op` over `n` whole lines from the cursor's.
    fn lines(&mut self, op: Operator, n: usize) -> Effect {
        let effect = self.operate(op, n, Motion::Lines);
        self.settle(&effect);
        effect
    }

    /// `motion` `n` times for the cursor, or for the operator waiting.
    fn motion(&mut self, motion: Motion, n: usize) -> Effect {
        let effect = match self.pending.operator.take() {
            Some((op, count)) => self.operate(op, count * n, motion),
            None => self.move_by(motion, n),
        };
        self.settle(&effect);
        effect
    }

    /// The command is over: a change is what `.` repeats, anything
    /// else is forgotten. An insert goes on recording until Esc.
    fn settle(&mut self, effect: &Effect) {
        if self.repeat.replaying
            || matches!(self.mode, Mode::Insert | Mode::Visual(_))
        {
            return;
        }
        match effect {
            Effect::Edit(_) | Effect::Edits(_) => {
                self.repeat.last = core::mem::take(&mut self.repeat.record);
            }
            Effect::Clipboard => {}
            _ => self.repeat.record.clear(),
        }
    }

    /// `.`: the last change's keys fed back in, its count replaced
    /// by `count` when there is one; every edit they make, in order.
    fn replay(&mut self, count: usize) -> Effect {
        self.repeat.record.clear();
        let mut keys: Vec<Key> = self.repeat.last.clone();
        if count > 0 {
            let digits = keys.iter().take_while(|k| is_digit(**k)).count();
            keys.drain(..digits);
            let count = alloc::format!("{count}");
            keys.splice(0..0, count.chars().map(Key::Char));
        }
        self.repeat.replaying = true;
        let mut edits = Vec::new();
        for key in keys {
            let effect = self.key(key);
            collect(&mut edits, effect);
        }
        self.repeat.replaying = false;
        if edits.is_empty() { Effect::Nothing } else { Effect::Edits(edits) }
    }

    fn insert(&mut self, key: Key) -> Effect {
        match key {
            Key::Char(c) => self.put(c),
            Key::Enter => {
                let indent = self.indent_of(self.cursor);
                self.open_line(&indent)
            }
            Key::Tab => self.put('\t'),
            Key::Backspace => self.backspace(),
            Key::Left => self.arrow(Self::left),
            Key::Right => self.arrow(Self::right),
            Key::Up => self.arrow(|v| v.vertical(false)),
            Key::Down => self.arrow(|v| v.vertical(true)),
            Key::Home => self.arrow(|v| v.go(v.line_start(v.cursor))),
            Key::End => self.arrow(|v| v.go(v.line_end(v.cursor))),
            Key::Esc => self.leave_insert(),
            Key::Ctrl(_) => Effect::Nothing,
        }
    }

    /// Esc: back to normal mode, the insert being the last change,
    /// a pending indent dropped, the cursor stepped back.
    fn leave_insert(&mut self) -> Effect {
        self.mode = Mode::Normal;
        if !self.repeat.replaying {
            self.repeat.last = core::mem::take(&mut self.repeat.record);
        }
        if let Some(block) = self.block.take() {
            let effect = self.finish_block(block);
            self.clamp();
            return effect;
        }
        let dropped = self.drop_indent();
        self.cursor = self.stepped_back();
        dropped.unwrap_or(Effect::Moved)
    }

    /// A new line after the cursor with `indent` copied onto it, as
    /// `autoindent` does; an indent still pending on the old line goes
    /// with the same edit, so that line is left empty.
    fn open_line(&mut self, indent: &str) -> Effect {
        let pending = core::mem::take(&mut self.indent);
        let at = self.cursor - pending;
        let text = alloc::format!("\n{indent}");
        self.text.replace_range(at..self.cursor, &text);
        self.cursor = at + text.len();
        self.indent = indent.len();
        self.edit(at, pending, text)
    }

    /// The indent nothing was typed after, removed; the cursor at
    /// the line's start.
    fn drop_indent(&mut self) -> Option<Effect> {
        let pending = core::mem::take(&mut self.indent);
        if pending == 0 {
            return None;
        }
        let at = self.cursor - pending;
        self.text.replace_range(at..self.cursor, "");
        self.cursor = at;
        Some(self.edit(at, pending, String::new()))
    }

    /// Insert mode at the cursor; the first edit starts a revision.
    const fn enter_insert(&mut self) -> Effect {
        self.mode = Mode::Insert;
        self.joining = false;
        Effect::Moved
    }

    /// A move inside an insert ends its undo step, as in Vim, and
    /// keeps whatever indent was copied.
    fn arrow(&mut self, motion: impl FnOnce(&mut Self) -> Effect) -> Effect {
        self.joining = false;
        self.indent = 0;
        motion(self)
    }

    /// `c` at the cursor.
    fn put(&mut self, c: char) -> Effect {
        self.indent = 0;
        self.put_str(&String::from(c))
    }

    /// `text` at the cursor.
    fn put_str(&mut self, text: &str) -> Effect {
        let at = self.cursor;
        self.text.insert_str(at, text);
        self.cursor += text.len();
        self.edit(at, 0, String::from(text))
    }

    fn backspace(&mut self) -> Effect {
        self.indent = 0;
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
        Effect::Edit(Edit { at, len, text, join })
    }

    /// `len` bytes at `at` gone as a revision of their own. Nothing
    /// when there is nothing to remove.
    fn remove(&mut self, at: usize, len: usize) -> Effect {
        if len == 0 {
            return Effect::Nothing;
        }
        self.text.replace_range(at..at + len, "");
        self.joining = false;
        self.edit(at, len, String::new())
    }

    /// `motion` `n` times, or until one goes nowhere; `Moved` if any
    /// of them moved.
    fn repeat(&mut self, n: usize, motion: fn(&mut Self) -> Effect) -> Effect {
        let moved =
            (0..n).take_while(|_| motion(self) == Effect::Moved).count();
        if moved > 0 { Effect::Moved } else { Effect::Nothing }
    }
}

const fn is_digit(key: Key) -> bool {
    matches!(key, Key::Char('0'..='9'))
}

/// The edits `effect` made, onto `edits`.
fn collect(edits: &mut Vec<Edit>, effect: Effect) {
    match effect {
        Effect::Edit(edit) => edits.push(edit),
        Effect::Edits(more) => edits.extend(more),
        _ => {}
    }
}
