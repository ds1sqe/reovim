//! `[count] operator [count] motion`: the motions as one type, their
//! spans under an operator with `:help exclusive`'s rules, the three
//! operators, the register, `p`, `P` and `J`.

use alloc::string::String;

use crate::{Edit, Effect, Key, Mode, Register, Vim};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operator {
    Delete,
    Change,
    Yank,
    ShiftRight,
    ShiftLeft,
    Upper,
    Lower,
    Toggle,
}

impl Operator {
    /// The key that, typed again, makes the operator take lines.
    pub(crate) const fn key(self) -> char {
        match self {
            Self::Delete => 'd',
            Self::Change => 'c',
            Self::Yank => 'y',
            Self::ShiftRight => '>',
            Self::ShiftLeft => '<',
            Self::Upper => 'U',
            Self::Lower => 'u',
            Self::Toggle => '~',
        }
    }

    const fn linewise(self) -> bool {
        matches!(self, Self::ShiftRight | Self::ShiftLeft)
    }
}

/// `f`, `t`, `F`, `T`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Find {
    Forward,
    ForwardTo,
    Backward,
    BackwardTo,
}

impl Find {
    const fn forward(self) -> bool {
        matches!(self, Self::Forward | Self::ForwardTo)
    }

    /// `t` and `T` stop before the char.
    const fn before(self) -> bool {
        matches!(self, Self::ForwardTo | Self::BackwardTo)
    }

    const fn reversed(self) -> Self {
        match self {
            Self::Forward => Self::Backward,
            Self::ForwardTo => Self::BackwardTo,
            Self::Backward => Self::Forward,
            Self::BackwardTo => Self::ForwardTo,
        }
    }
}

/// A motion, for the cursor or for an operator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Motion {
    Left,
    Right,
    Up,
    Down,
    LineStart,
    FirstNonBlank,
    LineEnd,
    WordForward,
    WordBack,
    WordEnd,
    /// `gg` and `G`: a line from 0, clamped.
    Line(usize),
    Find(Find, char),
    /// `;` and `,`: the last find, `reversed` for `,`.
    FindAgain {
        reversed: bool,
    },
    Percent,
    /// `dd`, `cc`, `yy`: whole lines.
    Lines,
    /// `iw`, `a(`, `ip` and the like.
    Object {
        inner: bool,
        kind: char,
    },
}

/// What an operator does at a span's end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Exclusive,
    Inclusive,
    Linewise,
}

/// A span for an operator: `to` is exclusive of the char there.
pub type Span = (usize, usize, Kind);

impl Motion {
    /// The motion one key names, `count` being the digits before it.
    pub(crate) const fn of(key: Key, count: usize) -> Option<Self> {
        let motion = match key {
            Key::Char('h') | Key::Left => Self::Left,
            Key::Char('l') | Key::Right => Self::Right,
            Key::Char('k') | Key::Up => Self::Up,
            Key::Char('j') | Key::Down => Self::Down,
            Key::Char('0') | Key::Home => Self::LineStart,
            Key::Char('^') => Self::FirstNonBlank,
            Key::Char('$') | Key::End => Self::LineEnd,
            Key::Char('w') => Self::WordForward,
            Key::Char('b') => Self::WordBack,
            Key::Char('e') => Self::WordEnd,
            Key::Char('G') => Self::Line(count.wrapping_sub(1)),
            Key::Char(';') => Self::FindAgain { reversed: false },
            Key::Char(',') => Self::FindAgain { reversed: true },
            Key::Char('%') => Self::Percent,
            _ => return None,
        };
        Some(motion)
    }

    /// `$` under an operator already stops past the last char, so it
    /// is exclusive here.
    const fn kind(self, find: Option<(Find, char)>) -> Kind {
        let last_forward = match find {
            Some((kind, _)) => kind.forward(),
            None => false,
        };
        match self {
            Self::Up | Self::Down | Self::Line(_) | Self::Lines => {
                Kind::Linewise
            }
            Self::WordEnd | Self::Percent => Kind::Inclusive,
            Self::Find(kind, _) if kind.forward() => Kind::Inclusive,
            Self::FindAgain { reversed } if last_forward != reversed => {
                Kind::Inclusive
            }
            _ => Kind::Exclusive,
        }
    }
}

