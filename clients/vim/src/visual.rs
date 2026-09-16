//! Visual modes: a selection the operators take as their span, drawn
//! before the operator comes. `v` by chars, `V` by lines, Ctrl-V by
//! a block of columns.

use alloc::string::String;
use alloc::vec::Vec;

use crate::operator::{Find, Kind, Operator, Span};
use crate::{Edit, Effect, Key, Mode, Motion, Register, Vim};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Visual {
    Char,
    Line,
    Block,
}

/// A selection, kept for `gv`.
#[derive(Clone, Copy, Debug)]
pub struct Selection {
    pub kind: Visual,
    pub anchor: usize,
    pub cursor: usize,
    pub to_end: bool,
}

/// An insert on the first line of a block, to be copied onto the
/// others at Esc: `I`, `A` and `c` in block mode.
#[derive(Clone, Copy, Debug)]
pub struct BlockInsert {
    first: usize,
    last: usize,
    column: usize,
    to_end: bool,
    /// Where the typing began.
    from: usize,
}

impl Vim {
    /// `v`, `V` or Ctrl-V: into that mode, another kind switches, the
    /// same kind leaves.
    pub(crate) fn enter_visual(&mut self, kind: Visual) -> Effect {
        match self.mode {
            Mode::Visual(current) if current == kind => self.leave_visual(),
            Mode::Visual(_) => {
                self.mode = Mode::Visual(kind);
                Effect::Moved
            }
            _ => {
                self.anchor = self.cursor;
                self.to_end = false;
                self.mode = Mode::Visual(kind);
                Effect::Moved
            }
        }
    }

    /// Back to normal mode, the selection kept for `gv`.
    pub(crate) fn leave_visual(&mut self) -> Effect {
        if let Mode::Visual(kind) = self.mode {
            let (anchor, cursor, to_end) =
                (self.anchor, self.cursor, self.to_end);
            self.last_visual = Some(Selection { kind, anchor, cursor, to_end });
        }
        self.mode = Mode::Normal;
        self.clamp();
        Effect::Moved
    }

    /// `gv`: the last selection again.
    pub(crate) fn reselect(&mut self) -> Effect {
        let Some(last) = self.last_visual else { return Effect::Nothing };
        self.anchor = last.anchor.min(self.bound());
        self.cursor = last.cursor.min(self.bound());
        self.to_end = last.to_end;
        self.mode = Mode::Visual(last.kind);
        Effect::Moved
    }

