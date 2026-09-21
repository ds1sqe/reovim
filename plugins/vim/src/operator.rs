//! `[count] operator [count] motion`: the motions as one type, their
//! spans under an operator with `:help exclusive`'s rules, the three
//! operators, the register, `p`, `P` and `J`.

use alloc::string::String;
use alloc::vec::Vec;

use reovim_kernel_interface::Rd;

use crate::jumps::Jump;
use crate::{
    Effect, Key, Kind, Mode, Pos, Put, Register, Vim, Visual, edited, going,
    land, moved,
};

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

/// Why bytes enter register storage; search motions force numbered history.
#[derive(Clone, Copy)]
pub enum Took {
    Yank,
    Delete { one: bool },
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
/// A motion's semantic kind. WORD motion groups punctuation with words.
///
/// ```
/// use reovim_plugin_vim::Motion;
/// let word = Motion::WordForward { big: true }; // W
/// let previous = Motion::WordEndBack { big: false }; // ge
/// assert_ne!(word, previous);
/// let window = [Motion::High, Motion::Middle, Motion::Low];
/// let display = [Motion::ScreenDown, Motion::ScreenUp];
/// assert_ne!(window[0], display[0]);
/// let backwards = Motion::WordBack { big: true }; // B
/// let end = Motion::WordEnd { big: true }; // E
/// assert_ne!(backwards, end);
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Motion {
    Left,
    Right,
    Up,
    Down,
    LineStart,
    FirstNonBlank,
    LineEnd,
    /// Start of the next word; WORD when `big` groups punctuation.
    WordForward {
        big: bool,
    },
    /// Start of the previous word, or WORD.
    WordBack {
        big: bool,
    },
    /// End of the current or next word, or WORD.
    WordEnd {
        big: bool,
    },
    /// End of the preceding word, or WORD, including both endpoints.
    WordEndBack {
        big: bool,
    },
    /// Top visible line, adjusted by the command count.
    High,
    /// Middle visible line; counts are ignored.
    Middle,
    /// Bottom visible line, adjusted by the command count.
    Low,
    /// Next unwrapped display line; an exclusive operator motion.
    ScreenDown,
    /// Previous unwrapped display line; an exclusive operator motion.
    ScreenUp,
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
    /// `'x` and `` `x ``: `exact` for the backtick form, which lands
    /// on the mark's own column; the quote form is linewise, to the
    /// first non-blank.
    Mark {
        name: char,
        exact: bool,
    },
    /// `/` and `?` run: the last pattern in that direction.
    Search {
        forward: bool,
    },
    /// `n` and `N`.
    SearchAgain {
        reversed: bool,
    },
    /// `*` and `#`.
    Star {
        forward: bool,
    },
}

/// How far a span reaches at its end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Extent {
    Exclusive,
    Inclusive,
    Linewise,
}

/// A span for an operator: `to` is exclusive of the char there.
pub type Span = (usize, usize, Extent);

impl Motion {
    /// These otherwise-small deletes also enter numbered history.
    const fn one(self) -> bool {
        matches!(
            self,
            Self::Mark { exact: true, .. }
                | Self::Search { .. }
                | Self::SearchAgain { .. }
                | Self::Star { .. }
        )
    }

    /// The motion one key names, `count` being the digits before it.
    pub(crate) const fn of(key: Key, count: usize) -> Option<Self> {
        let motion = match key {
            Key::Char('h') | Key::Left => Self::Left,
            Key::Char('l') | Key::Right => Self::Right,
            Key::Char('k') | Key::Up => Self::Up,
            Key::Char('j') | Key::Down | Key::Ctrl('j') => Self::Down,
            Key::Char('0') | Key::Home => Self::LineStart,
            Key::Char('^') => Self::FirstNonBlank,
            Key::Char('$') | Key::End => Self::LineEnd,
            Key::Char('w') => Self::WordForward { big: false },
            Key::Char('W') => Self::WordForward { big: true },
            Key::Char('b') => Self::WordBack { big: false },
            Key::Char('B') => Self::WordBack { big: true },
            Key::Char('e') => Self::WordEnd { big: false },
            Key::Char('E') => Self::WordEnd { big: true },
            Key::Char('H') => Self::High,
            Key::Char('M') => Self::Middle,
            Key::Char('L') => Self::Low,
            Key::Char('G') => Self::Line(count.wrapping_sub(1)),
            Key::Char(';') => Self::FindAgain { reversed: false },
            Key::Char(',') => Self::FindAgain { reversed: true },
            Key::Char('%') => Self::Percent,
            Key::Char('n') => Self::SearchAgain { reversed: false },
            Key::Char('N') => Self::SearchAgain { reversed: true },
            Key::Char('*') => Self::Star { forward: true },
            Key::Char('#') => Self::Star { forward: false },
            _ => return None,
        };
        Some(motion)
    }