impl Vim {
    /// `motion` `n` times, for the cursor.
    pub(crate) fn move_by(&mut self, motion: Motion, n: usize) -> Effect {
        match motion {
            Motion::Left => self.repeat(n, Self::left),
            Motion::Right => self.repeat(n, Self::right),
            Motion::Up => self.repeat(n, |v| v.vertical(false)),
            Motion::Down => self.repeat(n, |v| v.vertical(true)),
            Motion::LineStart => self.go(self.line_start(self.cursor)),
            Motion::FirstNonBlank => self.go(self.first_non_blank(self.cursor)),
            Motion::LineEnd => self.line_end_motion(n),
            Motion::WordForward => self.repeat(n, Self::word_forward),
            Motion::WordBack => self.repeat(n, Self::word_back),
            Motion::WordEnd => self.repeat(n, Self::word_end),
            Motion::Line(line) => self.jump(line),
            Motion::Find(kind, c) => self.repeat_find(n, kind, c),
            Motion::FindAgain { reversed } => self.find_again(n, reversed),
            Motion::Percent => self.percent(),
            Motion::Lines | Motion::Object { .. } => Effect::Nothing,
        }
    }

    /// `$` with a count: `n - 1` lines down, then the last char, or
    /// the line's end for an operator; a vertical motion after it
    /// wants the end of its line too.
    fn line_end_motion(&mut self, n: usize) -> Effect {
        let down = self.repeat(n - 1, |v| v.vertical(true));
        let to = if self.pending.running {
            self.line_end(self.cursor)
        } else {
            self.last_char(self.cursor)
        };
        let end = self.go(to);
        self.want = usize::MAX;
        self.keep_want = true;
        if down == Effect::Moved { down } else { end }
    }

    /// Line `line` from 0, clamped to the last, at the wanted column.
    pub(crate) fn jump(&mut self, line: usize) -> Effect {
        let from = self.position().0;
        let line = line.min(self.line_count() - 1);
        self.keep_want = true;
        if line == from {
            return Effect::Nothing;
        }
        let start = self.line_at(line).unwrap_or(0);
        self.go_clamped(self.at_column(start, self.want))
    }

    fn find_again(&mut self, n: usize, reversed: bool) -> Effect {
        let Some((kind, c)) = self.find else { return Effect::Nothing };
        let kind = if reversed { kind.reversed() } else { kind };
        self.repeat_find(n, kind, c)
    }

    fn repeat_find(&mut self, n: usize, kind: Find, c: char) -> Effect {
        let once = |v: &mut Self| {
            v.found(kind, c).map_or(Effect::Nothing, |to| v.go(to))
        };
        let moved = (0..n).take_while(|_| once(self) == Effect::Moved).count();
        if moved > 0 { Effect::Moved } else { Effect::Nothing }
    }

    /// Where `c` is on the cursor's line in `kind`'s direction; for
    /// `t` and `T` the char before it, skipping one already adjacent.
    fn found(&self, kind: Find, c: char) -> Option<usize> {
        if kind.forward() {
            self.found_forward(kind.before(), c)
        } else {
            self.found_back(kind.before(), c)
        }
    }

    fn found_forward(&self, before: bool, c: char) -> Option<usize> {
        let end = self.line_end(self.cursor);
        let from = self.forward_char(self.cursor).min(end);
        let adjacent = before && self.text[from..end].starts_with(c);
        let from =
            if adjacent { self.forward_char(from).min(end) } else { from };
        let at = from + self.text[from..end].find(c)?;
        Some(if before { self.prev_char(at) } else { at })
    }

    fn found_back(&self, before: bool, c: char) -> Option<usize> {
        let start = self.line_start(self.cursor);
        let adjacent = before && self.text[start..self.cursor].ends_with(c);
        let to =
            if adjacent { self.prev_char(self.cursor) } else { self.cursor };
        let at = start + self.text[start..to].rfind(c)?;
        Some(if before { self.forward_char(at) } else { at })
    }

    /// `%`: from the first bracket at or after the cursor on its line
    /// to the one it pairs with.
    fn percent(&mut self) -> Effect {
        self.matching_bracket().map_or(Effect::Nothing, |to| self.go(to))
    }

    fn matching_bracket(&self) -> Option<usize> {
        let end = self.line_end(self.cursor);
        let brackets = ['(', ')', '[', ']', '{', '}'];
        let at = self.cursor + self.text[self.cursor..end].find(brackets)?;
        let bytes = self.text.as_bytes();
        let open = bytes[at];
        let (pair, step): (u8, isize) = match open {
            b'(' => (b')', 1),
            b'[' => (b']', 1),
            b'{' => (b'}', 1),
            b')' => (b'(', -1),
            b']' => (b'[', -1),
            _ => (b'{', -1),
        };
        let (mut i, mut depth) = (at, 1usize);
        while depth > 0 {
            i = i.checked_add_signed(step)?;
            let b = *bytes.get(i)?;
            depth += usize::from(b == open);
            depth -= usize::from(b == pair);
        }
        Some(i)
    }

