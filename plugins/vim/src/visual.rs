//! Visual modes: a selection the operators take as their span, drawn
//! before the operator comes. `v` by chars, `V` by lines, Ctrl-V by
//! a block of columns.

use alloc::string::String;
use alloc::vec::Vec;

use crate::operator::{Extent, Operator, Span, Took};
use crate::{Edit, Effect, Key, Kind, Mode, Motion, Put, Register, Vim};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Visual {
    Char,
    Line,
    Block,
}

/// A selection, kept for `gv`: its ends are the marks `<` and `>`,
/// which an edit since moves with the text, `backward` saying which
/// end was the cursor's, `want` the column it wanted there, as nvim's
/// `vi_curswant` keeps it.
#[derive(Clone, Copy, Debug)]
pub struct Selection {
    pub kind: Visual,
    pub want: usize,
    pub backward: bool,
}

/// An insert on the first line of a block, to be copied onto the
/// others at Esc: `I`, `A` and `c` in block mode.
#[derive(Clone, Copy, Debug)]
pub struct BlockInsert {
    first: usize,
    last: usize,
    /// The column the typing goes to on every line.
    column: usize,
    /// The block's left column, where the cursor ends.
    left: usize,
    to_end: bool,
    /// Where the typing began.
    from: usize,
}

impl Vim {
    /// `v`, `V` or Ctrl-V: into that mode, another kind switches, the
    /// same kind leaves. The want is kept through all three, as nvim's
    /// `nv_visual` never touches `curswant`; Esc alone sets it again.
    pub(crate) fn enter_visual(&mut self, kind: Visual) -> Effect {
        self.keep_want = true;
        match self.mode {
            Mode::Visual(current) if current == kind => self.leave_visual(),
            Mode::Visual(_) => {
                self.mode = Mode::Visual(kind);
                Effect::Moved
            }
            _ => {
                self.anchor = self.cursor;
                self.mode = Mode::Visual(kind);
                Effect::Moved
            }
        }
    }

    /// Back to normal mode, the selection kept for `gv`: `<` and `>`
    /// set to its real low and high ends, which then move with the
    /// text. `V`'s own whole-line reading (`<` at column 0, `>` at
    /// MAXCOL) is `found_mark`'s to give a direct read of them, not
    /// stored here: `gv` needs the real columns back, the ones typing
    /// or a motion left the selection at, not the line's own bounds.
    pub(crate) fn leave_visual(&mut self) -> Effect {
        if let Mode::Visual(kind) = self.mode {
            let backward = self.cursor < self.anchor;
            let (lo, hi) = self.ends();
            let (lo, hi) = (self.pos_at(lo), self.pos_at(hi));
            self.marks.local.insert('<', lo);
            self.marks.local.insert('>', hi);
            self.last_visual =
                Some(Selection { kind, want: self.want, backward });
        }
        self.mode = Mode::Normal;
        self.clamp();
        Effect::Moved
    }

    /// `gv`: the last selection again, its ends read from `<` and
    /// `>`, wherever an edit since has moved them, and its want back
    /// with them, as nvim's `nv_gv_cmd` restores `vi_curswant`: a `$`
    /// selection reaches every line's end again.
    pub(crate) fn reselect(&mut self) -> Effect {
        let Some(last) = self.last_visual else { return Effect::Nothing };
        let (Some(&lo), Some(&hi)) =
            (self.marks.local.get(&'<'), self.marks.local.get(&'>'))
        else {
            return Effect::Nothing;
        };
        let (lo, hi) = (self.byte_at(lo), self.byte_at(hi));
        let (anchor, cursor) = if last.backward { (hi, lo) } else { (lo, hi) };
        self.anchor = anchor.min(self.bound());
        self.cursor = cursor.min(self.bound());
        self.want = last.want;
        self.keep_want = true;
        self.mode = Mode::Visual(last.kind);
        Effect::Moved
    }