    /// An inner bracket object: the one object whose span over whole
    /// lines leaves the cursor on the first non-blank.
    const fn inner_block(self) -> bool {
        matches!(
            self,
            Self::Object {
                inner: true,
                kind: '(' | ')' | 'b' | '[' | ']' | '{' | '}' | 'B' | '<' | '>',
            }
        )
    }

    /// `$` under an operator already stops past the last char, so it
    /// is exclusive here.
    const fn extent(self, find: Option<(Find, char)>) -> Extent {
        let last_forward = match find {
            Some((kind, _)) => kind.forward(),
            None => false,
        };
        match self {
            Self::Up
            | Self::Down
            | Self::Line(_)
            | Self::Lines
            | Self::High
            | Self::Middle
            | Self::Low
            | Self::Mark { exact: false, .. } => Extent::Linewise,
            Self::WordEnd { .. } | Self::WordEndBack { .. } | Self::Percent => {
                Extent::Inclusive
            }
            Self::Find(kind, _) if kind.forward() => Extent::Inclusive,
            Self::FindAgain { reversed } if last_forward != reversed => {
                Extent::Inclusive
            }
            _ => Extent::Exclusive,
        }
    }
}

impl Vim {
    /// `motion` `n` times, for the cursor.
    pub(crate) fn move_by(&mut self, motion: Motion, n: usize) -> Effect {
        let bell = self.bell;
        let effect = match motion {
            Motion::Left => self.repeat(n, Self::left),
            Motion::Right => self.repeat(n, Self::right),
            Motion::Up => self.repeat(n, |v| v.vertical(false)),
            Motion::Down => self.repeat(n, |v| v.vertical(true)),
            Motion::LineStart => self.go(self.line_start(self.cursor)),
            Motion::FirstNonBlank => self.go(self.first_non_blank(self.cursor)),
            Motion::LineEnd => self.line_end_motion(n),
            Motion::WordForward { big } => {
                self.repeat(n, |v| v.word_forward(big))
            }
            Motion::WordBack { big } => self.repeat(n, |v| v.word_back(big)),
            Motion::WordEnd { big } => self.repeat(n, |v| v.word_end(big)),
            Motion::WordEndBack { big } => {
                self.repeat(n, |v| v.word_end_back(big))
            }
            Motion::High | Motion::Middle | Motion::Low => {
                self.window_motion(motion, n)
            }
            Motion::ScreenDown => self.repeat(n, |v| v.vertical(true)),
            Motion::ScreenUp => self.repeat(n, |v| v.vertical(false)),
            Motion::Line(line) => self.jump(line),
            Motion::Find(kind, c) => self.repeat_find(n, kind, c),
            Motion::FindAgain { reversed } => self.find_again(n, reversed),
            Motion::Percent => self.percent(),
            Motion::Mark { name, exact } => self.go_to_mark(name, exact),
            Motion::Lines | Motion::Object { .. } => Effect::Nothing,
            Motion::Search { forward } => self.search(forward, n),
            Motion::SearchAgain { reversed } => {
                self.search(self.again(reversed), n)
            }
            Motion::Star { forward } => self.star(forward, n),
        };
        if self.pending.running
            && matches!(motion, Motion::Left | Motion::Right)
        {
            self.bell = bell;
        }
        effect
    }

    /// Resolve against the viewport after preceding keys in this frame.
    fn window_motion(&mut self, motion: Motion, n: usize) -> Effect {
        self.scrolled(self.rows);
        let shown = self.rows.max(1).min(self.line_count() - self.top);
        let offset = n.max(1).min(shown) - 1;
        let line = match motion {
            Motion::High => self.top + offset,
            Motion::Low => self.top + shown - 1 - offset,
            _ => self.top + (shown - 1) / 2,
        };
        self.jump(line)
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
        self.set_previous();
        let start = self.line_at(line).unwrap_or(0);
        self.go_clamped(self.at_screen_column(start, self.want))
    }

    fn find_again(&mut self, n: usize, reversed: bool) -> Effect {
        let Some((kind, c)) = self.find else {
            self.ring();
            return Effect::Nothing;
        };
        let kind = if reversed { kind.reversed() } else { kind };
        self.repeat_find(n, kind, c)
    }