    /// The span `motion` covers from the cursor, `n` times, for an
    /// operator, after `:help exclusive`'s adjustments: an exclusive
    /// motion ending at a line's start ends at the end of the line
    /// before instead (rule 1), and from a line's first non-blank or
    /// earlier it takes whole lines (rule 2). `None` when the motion
    /// goes nowhere.
    fn span(&mut self, motion: Motion, n: usize) -> Option<Span> {
        if let Motion::Object { inner, kind } = motion {
            return self.object_span(inner, kind);
        }
        let (start, want) = (self.cursor, self.want);
        let mut kind = motion.kind(self.find);
        self.pending.running = true;
        let moved = match motion {
            Motion::Lines => self.lines_down(n - 1),
            Motion::WordForward => self.words_for_operator(n),
            _ => self.move_by(motion, n),
        };
        self.pending.running = false;
        let end = self.cursor;
        (self.cursor, self.want) = (start, want);
        if moved == Effect::Nothing {
            return None;
        }
        let (from, to) = if end >= start { (start, end) } else { (end, start) };
        let inclusive = kind == Kind::Inclusive;
        let mut to = if inclusive { self.forward_char(to) } else { to };
        let rule1 =
            kind == Kind::Exclusive && to > from && self.line_start(to) == to;
        if rule1 {
            to = self.prev_char(to);
        }
        if rule1 && from <= self.first_non_blank(from) && from != to {
            kind = Kind::Linewise;
        }
        Some((from, to.min(self.text.len()), kind))
    }

    /// `n` lines down for `dd` and the like: a span unless fewer
    /// lines are left, when the whole command fails as Vim's does.
    fn lines_down(&mut self, n: usize) -> Effect {
        if self.position().0 + n >= self.line_count() {
            return Effect::Nothing;
        }
        self.repeat(n, |v| v.vertical(true));
        Effect::Moved
    }

    /// `w` under an operator: the last of `n` stops at a line's end
    /// rather than crossing it, as Vim's does.
    fn words_for_operator(&mut self, n: usize) -> Effect {
        let before = self.repeat(n - 1, Self::word_forward);
        let last = self.word_forward_op();
        if before == Effect::Moved || last == Effect::Moved {
            Effect::Moved
        } else {
            Effect::Nothing
        }
    }

    /// `op` over `motion`, `n` times: the register filled, the text
    /// changed, the cursor placed.
    pub(crate) fn operate(
        &mut self,
        op: Operator,
        n: usize,
        motion: Motion,
    ) -> Effect {
        // `cw` on a word is `ce`, stopping on a word's last char.
        let as_end = motion == Motion::WordForward
            && op == Operator::Change
            && !self.on_blank();
        let span = if as_end {
            self.change_word_span(n)
        } else {
            self.span(motion, n)
        };
        let Some(span) = span else { return Effect::Nothing };
        self.apply(op, span)
    }

    /// `op` over `span`.
    pub(crate) fn apply(&mut self, op: Operator, span: Span) -> Effect {
        let (from, to, kind) = span;
        let linewise = kind == Kind::Linewise || op.linewise();
        if op.linewise() {
            return self.shifted(from, to, op == Operator::ShiftRight);
        }
        let (at, len) = if linewise {
            self.whole_lines(from, to)
        } else {
            (from, to - from)
        };
        match op {
            Operator::Upper | Operator::Lower | Operator::Toggle => {
                return self.recased(op, at, len, linewise);
            }
            _ => {}
        }
        let text = if linewise {
            &self.text[self.line_start(from)..self.line_end(to)]
        } else {
            &self.text[at..at + len]
        };
        self.register = Register { text: text.into(), linewise };
        match op {
            Operator::Delete => self.deleted(at, len, linewise),
            Operator::Change if linewise => self.change_lines(from, to),
            Operator::Change => self.changed(at, len),
            _ => self.yanked(at, linewise),
        }
    }