    /// One key in visual mode `kind`.
    pub(crate) fn visual(&mut self, kind: Visual, key: Key) -> Effect {
        if self.digit(key) {
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
                first @ ('g' | 'f' | 't' | 'F' | 'T' | 'i' | 'a' | 'r' | '"'
                | '\'' | '`'),
            ) => self.wait(first, count),
            Key::Esc => self.leave_visual(),
            Key::Char('v') => self.enter_visual(Visual::Char),
            Key::Char('V') => self.enter_visual(Visual::Line),
            Key::Ctrl('v') => self.enter_visual(Visual::Block),
            Key::Char('o') => {
                core::mem::swap(&mut self.anchor, &mut self.cursor);
                Effect::Moved
            }
            Key::Char('d' | 'x') => {
                self.visual_operate(kind, Operator::Delete, n)
            }
            Key::Char('c' | 's') => {
                self.visual_operate(kind, Operator::Change, n)
            }
            Key::Char('y') => self.visual_operate(kind, Operator::Yank, n),
            Key::Char('>') => {
                self.visual_operate(kind, Operator::ShiftRight, n)
            }
            Key::Char('<') => self.visual_operate(kind, Operator::ShiftLeft, n),
            Key::Char('u') => self.visual_operate(kind, Operator::Lower, n),
            Key::Char('U') => self.visual_operate(kind, Operator::Upper, n),
            Key::Char('~') => self.visual_operate(kind, Operator::Toggle, n),
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
        match (first, key) {
            ('r', Key::Char(c)) => return self.visual_replace(c),
            ('"', Key::Char(name)) => return self.named(name, count),
            ('i' | 'a', Key::Char(kind)) => {
                return self.extend(first == 'i', kind, n);
            }
            _ => {}
        }
        let motion = self.pair(first, key, count);
        motion.map_or(Effect::Nothing, |motion| self.visual_motion(motion, n))
    }