    fn repeat_find(&mut self, n: usize, kind: Find, c: char) -> Effect {
        self.repeat(n, |v| {
            v.found(kind, c).map_or(Effect::Nothing, |to| v.go(to))
        })
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
        let Some(to) = self.matching_bracket() else {
            self.ring();
            return Effect::Nothing;
        };
        self.set_previous();
        self.go(to)
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

    /// `m{a-zA-Z}`: the mark set to the cursor, `a`-`z` in this
    /// Buffer's own table, `A`-`Z` in `files`, keyed by this Buffer's
    /// source; nothing over a Buffer with none to key it by.
    pub(crate) fn set_mark(&mut self, name: char) -> Effect {
        let pos = self.pos_at(self.cursor);
        if name.is_ascii_lowercase() {
            self.marks.local.insert(name, pos);
        } else if let Some(source) = self.source {
            self.files.insert(name, (source, pos));
        }
        Effect::Moved
    }

    /// `'` set to the cursor's own position, before a jump moves it:
    /// `Line`, `Percent`, the three searches, `Mark`, and `jump_to`'s
    /// own arrival by a file mark. The departing place is kept in the
    /// jump list too.
    pub(crate) fn set_previous(&mut self) {
        let pos = self.pos_at(self.cursor);
        self.marks.local.insert('\'', pos);
        self.jumps.push(Jump { source: self.source, pos });
    }

    /// Where `name` points, and the source elsewhere than this
    /// Buffer's when it is a file mark that names one. `None` for a
    /// name not set: the tables only ever hold valid names, so
    /// nothing here filters `name` first. The quote and the backtick
    /// both read `'`, the one previous-position mark, valid from the
    /// start at line 0 even before anything has set it (nvim's own
    /// default there).
    pub(crate) fn found_mark(&self, name: char) -> Option<(Option<Rd>, Pos)> {
        let key = if name == '`' { '\'' } else { name };
        let default = (key == '\'').then(Pos::default);
        if let Some(pos) = self.marks.local.get(&key).copied().or(default) {
            return Some((None, self.visual_end(key, pos)));
        }
        let &(rd, pos) = self.files.get(&name)?;
        let elsewhere = (self.source != Some(rd)).then_some(rd);
        Some((elsewhere, pos))
    }

    /// `<` and `>` as a direct read (not `gv`'s, which reads
    /// `marks.local` itself): `V`'s own whole-line reading, column 0
    /// for `<` and MAXCOL for `>`, substituted in over the real
    /// columns `leave_visual` stores, the ones `gv` needs back to
    /// reselect the same columns, not the line's own bounds.
    fn visual_end(&self, key: char, pos: Pos) -> Pos {
        let lines = self.last_visual.is_some_and(|s| s.kind == Visual::Line);
        match (key, lines) {
            ('<', true) => Pos { column: 0, ..pos },
            ('>', true) => Pos { column: usize::MAX, ..pos },
            _ => pos,
        }
    }

    /// `'x` and `` `x ``: the mark's position, `a`-`z`, `[ ] < > .`
    /// local, `A`-`Z` a file's own, the quote and the backtick both
    /// reading the one previous-position mark. A file mark whose
    /// source is not this Buffer's asks the Client to arrive there
    /// instead, and does nothing under an operator, which cannot
    /// span a swap; `arrive` is not this door's to set, only the
    /// Plugin's, once the ask it ends in has gone out. Not set is
    /// E20.
    pub(crate) fn go_to_mark(&mut self, name: char, exact: bool) -> Effect {
        let Some((elsewhere, pos)) = self.found_mark(name) else {
            self.error("E20: Mark not set");
            return Effect::Nothing;
        };
        let land = if exact { land::EXACT } else { land::LINE };
        let Some(rd) = elsewhere else { return self.jump_to(pos, land) };
        if self.pending.running {
            return Effect::Nothing;
        }
        Effect::Goto(going(rd, pos, land))
    }

    /// `pos`, landed as `land` says (see [`crate::Effect::Goto`]).
    pub(crate) fn jump_to(&mut self, pos: Pos, land: u64) -> Effect {
        if land != land::WALKED {
            self.set_previous();
        }
        let at = self.byte_at(pos);
        let to = if land == land::LINE { self.first_non_blank(at) } else { at };
        self.go_clamped(to)
    }

    /// `<C-o>` (`back`) and `<Tab>`, `n` at a time: `Jumps::walk` does
    /// the walking; `None` moves nothing. An entry of another source
    /// asks the Client to arrive there (`Effect::Goto`, `WALKED`);
    /// one of this source lands here (`landed`). `'` is set
    /// to where the walk stood if it began at the newest end.
    pub(crate) fn jumped(&mut self, back: bool, n: usize) -> Effect {
        let here = Jump { source: self.source, pos: self.pos_at(self.cursor) };
        let newest = self.jumps.at == self.jumps.list.len();
        let Some(jump) = self.jumps.walk(back, n, here) else {
            self.ring();
            return Effect::Nothing;
        };
        if newest {
            self.marks.local.insert('\'', here.pos);
        }
        let effect =
            jump.source.filter(|_| jump.source != self.source).map_or_else(
                || self.landed(jump.pos),
                |source| Effect::Goto(going(source, jump.pos, land::WALKED)),
            );
        self.settle(&effect);
        effect
    }

    /// `jumped`'s own entry of this source: E19 for a line the Buffer
    /// no longer has, else landed as a walk lands.
    fn landed(&mut self, pos: Pos) -> Effect {
        if pos.line >= self.line_count() {
            self.error("E19: Mark has invalid line number");
            return Effect::Nothing;
        }
        self.jump_to(pos, land::WALKED)
    }

    /// The span `motion` covers from the cursor, `n` times, for an
    /// operator, after `:help exclusive`'s adjustments: an exclusive
    /// motion ending at a line's start ends at the end of the line
    /// before instead (rule 1), and from a line's first non-blank or
    /// earlier it takes whole lines (rule 2). `None` when a walk goes
    /// nowhere; a jump that lands where it started is a span of no
    /// width there (see `still_a_place`).
    fn span(&mut self, motion: Motion, n: usize) -> Option<Span> {
        if let Motion::Object { inner, kind } = motion {
            return self.object_span(inner, kind, n);
        }
        // The motion moves to find the span's end, then everything it
        // did to the cursor and the want is undone: the operator's
        // landing sets the want, as `w_set_curswant` has it.
        let (start, want, keep) = (self.cursor, self.want, self.keep_want);
        let mut kind = motion.extent(self.find);
        self.pending.running = true;
        let moved = match motion {
            Motion::Lines => self.lines_down(n - 1),
            Motion::WordForward { big } => self.words_for_operator(n, big),
            _ => self.move_by(motion, n),
        };
        self.pending.running = false;
        let end = self.cursor;
        (self.cursor, self.want, self.keep_want) = (start, want, keep);
        if moved == Effect::Nothing && !self.still_a_place(motion) {
            return None;
        }
        let (from, to) = if end >= start { (start, end) } else { (end, start) };
        let inclusive = kind == Extent::Inclusive;
        let mut to = if inclusive { self.forward_char(to) } else { to };
        let rule1 =
            kind == Extent::Exclusive && to > from && self.line_start(to) == to;
        if rule1 {
            to = self.prev_char(to);
        }
        if rule1 && from <= self.first_non_blank(from) && from != to {
            kind = Extent::Linewise;
        }
        Some((from, to.min(self.text.len()), kind))
    }

    /// Whether a motion that moved nothing still names the place the
    /// cursor stands on, which is a span of no width there rather
    /// than a failed motion. The jumps do: `G`, `H`, `M` and `L`
    /// always name a line, and a mark names one when it is set in
    /// this Buffer, so `d'a` on the mark's own line takes that line
    /// while `d'z` unset and `d'A` of another file's mark still fail.
    /// A walk (`w`, `f`, `j`) that goes nowhere is a failed motion.
    fn still_a_place(&self, motion: Motion) -> bool {
        match motion {
            Motion::Line(_) | Motion::High | Motion::Middle | Motion::Low => {
                true
            }
            Motion::Mark { name, .. } => {
                matches!(self.found_mark(name), Some((None, _)))
            }
            _ => false,
        }
    }

    /// `n` lines down for `dd` and the like: nvim's `cursor_down`,
    /// which fails only from the last line and otherwise stops at it,
    /// so `3dd` on the second of three lines takes the two that are
    /// left. `d3j`, counting a motion rather than lines, still fails
    /// past the end.
    fn lines_down(&mut self, n: usize) -> Effect {
        if n > 0 && self.position().0 + 1 >= self.line_count() {
            return Effect::Nothing;
        }
        // Each step of its own accord stops at the last line: the clamp.
        self.repeat(n, |v| v.vertical(true));
        Effect::Moved
    }

    /// `w` under an operator: the last of `n` stops at a line's end
    /// rather than crossing it, as Vim's does.
    fn words_for_operator(&mut self, n: usize, big: bool) -> Effect {
        let before = self.repeat(n - 1, |v| v.word_forward(big));
        let last = self.word_forward_op(big);
        moved(before == Effect::Moved || last == Effect::Moved)
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
        let as_end = matches!(motion, Motion::WordForward { .. })
            && op == Operator::Change
            && !self.on_blank();
        let span = if as_end {
            self.change_word_span(
                n,
                matches!(motion, Motion::WordForward { big: true }),
            )
        } else {
            self.span(motion, n)
        };
        let Some(mut span) = span else { return Effect::Nothing };
        // `gUU` and its kin start at the first non-blank, as nvim's
        // doubled case operators move there first; with a count the
        // move ends on a later line and the start keeps its column.
        let cased =
            matches!(op, Operator::Upper | Operator::Lower | Operator::Toggle);
        if motion == Motion::Lines && cased && n == 1 {
            span.0 = span.0.min(self.first_non_blank(span.0));
        }
        if op == Operator::Delete {
            span = self.as_lines(span);
        }
        let effect = self.apply(op, span, motion.one());
        // nvim's inner bracket over whole lines is a charwise span
        // that `:help exclusive` makes linewise, so its delete ends
        // on the first non-blank of the line left behind; `dd`, `dj`
        // and `dap`, linewise from the start, keep their column.
        if op == Operator::Delete
            && span.2 == Extent::Linewise
            && motion.inner_block()
        {
            self.cursor = self.first_non_blank(self.cursor);
        }
        effect
    }

    /// nvim's `op_delete`: a charwise delete over more than one line
    /// that starts in a line's indent and ends in blanks takes whole
    /// lines instead, Vi's own strangeness. A change, a yank and
    /// visual mode keep their chars.
    fn as_lines(&self, span: Span) -> Span {
        let (from, to, kind) = span;
        let one_line = self.line_start(from) == self.line_start(to);
        if kind == Extent::Linewise || one_line {
            return span;
        }
        let rest = &self.text[to..self.line_end(to)];
        let blank = rest.bytes().all(|b| b == b' ' || b == b'\t');
        if blank && from <= self.first_non_blank(from) {
            return (from, to, Extent::Linewise);
        }
        span
    }

    /// `op` over `span`.
    pub(crate) fn apply(
        &mut self,
        op: Operator,
        span: Span,
        one: bool,
    ) -> Effect {
        let (from, to, kind) = span;
        let linewise = kind == Extent::Linewise || op.linewise();
        if op.linewise() {
            return self.shifted(from, to, op == Operator::ShiftRight, 1);
        }
        let (at, len) = if linewise {
            self.whole_lines(from, to)
        } else {
            (from, to - from)
        };
        match op {
            Operator::Upper | Operator::Lower | Operator::Toggle => {
                return self.recased(op, from, (at, len), linewise);
            }
            _ => {}
        }
        let text = if linewise {
            &self.text[self.line_start(from)..self.line_end(to)]
        } else {
            &self.text[at..at + len]
        };
        let kind = if linewise { Kind::Line } else { Kind::Char };
        // nvim's `op_delete` returns before the register on an empty
        // region, so a delete or a change of one leaves what stands
        // there; a yank of one still empties the register.
        if len > 0 || linewise || op == Operator::Yank {
            let took = match op {
                Operator::Yank => Took::Yank,
                _ => Took::Delete { one },
            };
            self.store(Register { text: text.into(), kind }, took);
        } else {
            self.pending.register = None;
        }
        match op {
            Operator::Delete => self.deleted(at, len, linewise),
            Operator::Change if linewise => self.change_lines(from, to),
            Operator::Change => self.changed(at, len),
            _ => self.yanked(from, at, len),
        }
    }

    /// `>` and `<` over the lines `from` to `to`, `times` over: each
    /// non-empty line's indent moved by `shiftwidth` and written with
    /// tabs, as nvim's defaults have it; the cursor on the first line
    /// at the column wanted, as `nostartofline` leaves it, and that
    /// column the want from then on, as every operator sets
    /// `w_set_curswant`.
    pub(crate) fn shifted(
        &mut self,
        from: usize,
        to: usize,
        right: bool,
        times: usize,
    ) -> Effect {
        let start = self.line_start(from);
        let end = self.line_end(to);
        let block = self.text[start..end]
            .split('\n')
            .map(|line| {
                (0..times).fold(String::from(line), |l, _| shift(&l, right))
            })
            .collect::<Vec<_>>()
            .join("\n");
        let effect = self.replace(start, end - start, block);
        self.cursor = self.at_screen_column(start, self.want);
        self.clamp();
        effect
    }

    /// `gu`, `gU`, `g~` over `len` bytes at `at`, the cursor where the
    /// span began; on whole lines their newlines stay.
    fn recased(
        &mut self,
        op: Operator,
        from: usize,
        (at, len): (usize, usize),
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
        let effect = self.replace(at, len, cased);
        self.cursor = from;
        self.clamp();
        effect
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
        let effect = self.replace(at, len, cased);
        self.cursor = at + len;
        self.clamp();
        effect
    }

    /// After a yank: `[` and `]` set to its span, `]` `last_char_before`
    /// its own end; the cursor where the span began, so `yy` does not
    /// move and `yk` keeps its column on the line above.
    fn yanked(&mut self, from: usize, at: usize, len: usize) -> Effect {
        let close = self.last_char_before(at + len, at);
        self.marks.local.insert('[', self.pos_at(at));
        self.marks.local.insert(']', self.pos_at(close));
        self.go_clamped(from.min(self.bound()));
        Effect::Moved
    }

    /// After a delete: the cursor where the span was, or at the wanted
    /// column of the line left when lines went, as `nostartofline`
    /// has it; either way the column landed on is the want from then
    /// on, as every operator sets `w_set_curswant`.
    ///
    /// Chars taken down to nothing leave one empty line, as Vim's
    /// buffer always has a line: the `\n` nvim writes for it, where
    /// `dd` writes none, which is the line's end and not a line
    /// opened, so `edit` is not asked for the `\n` it would owe. An
    /// empty span takes nothing, yet the cursor still lands at it:
    /// `di"` within `""` goes to the close.
    fn deleted(&mut self, at: usize, len: usize, linewise: bool) -> Effect {
        let before = self.cursor;
        let whole = !linewise && at == 0 && len == self.text.len();
        let effect = if whole {
            let text = String::from("\n");
            self.text.clone_from(&text);
            self.joining = false;
            Effect::Edits(alloc::vec![edited(0, len, text, false)])
        } else {
            self.remove(at, len)
        };
        let start = at.min(self.bound());
        let line = self.line_start(start);
        self.cursor = if linewise {
            self.at_screen_column(line, self.want)
        } else {
            start
        };
        self.clamp();
        if effect == Effect::Nothing {
            return moved(self.cursor != before);
        }
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
    fn change_word_span(&mut self, n: usize, big: bool) -> Option<Span> {
        let next = self.forward_char(self.cursor);
        let at_end = next >= self.text.len()
            || self.class_changes(self.cursor, next, big);
        if n == 1 && at_end {
            return Some((
                self.cursor,
                next.min(self.text.len()),
                Extent::Exclusive,
            ));
        }
        self.span(Motion::WordEnd { big }, n)
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
        let effect = self.replace(start, end - start, indent.clone());
        self.cursor = start + indent.len();
        self.mode = Mode::Insert;
        self.indent = indent.len();
        effect
    }

    /// What an operator took, into the unnamed register and the one
    /// `"x` named: `a`-`z` hold, `A`-`Z` append, `_` drops, `+` and
    /// `*` go to the clipboard. The unnamed register is then the
    /// named one whole, as Vim's points at the register last written.
    pub(crate) fn store(&mut self, taken: Register, took: Took) {
        // Explicit unnamed writes use register0; unnamed reads still use
        // the last register's contents through fetch.
        let selected = match self.pending.register.take() {
            Some('"') => Some('0'),
            other => other,
        };
        if selected == Some('_') {
            return;
        }
        let named = self.store_named(selected, &taken);
        let whole = named.and_then(|name| self.registers.get(&name).cloned());
        self.store_history(&taken, took, selected.is_some());
        self.register = Some(whole.unwrap_or(taken));
    }

    /// A selected clipboard remains selected, though it has no map entry.
    fn store_named(
        &mut self,
        name: Option<char>,
        taken: &Register,
    ) -> Option<char> {
        let end = if taken.kind == Kind::Line { "\n" } else { "" };
        match name {
            Some('+' | '*') => {
                self.clipboard = Some(alloc::format!("{}{end}", taken.text));
                None
            }
            Some(name @ ('a'..='z' | '0'..='9' | '-')) => {
                self.registers.insert(name, taken.clone());
                Some(name)
            }
            Some(name @ 'A'..='Z') => {
                let name = name.to_ascii_lowercase();
                self.append(name, taken);
                Some(name)
            }
            _ => None,
        }
    }

    fn store_history(&mut self, taken: &Register, took: Took, named: bool) {
        if matches!(took, Took::Yank) && !named {
            self.registers.insert('0', taken.clone());
        }
        let Took::Delete { one } = took else { return };
        let small = taken.kind != Kind::Line && !taken.text.contains('\n');
        if !small || one {
            self.shift_history(taken);
        }
        if small && !named {
            self.registers.insert('-', taken.clone());
        }
    }

    fn shift_history(&mut self, taken: &Register) {
        for from in (b'1'..=b'8').rev() {
            self.shift_register(from);
        }
        self.registers.insert('1', taken.clone());
    }

    fn shift_register(&mut self, from: u8) {
        let to = char::from(from + 1);
        let held = self.registers.get(&char::from(from)).cloned();
        match held {
            Some(held) => {
                self.registers.insert(to, held);
            }
            None => {
                self.registers.remove(&to);
            }
        }
    }

    /// `taken` onto register `name`: lines on their own line, chars
    /// straight after.
    pub(crate) fn append(&mut self, name: char, taken: &Register) {
        let entry = self.registers.entry(name).or_default();
        let lines = entry.kind != Kind::Char || taken.kind != Kind::Char;
        if !entry.text.is_empty() && lines {
            entry.text.push('\n');
        }
        entry.text.push_str(&taken.text);
        entry.kind = match (entry.kind, taken.kind) {
            (Kind::Line, _) | (_, Kind::Line) => Kind::Line,
            (Kind::Block, _) | (_, Kind::Block) => Kind::Block,
            (Kind::Char, Kind::Char) => Kind::Char,
        };
    }

    /// The register a put reads: the one `"x` named, else the unnamed;
    /// `None` for the clipboard, which the tui must bring.
    pub(crate) fn fetch(&mut self) -> Option<Register> {
        match self.pending.register.take() {
            // `.` puts what the clipboard gave the last time.
            Some('+' | '*') if self.repeat.replaying => {
                Some(self.last_clip.clone())
            }
            Some('+' | '*') => None,
            Some('_') => Some(Register::default()),
            Some(name @ ('a'..='z' | 'A'..='Z' | '0'..='9' | '-')) => {
                let name = name.to_ascii_lowercase();
                Some(self.registers.get(&name).cloned().unwrap_or_default())
            }
            _ => Some(self.register.clone().unwrap_or_default()),
        }
    }

    /// `p` after the cursor or `P` before it, `n` times; a linewise
    /// register goes on lines of its own.
    pub(crate) fn paste(&mut self, after: bool, n: usize) -> Effect {
        let Some(register) = self.fetch() else {
            let continuation = self.continuing();
            self.put = Some(Put { after, count: n, over: None, continuation });
            return Effect::Clipboard(continuation);
        };
        self.paste_register(register, after, n)
    }

    pub(crate) fn paste_register(
        &mut self,
        register: Register,
        after: bool,
        n: usize,
    ) -> Effect {
        let Register { text, kind } = register;
        match kind {
            _ if text.is_empty() => Effect::Nothing,
            Kind::Line => self.paste_lines(after, n, &text),
            Kind::Block => self.paste_block(after, n, &text),
            Kind::Char => self.paste_chars(after, n, &text),
        }
    }

    pub(crate) fn paste_lines(
        &mut self,
        after: bool,
        n: usize,
        text: &str,
    ) -> Effect {
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
        let effect = self.replace(at, 0, put);
        let first = at + usize::from(after && last);
        self.cursor = self.first_non_blank(first);
        self.clamp();
        effect
    }

    fn paste_chars(&mut self, after: bool, n: usize, text: &str) -> Effect {
        let put = text.repeat(n);
        let at = if after { self.append_at() } else { self.cursor };
        let lines = put.contains('\n');
        let effect = self.replace(at, 0, put);
        let end = self.prev_char(at + effect_len(&effect));
        self.cursor = if lines { at } else { end };
        effect
    }

    /// A block register put: piece `i` onto the line `i` below the
    /// cursor's at its column (after it for `p`), a count repeating
    /// each piece; the cursor at the first line's column.
    pub(crate) fn paste_block(
        &mut self,
        after: bool,
        n: usize,
        text: &str,
    ) -> Effect {
        let pieces: Vec<String> =
            text.split('\n').map(|piece| piece.repeat(n)).collect();
        let width = pieces.iter().map(|p| p.chars().count()).max();
        let (line, column) = self.position();
        let on_char = self.cursor < self.line_end(self.cursor);
        let column = column + usize::from(after && on_char);
        let mut edits = Vec::new();
        for (i, piece) in pieces.iter().enumerate() {
            let (at, text) =
                self.block_piece(line + i, column, piece, width.unwrap_or(0));
            self.text.insert_str(at, &text);
            edits.push(edited(at, 0, text, i > 0));
        }
        let start = self.line_at(line).unwrap_or(0);
        self.cursor = self.at_column(start, column);
        self.joining = false;
        Effect::Edits(edits)
    }

    /// Where `piece` goes on line `line` at `column`, and as what:
    /// the line padded with spaces to reach the column, the piece
    /// padded to `width` when text follows it, a line past the end
    /// appended, as nvim does.
    fn block_piece(
        &self,
        line: usize,
        column: usize,
        piece: &str,
        width: usize,
    ) -> (usize, String) {
        let Some(start) = self.line_at(line) else {
            return self.appended(column, piece);
        };
        let end = self.line_end(start);
        let chars = self.text[start..end].chars().count();
        let at = self.at_column(start, column);
        let pad = " ".repeat(column.saturating_sub(chars));
        let fill = if at < end { width - piece.chars().count() } else { 0 };
        (at, alloc::format!("{pad}{piece}{}", " ".repeat(fill)))
    }

    /// A new last line for a block piece: `piece` at `column`, on its
    /// own line after the text, ended as the text is.
    fn appended(&self, column: usize, piece: &str) -> (usize, String) {
        let pad = " ".repeat(column);
        let ends = self.text.ends_with('\n');
        let (before, after) = if ends { ("", "\n") } else { ("\n", "") };
        let text = alloc::format!("{before}{pad}{piece}{after}");
        (self.text.len(), text)
    }

    /// `J`: `n` lines (at least two) made one, a space between unless
    /// the next starts with `)` or the line ends in a blank.
    ///
    /// One minimal edit per seam, its own `\n` and the next line's
    /// leading blanks replaced by the gap: no edit ever retypes a
    /// line's own content, so a mark within one is never taken by
    /// this, `moved`'s merge arm finding it afresh where its line
    /// now begins, `3J`'s middle lines among them.
    pub(crate) fn join(&mut self, n: usize) -> Effect {
        let line = self.position().0;
        let joins = (n.max(2) - 1).min(self.line_count() - 1 - line);
        if joins == 0 {
            return Effect::Nothing;
        }
        let start = self.line_start(self.cursor);
        let mut edits = Vec::new();
        let mut seam = start;
        for i in 0..joins {
            let at = self.line_end(start);
            let next = at + 1;
            let end = self.line_end(next);
            let piece = self.text[next..end].trim_start_matches([' ', '\t']);
            let blanks = (end - next) - piece.len();
            let text = String::from(gap(&self.text[start..at], piece));
            let len = 1 + blanks;
            self.text.replace_range(at..at + len, &text);
            seam = at;
            edits.push(edited(at, len, text, i > 0));
        }
        self.cursor = seam;
        self.clamp();
        self.joining = false;
        Effect::Edits(edits)
    }
}

/// The bytes an edit put in.
fn effect_len(effect: &Effect) -> usize {
    match effect {
        Effect::Edits(edits) => edits.iter().map(|e| e.text.len()).sum(),
        _ => 0,
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
    let mut out = whitespace(0, columns);
    out.push_str(rest);
    out
}

/// Whitespace from screen cell `from` to `to`: tabs while a stop
/// fits, spaces after, as `noexpandtab` writes it.
pub fn whitespace(from: usize, to: usize) -> String {
    let stops = (from / 8 * 8 + 8..=to).step_by(8).count();
    let mut out = "\t".repeat(stops);
    let reached = if stops == 0 { from } else { from / 8 * 8 + 8 * stops };
    out.push_str(&" ".repeat(to - reached));
    out
}

#[cfg(test)]
mod tests {
    use super::{Operator, gap, recase, shift, whitespace};

    #[test]
    fn shift_moves_an_indent_by_eight_cells_in_tabs() {
        assert_eq!(shift("ab", true), "\tab");
        assert_eq!(shift("  ab", true), "\t  ab");
        assert_eq!(shift("\t  ab", true), "\t\t  ab");
        assert_eq!(
            shift("     \tab", true),
            "\t\tab",
            "a tab reaches its stop"
        );
        assert_eq!(shift("", true), "", "an empty line stays");
        assert_eq!(shift("\t\t  ab", false), "\t  ab");
        assert_eq!(shift("   ab", false), "ab");
        assert_eq!(shift("ab", false), "ab");
    }

    #[test]
    fn whitespace_fills_cells_with_tabs_where_a_stop_fits() {
        assert_eq!(whitespace(0, 0), "");
        assert_eq!(whitespace(0, 8), "\t");
        assert_eq!(whitespace(1, 9), "\t ");
        assert_eq!(whitespace(1, 17), "\t\t ");
        assert_eq!(whitespace(3, 7), "    ");
        assert_eq!(whitespace(8, 8), "");
    }

    #[test]
    fn a_join_puts_one_space_unless_a_paren_or_a_blank_says_not() {
        assert_eq!(gap("ab", "cd"), " ");
        assert_eq!(gap("ab", ")"), "");
        assert_eq!(gap("ab ", "cd"), "");
        assert_eq!(gap("ab\t", "cd"), "");
        assert_eq!(gap("ab", ""), "");
    }

    #[test]
    fn recase_keeps_a_char_that_would_become_two() {
        assert_eq!(recase(Operator::Upper, 'a'), 'A');
        assert_eq!(recase(Operator::Lower, 'É'), 'é');
        assert_eq!(recase(Operator::Toggle, 'a'), 'A');
        assert_eq!(recase(Operator::Toggle, 'A'), 'a');
        assert_eq!(recase(Operator::Toggle, '1'), '1');
        assert_eq!(recase(Operator::Upper, 'ß'), 'ß', "SS would be two");
    }
}