    /// `>` and `<` over the lines `from` to `to`: each non-empty
    /// line's indent moved by `shiftwidth` and written with tabs, as
    /// nvim's defaults have it.
    fn shifted(&mut self, from: usize, to: usize, right: bool) -> Effect {
        let start = self.line_start(from);
        let end = self.line_end(to);
        let block = self.text[start..end]
            .split('\n')
            .map(|line| shift(line, right))
            .collect::<alloc::vec::Vec<_>>()
            .join("\n");
        self.text.replace_range(start..end, &block);
        self.cursor = self.first_non_blank(start);
        self.clamp();
        self.joining = false;
        self.edit(start, end - start, block)
    }

    /// `gu`, `gU`, `g~` over `len` bytes at `at`, the cursor at the
    /// start; on whole lines their newlines stay.
    fn recased(
        &mut self,
        op: Operator,
        at: usize,
        len: usize,
        linewise: bool,
    ) -> Effect {
        let (at, len) = if linewise {
            let end =
                at + len - usize::from(self.text[at..at + len].ends_with('\n'));
            let at = at + usize::from(self.text[at..].starts_with('\n'));
            (at, end - at)
        } else {
            (at, len)
        };
        let cased: String =
            self.text[at..at + len].chars().map(|c| recase(op, c)).collect();
        self.text.replace_range(at..at + len, &cased);
        self.cursor = if linewise { self.first_non_blank(at) } else { at };
        self.clamp();
        self.joining = false;
        self.edit(at, len, cased)
    }

    /// `~`: the case of `n` chars from the cursor on its line
    /// toggled, the cursor moved past them.
    pub(crate) fn tilde(&mut self, n: usize) -> Effect {
        let end = self.line_end(self.cursor);
        let len: usize = self.text[self.cursor..end]
            .chars()
            .take(n)
            .map(char::len_utf8)
            .sum();
        if len == 0 {
            return Effect::Nothing;
        }
        let at = self.cursor;
        let cased: String = self.text[at..at + len]
            .chars()
            .map(|c| recase(Operator::Toggle, c))
            .collect();
        self.text.replace_range(at..at + len, &cased);
        self.cursor = at + len;
        self.clamp();
        self.joining = false;
        self.edit(at, len, cased)
    }

    /// After a yank: the cursor at the span's start.
    fn yanked(&mut self, at: usize, linewise: bool) -> Effect {
        let start = at.min(self.bound());
        let to = if linewise { self.first_non_blank(start) } else { start };
        self.go_clamped(to);
        Effect::Moved
    }

    /// After a delete: the cursor where the span was, on a line's
    /// first non-blank when lines went. Chars taken down to nothing
    /// leave one empty line, as Vim's buffer always has a line.
    fn deleted(&mut self, at: usize, len: usize, linewise: bool) -> Effect {
        let removed = self.remove(at, len);
        let emptied = !linewise && self.text.is_empty();
        let effect = if emptied {
            self.text.push('\n');
            Effect::Edit(Edit {
                at: 0,
                len,
                text: String::from("\n"),
                join: false,
            })
        } else {
            removed
        };
        let start = at.min(self.bound());
        let line = self.line_start(start);
        self.cursor = if linewise { self.first_non_blank(line) } else { start };
        self.clamp();
        effect
    }

    /// After a charwise change: insert mode where the span was, its
    /// typing joining the delete's revision.
    fn changed(&mut self, at: usize, len: usize) -> Effect {
        let effect = self.remove(at, len);
        self.cursor = at;
        self.mode = Mode::Insert;
        self.indent = 0;
        if effect == Effect::Nothing { Effect::Moved } else { effect }
    }

    /// `ce` under `c`: on a word's last char, just that char; else
    /// `e`.
    fn change_word_span(&mut self, n: usize) -> Option<Span> {
        let next = self.forward_char(self.cursor);
        let at_end =
            next >= self.text.len() || self.class_changes(self.cursor, next);
        if n == 1 && at_end {
            return Some((
                self.cursor,
                next.min(self.text.len()),
                Kind::Exclusive,
            ));
        }
        self.span(Motion::WordEnd, n)
    }

    /// The lines from the one `from` is on to the one `to` is on, as
    /// a byte range with the newline after them, or before the last.
    pub(crate) fn whole_lines(&self, from: usize, to: usize) -> (usize, usize) {
        let start = self.line_start(from);
        let end = self.line_end(to);
        if end < self.text.len() {
            (start, end + 1 - start)
        } else {
            let at = start.saturating_sub(1);
            (at, self.text.len() - at)
        }
    }