    /// A motion moves the cursor end of the selection; `$` takes the
    /// line's end itself, newline included.
    fn visual_motion(&mut self, motion: Motion, n: usize) -> Effect {
        // The same door `motion` opens with: a search that cannot be
        // answered leaves before the selection moves at all.
        if let Some(effect) = self.wanting(motion, n) {
            return effect;
        }
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
    fn extend(&mut self, inner: bool, kind: char, n: usize) -> Effect {
        let (lo, hi) = self.ends();
        let lines = self.mode == Mode::Visual(Visual::Line);
        // A selection already made grows by the object after it: the
        // line after its last when it holds whole lines, else the
        // char after its end.
        let at = match (lo == hi, lines) {
            (true, _) => self.cursor,
            (false, true) => self.line_end(hi) + 1,
            (false, false) => self.forward_char(hi),
        };
        let at = at.min(self.bound());
        let here = core::mem::replace(&mut self.cursor, at);
        let span = self.object_span(inner, kind, n);
        self.cursor = here;
        let Some((from, to, span_kind)) = span else { return Effect::Nothing };
        let last = if span_kind == Extent::Linewise {
            // Whole lines leave the cursor at the last line's start,
            // as nvim's paragraph does.
            self.line_start(to)
        } else {
            self.prev_char(to.max(from + 1))
        };
        if span_kind == Extent::Linewise {
            self.mode = Mode::Visual(Visual::Line);
        }
        self.anchor = if lo == hi { from } else { lo.min(from) };
        self.cursor = if lo == hi { last } else { hi.max(last) };
        Effect::Moved
    }

    /// The selection as an operator's span: chars through the cursor's
    /// (a newline included after `$`), or whole lines.
    ///
    /// The last line's end is `bound`, not the text's: nvim's buffer
    /// has no line break after its last line for `op_delete` or
    /// `op_yank` to take, the one the file ends with being
    /// `fixeol`'s, so a charwise selection reaching that line's NUL
    /// takes no newline where an inner line's does.
    fn selection(&self, kind: Visual) -> Span {
        let (lo, hi) = self.ends();
        match kind {
            // A `V` selection begins at its first line's start.
            Visual::Line => (self.line_start(lo), hi, Extent::Linewise),
            _ => {
                (lo, self.forward_char(hi).min(self.bound()), Extent::Exclusive)
            }
        }
    }

    /// Where an operator over the selection puts the cursor: the
    /// first position of the operated text, nvim's `oap->start`,
    /// which `do_pending_operator` moves to, `w_set_curswant` with
    /// it, before the operator itself runs. Column 0 for `V`, and
    /// for Ctrl-V the block's left column on its first line, which
    /// is not the anchor's own column when the cursor is left of it.
    pub(crate) fn selection_start(&self, kind: Visual) -> usize {
        if kind == Visual::Block {
            let ((top, _), (left, _)) = self.corners();
            return self.at_column(self.line_at(top).unwrap_or(0), left);
        }
        self.selection(kind).0
    }

    /// The byte range of the block's columns on each of its lines,
    /// top to bottom; on a line short of the block's left column it
    /// is empty, at the line's end.
    fn block_ranges(&self) -> Vec<(usize, usize)> {
        let ((top, bottom), (left, right)) = self.corners();
        (top..=bottom)
            .filter_map(|line| self.line_at(line))
            .map(|start| self.block_range(start, left, right))
            .collect()
    }

    fn block_range(
        &self,
        start: usize,
        left: usize,
        right: usize,
    ) -> (usize, usize) {
        let end = self.line_end(start);
        let from = self.at_column(start, left);
        let to =
            if self.to_end { end } else { self.at_column(start, right + 1) };
        (from, to)
    }

    /// An operator over the selection, `n` being its count, which
    /// only a shift uses: `3>` shifts three times.
    fn visual_operate(
        &mut self,
        kind: Visual,
        op: Operator,
        n: usize,
    ) -> Effect {
        if kind == Visual::Block {
            return self.block_operate(op, n);
        }
        let (from, to, extent) = self.selection(kind);
        self.leave_visual();
        match op {
            Operator::ShiftRight | Operator::ShiftLeft => {
                self.shifted(from, to, op == Operator::ShiftRight, n)
            }
            _ => self.apply(op, (from, to, extent), false),
        }
    }

    /// An operator over the block: one edit per line, all one
    /// revision, applied bottom-up so no offset moves under another.
    fn block_operate(&mut self, op: Operator, n: usize) -> Effect {
        let ranges = self.block_ranges();
        if ranges.is_empty() {
            return self.leave_visual();
        }
        let (lines, (left, _)) = self.corners();
        self.leave_visual();
        let top_left = self.at_column(self.line_at(lines.0).unwrap_or(0), left);
        match op {
            Operator::Yank => {
                let register = self.block_register(&ranges);
                self.store(register, Took::Yank);
                self.cursor = top_left;
                Effect::Moved
            }
            Operator::Delete | Operator::Change => {
                self.block_removed(op, &ranges, lines, left)
            }
            Operator::ShiftRight | Operator::ShiftLeft => {
                let right = op == Operator::ShiftRight;
                self.block_shifted(&ranges, left, (right, n))
            }
            _ => self.block_recased(op, &ranges, top_left),
        }
    }

    /// The selection's byte ends, low then high.
    fn ends(&self) -> (usize, usize) {
        (self.anchor.min(self.cursor), self.anchor.max(self.cursor))
    }

    /// The selection's first and last line, and its left and right
    /// column.
    fn corners(&self) -> ((usize, usize), (usize, usize)) {
        let (a, c) =
            (self.position_of(self.anchor), self.position_of(self.cursor));
        ((a.0.min(c.0), a.0.max(c.0)), (a.1.min(c.1), a.1.max(c.1)))
    }

    /// `>` and `<` on a block, `times` over: the whitespace at the
    /// block's `left` column rebuilt `shiftwidth` cells wider or
    /// narrower, as nvim does; a line short of the column stays.
    fn block_shifted(
        &mut self,
        ranges: &[(usize, usize)],
        left: usize,
        (right, times): (bool, usize),
    ) -> Effect {
        let reaching: Vec<usize> = ranges
            .iter()
            .map(|&(from, _)| from)
            .filter(|&from| self.position_of(from).1 >= left)
            .collect();
        let mut edits = Vec::new();
        for from in reaching.into_iter().rev() {
            let (a, b, text) = self.block_shift_at(from, right, 8 * times);
            self.text.replace_range(a..b, &text);
            let join = !edits.is_empty();
            edits.push(Edit { at: a, len: b - a, text, join });
        }
        let start = self.line_at(self.position_of(ranges[0].0).0).unwrap_or(0);
        self.cursor = self.at_column(start, left);
        self.clamp();
        self.joining = false;
        Effect::Edits(edits)
    }

    /// The whitespace to rebuild around the block's column `from`:
    /// a right shift takes the blanks before the column and grows
    /// them by `cells`, a left shift takes the blanks after it and
    /// shrinks them by up to `cells`; the range and its new text.
    fn block_shift_at(
        &self,
        from: usize,
        right: bool,
        cells: usize,
    ) -> (usize, usize, String) {
        let start = self.line_start(from);
        let blank = |c: char| c == ' ' || c == '\t';
        if right {
            let before = self.text[start..from].trim_end_matches(blank).len();
            let ws = start + before;
            let (a, b) = (self.cells(start, ws), self.cells(start, from));
            return (ws, from, crate::operator::whitespace(a, b + cells));
        }
        let run = self.text[from..].len()
            - self.text[from..].trim_start_matches(blank).len();
        let ws = from + run;
        let (a, b) = (self.cells(start, from), self.cells(start, ws));
        (from, ws, crate::operator::whitespace(a, b - (b - a).min(cells)))
    }

    fn block_register(&self, ranges: &[(usize, usize)]) -> Register {
        let pieces: Vec<&str> =
            ranges.iter().map(|&(a, b)| &self.text[a..b]).collect();
        Register { text: pieces.join("\n"), kind: Kind::Block }
    }

    /// The block taken out; `c` goes on to type onto every line.
    fn block_removed(
        &mut self,
        op: Operator,
        ranges: &[(usize, usize)],
        lines: (usize, usize),
        left: usize,
    ) -> Effect {
        let register = self.block_register(ranges);
        self.store(register, Took::Delete { one: false });
        let effect = self.block_edits(ranges, |_| String::new());
        let start = self.line_at(lines.0).unwrap_or(0);
        self.cursor = self.at_column(start, left);
        if op == Operator::Change {
            self.begin_block_insert(lines.0, lines.1, (left, left), false);
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
        // An empty range is a line short of the block: nothing to do.
        for &(from, to) in ranges.iter().rev().filter(|&&(a, b)| a < b) {
            let mut text = with(&self.text[from..to]);
            self.owed_eol(from, to - from, &mut text);
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
        let (lo, hi) = self.ends();
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
        let Some(register) = self.fetch() else {
            self.put = Some(Put { after: true, count: 1, over: Some(kind) });
            return Effect::Clipboard;
        };
        self.paste_over(kind, register)
    }

    /// `register` in place of the selection.
    pub(crate) fn paste_over(
        &mut self,
        kind: Visual,
        register: Register,
    ) -> Effect {
        if kind == Visual::Block {
            return self.paste_over_block(register);
        }
        let Register { text, kind: shape } = register;
        let (from, to, extent) = self.selection(kind);
        let lines = extent == Extent::Linewise;
        let (at, len) =
            if lines { self.whole_lines(from, to) } else { (from, to - from) };
        // A block register over chars goes in as its lines joined.
        let put = match (lines, shape == Kind::Line) {
            (true, _) => alloc::format!("{text}\n"),
            (false, true) => alloc::format!("\n{text}\n"),
            (false, false) => text,
        };
        let taken = self.text[at..at + len].trim_end_matches('\n').into();
        self.leave_visual();
        let below = put.starts_with('\n');
        let inline = shape == Kind::Char
            && !lines
            && !put.contains('\n')
            && !put.is_empty();
        let end = at + put.len();
        let effect = self.replace(at, len, put);
        let kind = if lines { Kind::Line } else { Kind::Char };
        self.store(Register { text: taken, kind }, Took::Delete { one: false });
        self.cursor =
            if inline { self.prev_char(end) } else { at + usize::from(below) };
        self.clamp();
        effect
    }

    /// `p` over a block: the block goes to the register; a charwise
    /// register's text onto every line of the block, a linewise one
    /// after the block's last line, a block where the block was.
    fn paste_over_block(&mut self, register: Register) -> Effect {
        let ranges = self.block_ranges();
        let (lines, (left, _)) = self.corners();
        self.leave_visual();
        let taken = self.block_register(&ranges);
        self.store(taken, Took::Delete { one: false });
        let Register { text, kind } = register;
        let mut edits = Vec::new();
        let start = self.line_at(lines.0).unwrap_or(0);
        if kind == Kind::Char {
            let effect = self.block_edits(&ranges, |_| text.clone());
            self.cursor = self.at_column(start, left);
            self.joining = false;
            return effect;
        }
        crate::collect(
            &mut edits,
            self.block_edits(&ranges, |_| String::new()),
        );
        let line = if kind == Kind::Line { lines.1 } else { lines.0 };
        self.cursor = self.at_column(self.line_at(line).unwrap_or(start), left);
        let put = if kind == Kind::Line {
            self.paste_lines(true, 1, &text)
        } else {
            self.paste_block(false, 1, &text)
        };
        crate::collect(&mut edits, put);
        for (i, edit) in edits.iter_mut().enumerate() {
            edit.join = i > 0;
        }
        Effect::Edits(edits)
    }

    /// `I` or `A` on a block: insert mode on the first line, the
    /// typing copied onto the others at Esc.
    fn block_insert(&mut self, append: bool) -> Effect {
        let ((top, bottom), (left, right)) = self.corners();
        let column = if append { right + 1 } else { left };
        let to_end = append && self.to_end;
        self.leave_visual();
        self.begin_block_insert(top, bottom, (column, left), to_end);
        Effect::Moved
    }

    /// Insert mode at `column` on line `first`, the block's `left`
    /// column being where the cursor ends.
    fn begin_block_insert(
        &mut self,
        first: usize,
        last: usize,
        (column, left): (usize, usize),
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
        self.block =
            Some(BlockInsert { first, last, column, left, to_end, from });
    }

    /// Esc after a block insert: what was typed on the first line,
    /// onto every other line of the block, as part of the revision;
    /// the cursor at the block's top-left, as nvim leaves it.
    pub(crate) fn finish_block(&mut self, block: BlockInsert) -> Effect {
        let typed = self
            .text
            .get(block.from..self.cursor)
            .map(String::from)
            .filter(|t| !t.is_empty() && !t.contains('\n'));
        let top = self.line_at(block.first).unwrap_or(0);
        self.cursor = self.at_column(top, block.left);
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

    /// What is selected, as byte spans of the text, `to` exclusive and
    /// one past a line's end where its newline belongs to the
    /// selection. Charwise and linewise are one span; a block is one
    /// per line, of the lines `top` and `height` show.
    pub(crate) fn selection_spans(
        &self,
        kind: Visual,
        top: usize,
        height: usize,
    ) -> Vec<(usize, usize)> {
        let (lo, hi) = self.ends();
        match kind {
            Visual::Char => {
                alloc::vec![(lo, self.forward_char(hi))]
            }
            Visual::Line => {
                alloc::vec![(self.line_start(lo), self.line_end(hi) + 1)]
            }
            Visual::Block => self.block_spans(top, height),
        }
    }

    /// A block's span on each line shown: its columns, or through the
    /// line's newline after `$`.
    fn block_spans(&self, top: usize, height: usize) -> Vec<(usize, usize)> {
        let ((first, last), (left, right)) = self.corners();
        let rows = first.max(top)
            ..=last.min(top.saturating_add(height.saturating_sub(1)));
        rows.filter_map(|line| self.line_at(line))
            .map(|start| {
                let (from, to) = self.block_range(start, left, right);
                (from, to + usize::from(self.to_end))
            })
            .collect()
    }
}

/// `r`'s char for one of the selection's: newlines stay.
const fn keep_newline(old: char, c: char) -> char {
    if old == '\n' { old } else { c }
}