    /// One key in visual mode `kind`.
    pub(crate) fn visual(&mut self, kind: Visual, key: Key) -> Effect {
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
            return self.visual_second(first, key, count);
        }
        if let Some(motion) = Motion::of(key, count) {
            return self.visual_motion(motion, n);
        }
        match key {
            Key::Char(
                first @ ('g' | 'f' | 't' | 'F' | 'T' | 'i' | 'a' | 'r'),
            ) => self.wait(first, count),
            Key::Esc => self.leave_visual(),
            Key::Char('v') => self.enter_visual(Visual::Char),
            Key::Char('V') => self.enter_visual(Visual::Line),
            Key::Ctrl('v') => self.enter_visual(Visual::Block),
            Key::Char('o') => {
                core::mem::swap(&mut self.anchor, &mut self.cursor);
                Effect::Moved
            }
            Key::Char('d' | 'x') => self.visual_operate(kind, Operator::Delete),
            Key::Char('c' | 's') => self.visual_operate(kind, Operator::Change),
            Key::Char('y') => self.visual_operate(kind, Operator::Yank),
            Key::Char('>') => self.visual_operate(kind, Operator::ShiftRight),
            Key::Char('<') => self.visual_operate(kind, Operator::ShiftLeft),
            Key::Char('u') => self.visual_operate(kind, Operator::Lower),
            Key::Char('U') => self.visual_operate(kind, Operator::Upper),
            Key::Char('~') => self.visual_operate(kind, Operator::Toggle),
            Key::Char('J') => self.visual_join(),
            Key::Char('p' | 'P') => self.visual_paste(kind),
            Key::Char('I') if kind == Visual::Block => self.block_insert(false),
            Key::Char('A') if kind == Visual::Block => self.block_insert(true),
            Key::Char(c @ ('/' | '?' | ':')) => self.prompt(c, count),
            _ => Effect::Nothing,
        }
    }

    fn visual_second(&mut self, first: char, key: Key, count: usize) -> Effect {
        let n = count.max(1);
        let motion = match (first, key) {
            ('r', Key::Char(c)) => return self.visual_replace(c),
            ('g', Key::Char('g')) => Motion::Line(count.saturating_sub(1)),
            ('f', Key::Char(c)) => Motion::Find(Find::Forward, c),
            ('t', Key::Char(c)) => Motion::Find(Find::ForwardTo, c),
            ('F', Key::Char(c)) => Motion::Find(Find::Backward, c),
            ('T', Key::Char(c)) => Motion::Find(Find::BackwardTo, c),
            ('i' | 'a', Key::Char(kind)) => {
                return self.extend(first == 'i', kind);
            }
            _ => return Effect::Nothing,
        };
        if let Motion::Find(kind, c) = motion {
            self.find = Some((kind, c));
        }
        self.visual_motion(motion, n)
    }

    /// A motion moves the cursor end of the selection; `$` takes the
    /// line's end itself, newline included.
    fn visual_motion(&mut self, motion: Motion, n: usize) -> Effect {
        if motion == Motion::LineEnd {
            self.repeat(n - 1, |v| v.vertical(true));
            let effect = self.go(self.line_end(self.cursor));
            self.want = usize::MAX;
            self.keep_want = true;
            return effect;
        }
        self.move_by(motion, n)
    }

    /// `iw`, `ap` and the like: the object under the cursor joins the
    /// selection; a paragraph makes it linewise.
    fn extend(&mut self, inner: bool, kind: char) -> Effect {
        let (lo, hi) =
            (self.anchor.min(self.cursor), self.anchor.max(self.cursor));
        // A selection already made grows by the object after it.
        let at = if lo == hi { self.cursor } else { self.forward_char(hi) };
        let at = at.min(self.bound());
        let here = core::mem::replace(&mut self.cursor, at);
        let span = self.object_span(inner, kind);
        self.cursor = here;
        let Some((from, to, span_kind)) = span else { return Effect::Nothing };
        let last = if span_kind == Kind::Linewise {
            to
        } else {
            self.prev_char(to.max(from + 1))
        };
        if span_kind == Kind::Linewise {
            self.mode = Mode::Visual(Visual::Line);
        }
        self.anchor = if lo == hi { from } else { lo.min(from) };
        self.cursor = if lo == hi { last } else { hi.max(last) };
        Effect::Moved
    }

    /// The selection as an operator's span: chars through the cursor's
    /// (a newline included after `$`), or whole lines.
    fn selection(&self, kind: Visual) -> Span {
        let (lo, hi) =
            (self.anchor.min(self.cursor), self.anchor.max(self.cursor));
        match kind {
            Visual::Line => (lo, hi, Kind::Linewise),
            _ => (
                lo,
                self.forward_char(hi).min(self.text.len()),
                Kind::Exclusive,
            ),
        }
    }

    /// The byte range of the block's columns on each of its lines,
    /// top to bottom; a line too short for the block's left column
    /// has none.
    fn block_ranges(&self) -> Vec<(usize, usize)> {
        let (a, c) =
            (self.position_of(self.anchor), self.position_of(self.cursor));
        let (top, bottom) = (a.0.min(c.0), a.0.max(c.0));
        let (left, right) = (a.1.min(c.1), a.1.max(c.1));
        (top..=bottom)
            .filter_map(|line| self.line_at(line))
            .filter_map(|start| self.block_range(start, left, right))
            .collect()
    }

    fn block_range(
        &self,
        start: usize,
        left: usize,
        right: usize,
    ) -> Option<(usize, usize)> {
        let end = self.line_end(start);
        let chars = self.text[start..end].chars().count();
        if chars <= left {
            return None;
        }
        let from = self.at_column(start, left);
        let to =
            if self.to_end { end } else { self.at_column(start, right + 1) };
        Some((from, to))
    }

    /// The line and column of `at`, in chars from 0.
    pub(crate) fn position_of(&self, at: usize) -> (usize, usize) {
        let before = &self.text[..at];
        let line = before.matches('\n').count();
        let column = before.rsplit('\n').next().unwrap_or("").chars().count();
        (line, column)
    }

    fn visual_operate(&mut self, kind: Visual, op: Operator) -> Effect {
        if kind == Visual::Block {
            return self.block_operate(op);
        }
        let span = self.selection(kind);
        self.leave_visual();
        self.apply(op, span)
    }

    /// An operator over the block: one edit per line, all one
    /// revision, applied bottom-up so no offset moves under another.
    fn block_operate(&mut self, op: Operator) -> Effect {
        let ranges = self.block_ranges();
        let Some(&(top_left, _)) = ranges.first() else {
            return self.leave_visual();
        };
        let (first, last) =
            (self.position_of(self.anchor).0, self.position_of(self.cursor).0);
        let lines = (first.min(last), first.max(last));
        self.leave_visual();
        match op {
            Operator::Yank => {
                self.register = self.block_register(&ranges);
                self.cursor = top_left;
                Effect::Moved
            }
            Operator::Delete | Operator::Change => {
                self.block_removed(op, &ranges, lines, top_left)
            }
            Operator::ShiftRight | Operator::ShiftLeft => {
                let (from, to) = (ranges[0].0, ranges[ranges.len() - 1].0);
                self.apply(op, (from, to, Kind::Linewise))
            }
            _ => self.block_recased(op, &ranges, top_left),
        }
    }

    fn block_register(&self, ranges: &[(usize, usize)]) -> Register {
        let pieces: Vec<&str> =
            ranges.iter().map(|&(a, b)| &self.text[a..b]).collect();
        Register { text: pieces.join("\n"), linewise: false }
    }

    /// The block taken out; `c` goes on to type onto every line.
    fn block_removed(
        &mut self,
        op: Operator,
        ranges: &[(usize, usize)],
        lines: (usize, usize),
        top_left: usize,
    ) -> Effect {
        self.register = self.block_register(ranges);
        let column = self.position_of(top_left).1;
        let effect = self.block_edits(ranges, |_| String::new());
        self.cursor = top_left;
        if op == Operator::Change {
            self.begin_block_insert(lines.0, lines.1, column, false);
        } else {
            self.clamp();
        }
        effect
    }

    fn block_recased(
        &mut self,
        op: Operator,
        ranges: &[(usize, usize)],
        top_left: usize,
    ) -> Effect {
        let cased = |s: &str| {
            s.chars().map(|c| crate::operator::recase(op, c)).collect()
        };
        let effect = self.block_edits(ranges, cased);
        self.cursor = top_left;
        effect
    }

    /// `ranges` replaced by `with` of their text, bottom-up, as one
    /// revision.
    fn block_edits(
        &mut self,
        ranges: &[(usize, usize)],
        with: impl Fn(&str) -> String,
    ) -> Effect {
        let mut edits = Vec::new();
        for &(from, to) in ranges.iter().rev() {
            let text = with(&self.text[from..to]);
            self.text.replace_range(from..to, &text);
            edits.push(Edit {
                at: from,
                len: to - from,
                text,
                join: !edits.is_empty(),
            });
        }
        self.joining = true;
        Effect::Edits(edits)
    }

    /// `r` over the selection: every char but a newline becomes `c`.
    fn visual_replace(&mut self, c: char) -> Effect {
        let Mode::Visual(kind) = self.mode else { return Effect::Nothing };
        let replaced = move |s: &str| -> String {
            s.chars().map(|old| keep_newline(old, c)).collect()
        };
        let ranges = match kind {
            Visual::Block => self.block_ranges(),
            Visual::Line => {
                let (lo, hi, _) = self.selection(kind);
                alloc::vec![(self.line_start(lo), self.line_end(hi))]
            }
            Visual::Char => {
                let (lo, hi, _) = self.selection(kind);
                alloc::vec![(lo, hi)]
            }
        };
        let start = ranges.first().map_or(self.cursor, |r| r.0);
        self.leave_visual();
        let effect = self.block_edits(&ranges, replaced);
        self.cursor = start;
        self.clamp();
        self.joining = false;
        effect
    }

    /// `J` over the selected lines.
    fn visual_join(&mut self) -> Effect {
        let (lo, hi) =
            (self.anchor.min(self.cursor), self.anchor.max(self.cursor));
        let lines = self.position_of(hi).0 - self.position_of(lo).0 + 1;
        self.leave_visual();
        self.cursor = lo;
        let effect = self.join(lines.max(2));
        self.settle(&effect);
        effect
    }

    /// `p` over a selection: the selection goes, the register's text
    /// comes, on lines of its own when it is linewise.
    fn visual_paste(&mut self, kind: Visual) -> Effect {
        let Register { text, linewise } = self.register.clone();
        let (from, to, span_kind) = self.selection(kind);
        let (at, len) = if span_kind == Kind::Linewise {
            self.whole_lines(from, to)
        } else {
            (from, to - from)
        };
        let put = match (span_kind == Kind::Linewise, linewise) {
            (true, _) => alloc::format!("{text}\n"),
            (false, true) => alloc::format!("\n{text}\n"),
            (false, false) => text,
        };
        let taken = self.text[at..at + len].trim_end_matches('\n').into();
        self.leave_visual();
        self.text.replace_range(at..at + len, &put);
        self.register =
            Register { text: taken, linewise: span_kind == Kind::Linewise };
        self.cursor = at + usize::from(put.starts_with('\n'));
        self.clamp();
        self.joining = false;
        self.edit(at, len, put)
    }

    /// `I` or `A` on a block: insert mode on the first line, the
    /// typing copied onto the others at Esc.
    fn block_insert(&mut self, append: bool) -> Effect {
        let (a, c) =
            (self.position_of(self.anchor), self.position_of(self.cursor));
        let (top, bottom) = (a.0.min(c.0), a.0.max(c.0));
        let column = if append { a.1.max(c.1) + 1 } else { a.1.min(c.1) };
        let to_end = append && self.to_end;
        self.leave_visual();
        self.begin_block_insert(top, bottom, column, to_end);
        Effect::Moved
    }

    fn begin_block_insert(
        &mut self,
        first: usize,
        last: usize,
        column: usize,
        to_end: bool,
    ) {
        let start = self.line_at(first).unwrap_or(0);
        let from = if to_end {
            self.line_end(start)
        } else {
            self.at_column(start, column)
        };
        self.cursor = from;
        self.mode = Mode::Insert;
        self.indent = 0;
        self.block = Some(BlockInsert { first, last, column, to_end, from });
    }

    /// Esc after a block insert: what was typed on the first line,
    /// onto every other line of the block, as part of the revision.
    pub(crate) fn finish_block(&mut self, block: BlockInsert) -> Effect {
        let typed = self
            .text
            .get(block.from..self.cursor)
            .map(String::from)
            .filter(|t| !t.is_empty() && !t.contains('\n'));
        let Some(typed) = typed else { return Effect::Moved };
        let columns: Vec<usize> = (block.first + 1..=block.last)
            .rev()
            .filter_map(|line| self.block_column(line, block))
            .collect();
        let mut edits = Vec::new();
        for at in columns {
            self.text.insert_str(at, &typed);
            edits.push(Edit { at, len: 0, text: typed.clone(), join: true });
        }
        self.cursor = block.from;
        if edits.is_empty() { Effect::Moved } else { Effect::Edits(edits) }
    }

    /// Where the block's column falls on `line`, if the line reaches
    /// it.
    fn block_column(&self, line: usize, block: BlockInsert) -> Option<usize> {
        let start = self.line_at(line)?;
        let end = self.line_end(start);
        if block.to_end {
            return Some(end);
        }
        let chars = self.text[start..end].chars().count();
        (chars >= block.column).then(|| self.at_column(start, block.column))
    }

    /// The selected cells of the rows shown: `(row, from, to)` in
    /// columns, `to` exclusive and one past a line's end for the
    /// newline cell.
    pub(crate) fn selection_cells(
        &self,
        kind: Visual,
        top: usize,
        height: usize,
    ) -> Vec<(u16, u16, u16)> {
        let (a, c) =
            (self.position_of(self.anchor), self.position_of(self.cursor));
        let (first, last) = (a.0.min(c.0), a.0.max(c.0));
        let (left, right) = (a.1.min(c.1), a.1.max(c.1));
        let rows = first.max(top)..=last.min(top + height - 1);
        let cell = |n: usize| u16::try_from(n).unwrap_or(u16::MAX);
        let mut cells = Vec::new();
        for (line, start) in
            rows.filter_map(|l| self.line_at(l).map(|s| (l, s)))
        {
            let len = self.text[start..self.line_end(start)].chars().count();
            let (from, to) = match kind {
                Visual::Line => (0, len + 1),
                Visual::Block if self.to_end => (left, len + 1),
                Visual::Block => (left, right + 1),
                Visual::Char => self.char_cells(line, (first, last), len),
            };
            cells.push((cell(line - top), cell(from), cell(to)));
        }
        cells
    }

    /// A charwise selection's columns on `line`: from its start on
    /// the first line, to its end on the last, whole lines between.
    fn char_cells(
        &self,
        line: usize,
        lines: (usize, usize),
        len: usize,
    ) -> (usize, usize) {
        let (lo, hi) =
            (self.anchor.min(self.cursor), self.anchor.max(self.cursor));
        let from = if line == lines.0 { self.position_of(lo).1 } else { 0 };
        let to =
            if line == lines.1 { self.position_of(hi).1 + 1 } else { len + 1 };
        (from, to)
    }
}

/// `r`'s char for one of the selection's: newlines stay.
const fn keep_newline(old: char, c: char) -> char {
    if old == '\n' { old } else { c }
}