    /// `cc`: the lines' content replaced by the first line's indent,
    /// which `autoindent` keeps; insert mode after it.
    fn change_lines(&mut self, from: usize, to: usize) -> Effect {
        let start = self.line_start(from);
        let end = self.line_end(to);
        let indent = self.indent_of(from);
        self.text.replace_range(start..end, &indent);
        self.cursor = start + indent.len();
        self.mode = Mode::Insert;
        self.indent = indent.len();
        self.joining = false;
        self.edit(start, end - start, indent)
    }

    /// `p` after the cursor or `P` before it, `n` times; a linewise
    /// register goes on lines of its own.
    pub(crate) fn paste(&mut self, after: bool, n: usize) -> Effect {
        let Register { text, linewise } = self.register.clone();
        if text.is_empty() {
            Effect::Nothing
        } else if linewise {
            self.paste_lines(after, n, &text)
        } else {
            self.paste_chars(after, n, &text)
        }
    }

    fn paste_lines(&mut self, after: bool, n: usize, text: &str) -> Effect {
        let block = alloc::format!("{text}\n").repeat(n);
        let end = self.line_end(self.cursor);
        let last = end >= self.text.len();
        let at = match (after, last) {
            (false, _) => self.line_start(self.cursor),
            (true, false) => end + 1,
            (true, true) => end,
        };
        let put = if after && last {
            alloc::format!("\n{}", block.trim_end_matches('\n'))
        } else {
            block
        };
        self.text.insert_str(at, &put);
        self.joining = false;
        let first = at + usize::from(put.starts_with('\n'));
        self.cursor = self.first_non_blank(first);
        self.clamp();
        self.edit(at, 0, put)
    }

    fn paste_chars(&mut self, after: bool, n: usize, text: &str) -> Effect {
        let put = text.repeat(n);
        let at = if after { self.append_at() } else { self.cursor };
        self.text.insert_str(at, &put);
        self.joining = false;
        let end = self.prev_char(at + put.len());
        self.cursor = if put.contains('\n') { at } else { end };
        self.edit(at, 0, put)
    }

    /// `J`: `n` lines (at least two) made one, a space between unless
    /// the next starts with `)` or the line ends in a blank.
    pub(crate) fn join(&mut self, n: usize) -> Effect {
        let line = self.position().0;
        let joins = (n.max(2) - 1).min(self.line_count() - 1 - line);
        if joins == 0 {
            return Effect::Nothing;
        }
        let start = self.line_start(self.cursor);
        let mut out = String::from(&self.text[start..self.line_end(start)]);
        let mut end = self.line_end(start);
        let mut seam = start;
        for _ in 0..joins {
            let next = self.line_end(end + 1);
            let piece =
                self.text[end + 1..next].trim_start_matches([' ', '\t']);
            seam = start + out.len();
            out.push_str(gap(&out, piece));
            out.push_str(piece);
            end = next;
        }
        self.text.replace_range(start..end, &out);
        self.cursor = seam;
        self.clamp();
        self.joining = false;
        self.edit(start, end - start, out)
    }
}

/// What `J` puts between `out` and `piece`: one space, or nothing
/// before an empty piece or a `)`, or after a trailing blank.
fn gap(out: &str, piece: &str) -> &'static str {
    let none = piece.is_empty()
        || piece.starts_with(')')
        || out.ends_with([' ', '\t']);
    if none { "" } else { " " }
}

/// One char in `op`'s case.
pub fn recase(op: Operator, c: char) -> char {
    let upper = match op {
        Operator::Upper => true,
        Operator::Lower => false,
        _ => c.is_lowercase(),
    };
    let cased: alloc::vec::Vec<char> = if upper {
        c.to_uppercase().collect()
    } else {
        c.to_lowercase().collect()
    };
    match cased[..] {
        [one] => one,
        _ => c,
    }
}

/// `line` with its indent moved by `shiftwidth` (8) to the right or
/// left, tabs first; an empty line stays.
fn shift(line: &str, right: bool) -> String {
    if line.is_empty() {
        return String::new();
    }
    let rest = line.trim_start_matches([' ', '\t']);
    let columns =
        line[..line.len() - rest.len()].chars().fold(0usize, |col, c| {
            if c == '\t' { col / 8 * 8 + 8 } else { col + 1 }
        });
    let columns = if right { columns + 8 } else { columns.saturating_sub(8) };
    let mut out = "\t".repeat(columns / 8);
    out.push_str(&" ".repeat(columns % 8));
    out.push_str(rest);
    out
}
