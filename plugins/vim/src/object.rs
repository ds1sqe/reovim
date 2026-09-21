//! Text objects: a span around the cursor without a motion. `iw aw
//! iW aW`, the bracket pairs, the quotes, `ip ap`.

use alloc::vec::Vec;

use crate::Vim;
use crate::operator::{Extent, Span};

/// Where one step forward landed, as nvim's `inc` tells the three
/// apart: on a char of the same line, on the line's end, which is a
/// place of its own, or on the line after.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Step {
    Char,
    End,
    Line,
}

/// The quotes before and after `col`, or the first pair on the line
/// when none is before it.
fn around(quotes: &[usize], col: usize) -> Option<(usize, usize)> {
    let Some(open) = quotes.iter().rev().find(|q| **q < col) else {
        return Some((*quotes.first()?, *quotes.get(1)?));
    };
    let close = quotes.iter().find(|q| **q > col)?;
    Some((*open, *close))
}

/// The `quote`s in `line`, which starts at `start` in the text; a
/// `\`, as `quoteescape` has it, makes the char after it text.
fn quotes(line: &str, start: usize, quote: char) -> Vec<usize> {
    let mut escaped = false;
    line.char_indices()
        .filter(|&(_, c)| {
            let found = !escaped && c == quote;
            escaped = !escaped && c == '\\';
            found
        })
        .map(|(i, _)| start + i)
        .collect()
}

impl Vim {
    /// The object `kind` around the cursor, `inner` or with what
    /// Vim adds around it, `n` of them.
    pub(crate) fn object_span(
        &self,
        inner: bool,
        kind: char,
        n: usize,
    ) -> Option<Span> {
        match kind {
            'w' | 'W' => self.word_object(inner, kind == 'W', n),
            '(' | ')' | 'b' => self.bracket_object(inner, b'(', b')', n),
            '[' | ']' => self.bracket_object(inner, b'[', b']', n),
            '{' | '}' | 'B' => self.bracket_object(inner, b'{', b'}', n),
            '<' | '>' => self.bracket_object(inner, b'<', b'>', n),
            '"' | '\'' | '`' => self.quote_object(inner, kind, n),
            'p' => self.paragraph_object(inner, n),
            _ => None,
        }
    }

    /// nvim's `inc`: one char on, a line's end being a place of its
    /// own; `None` past the last line.
    fn inc(&self, at: usize) -> Option<(usize, Step)> {
        if at >= self.text.len() {
            return None;
        }
        if self.text.as_bytes()[at] == b'\n' {
            return (at + 1 < self.text.len()).then_some((at + 1, Step::Line));
        }
        let next = self.forward_char(at);
        let step =
            if next == self.line_end(at) { Step::End } else { Step::Char };
        Some((next, step))
    }

    /// nvim's `incl`: `inc`, and over the end of a line that has
    /// chars on it, so a run of blanks holds one newline per line.
    fn incl(&self, at: usize) -> Option<usize> {
        match self.inc(at)? {
            (next, Step::End) => Some(self.inc(next)?.0),
            (next, _) => Some(next),
        }
    }

    /// nvim's `dec`: one char back, a line's end being a place of
    /// its own; `true` when it crossed onto the line before.
    fn dec(&self, at: usize) -> Option<(usize, bool)> {
        let start = self.line_start(at);
        if at > start {
            return Some((self.prev_char(at), false));
        }
        start.checked_sub(1).map(|end| (end, true))
    }

    /// nvim's `decl`: `dec`, and over the end of a line that has
    /// chars on it; `true` when it stopped on a line's end.
    fn decl(&self, at: usize) -> Option<(usize, bool)> {
        let (back, crossed) = self.dec(at)?;
        if crossed && back > self.line_start(back) {
            return self.dec(back);
        }
        Some((back, crossed))
    }

    /// nvim's `cls` is zero: a blank, which a line's end and the
    /// text's end are too.
    fn blank_at(&self, at: usize) -> bool {
        matches!(self.char_at(at), None | Some(' ' | '\t' | '\n'))
    }

    /// nvim's `skip_chars` forward: past the run of chars like the
    /// one at `at`; `None` at the text's end.
    fn past_class(&self, mut at: usize, big: bool) -> Option<usize> {
        let here = self.char_at(at);
        while crate::word::same_class(big, self.char_at(at), here) {
            at = self.inc(at)?.0;
        }
        Some(at)
    }

    /// Forward over blanks from `at`, stopping on an empty line,
    /// which is a word of its own; `None` at the text's end.
    fn past_blanks(&self, mut at: usize) -> Option<usize> {
        while self.blank_at(at) && !self.empty_line(at) {
            at = self.inc(at)?.0;
        }
        Some(at)
    }

    /// nvim's `back_in_line`: back to the first char of the run like
    /// the one at `at` on its line.
    fn back_in_line(&self, at: usize, big: bool) -> usize {
        let start = self.line_start(at);
        let here = self.char_at(at);
        let mut from = at;
        while from > start
            && crate::word::same_class(
                big,
                self.char_at(self.prev_char(from)),
                here,
            )
        {
            from = self.prev_char(from);
        }
        from
    }

    /// nvim's `end_word`: the last char of the word at `from`, or of
    /// the next word when `from` is already on a word's end. From a
    /// blank it crosses lines, an empty line ending it.
    fn end_word(&self, from: usize, big: bool) -> Option<usize> {
        let here = self.char_at(from);
        let at = self.inc(from)?.0;
        if !self.blank_at(from)
            && !crate::word::same_class(big, self.char_at(at), here)
        {
            return Some(from);
        }
        let at = self.past_blanks(at)?;
        if self.empty_line(at) {
            return Some(at);
        }
        Some(self.dec(self.past_class(at, big)?)?.0)
    }

    /// nvim's `fwd_word` with `eol`: the start of the next word, but
    /// no further than the end of the line it starts on. `false`
    /// when that end was the text's, which only the last count of an
    /// object may meet.
    fn fwd_word(&self, from: usize, big: bool) -> (usize, bool) {
        let end = self.line_end(from);
        let last_line = end + 1 >= self.text.len();
        let Some((mut at, step)) = self.inc(from) else {
            return (from, false);
        };
        if step != Step::Char {
            return (at, !last_line);
        }
        let here = self.char_at(from);
        let over_word = !self.blank_at(from);
        while at < end
            && over_word
            && crate::word::same_class(big, self.char_at(at), here)
        {
            at = self.forward_char(at);
        }
        while at < end && self.blank_at(at) {
            at = self.forward_char(at);
        }
        (at, true)
    }

    /// nvim's step back after `fwd_word`: onto the last blank, or
    /// onto the line before when the word starts a line.
    fn one_back(&self, at: usize) -> usize {
        if at == self.line_start(at) {
            return self.decl(at).map_or(at, |(back, _)| back);
        }
        self.prev_char(at)
    }

    /// One count more onto a word object, as nvim's `current_word`
    /// loop takes it: the word after, or the blanks after the word.
    /// The end, and whether the char there belongs to the object.
    fn grown(
        &self,
        at: usize,
        big: bool,
        include: bool,
        left: usize,
    ) -> Option<(usize, bool)> {
        let at = self.incl(at)?;
        if self.blank_at(at) == include {
            return Some((self.end_word(at, big)?, true));
        }
        let (to, ran) = self.fwd_word(at, big);
        if !ran && left > 1 {
            return None;
        }
        if to == self.line_start(to) {
            return Some((to, false));
        }
        Some((self.prev_char(to), true))
    }

    /// nvim's `include_white`: `aw` takes the blanks before it when
    /// it did not end on blanks of its own.
    fn takes_blanks_before(&self, at: usize, inclusive: bool) -> bool {
        !self.blank_at(at) || (!inclusive && at == self.line_start(at))
    }

    /// Back over the blanks before `start`, which stop at a line's
    /// end.
    fn blanks_before(&self, start: usize) -> usize {
        let mut start = start;
        let Some((mut back, false)) = self.decl(start) else {
            return start;
        };
        while self.blank_at(back) {
            start = back;
            match self.decl(back) {
                Some((next, false)) => back = next,
                _ => return start,
            }
        }
        start
    }

    /// nvim's `current_word`. `iw` is the word or the blanks under
    /// the cursor, `aw` the word with the blanks after it, or those
    /// before when none follow; on blanks `aw` is the blanks with
    /// the word after them, which may be lines away. Each further
    /// count takes a word or a run of blanks in turn.
    fn word_object(&self, inner: bool, big: bool, n: usize) -> Option<Span> {
        let include = !inner;
        let mut start = self.back_in_line(self.cursor, big);
        let take_before = include && !self.blank_at(start);
        let mut inclusive = true;
        let mut at = if self.blank_at(start) == include {
            self.end_word(start, big)?
        } else {
            self.one_back(self.fwd_word(start, big).0)
        };
        for left in (1..n).rev() {
            (at, inclusive) = self.grown(at, big, include, left)?;
        }
        if take_before && self.takes_blanks_before(at, inclusive) {
            start = self.blanks_before(start);
        }
        // A line's end is no char, so an object ending there is empty.
        let to = if inclusive {
            self.forward_char(at).min(self.line_end(at))
        } else {
            at
        };
        Some((start, to, Extent::Exclusive))
    }

    /// The `open`..`close` pair around the cursor, `n` levels out;
    /// with none around it, the next pair forward, at any distance
    /// and across lines, as nvim takes it.
    fn bracket_object(
        &self,
        inner: bool,
        open: u8,
        close: u8,
        n: usize,
    ) -> Option<Span> {
        // A `close` under the cursor pairs with its own opener, so
        // the search back starts before it.
        let back = match self.text.as_bytes().get(self.cursor) {
            Some(byte) if *byte == close => self.cursor.checked_sub(1),
            _ => Some(self.cursor),
        };
        let mut start = back
            .and_then(|at| self.opener_before(at, open, close))
            .or_else(|| self.opener_after(open))?;
        for _ in 1..n {
            let out = start.checked_sub(1)?;
            start = self.opener_before(out, open, close)?;
        }
        let end = self.closer_after(start, open, close)?;
        if !inner {
            return Some((start, end + 1, Extent::Exclusive));
        }
        Some(self.inner_block(start + 1, end))
    }

    /// The inside of the pair whose opener ends at `from` and whose
    /// closer is at `end`. A closer with nothing but blanks before
    /// it on its line ends the object at the end of the line above,
    /// so that newline stays; when the opener ends its line too, the
    /// lines between them are the object, and nothing between them
    /// is no object at all.
    fn inner_block(&self, from: usize, end: usize) -> Span {
        let line = self.line_start(end);
        let alone = self.first_non_blank(end) == end && line > from;
        let to = if alone { line - 1 } else { end };
        let own_lines = alone && self.text[from..].starts_with('\n');
        let from = from + usize::from(own_lines);
        if own_lines && from <= to {
            return (from, to, Extent::Linewise);
        }
        (from, to.max(from), Extent::Exclusive)
    }

    /// The `open` before `at` that no `close` between answers.
    fn opener_before(&self, at: usize, open: u8, close: u8) -> Option<usize> {
        let bytes = self.text.as_bytes();
        let mut at = at;
        let mut depth = 0usize;
        while *bytes.get(at)? != open || depth > 0 {
            depth += usize::from(bytes[at] == close);
            depth = depth.saturating_sub(usize::from(bytes[at] == open));
            at = at.checked_sub(1)?;
        }
        Some(at)
    }

    /// The first `open` at or after the cursor, however far.
    fn opener_after(&self, open: u8) -> Option<usize> {
        let rest = &self.text.as_bytes()[self.cursor.min(self.text.len())..];
        rest.iter().position(|byte| *byte == open).map(|at| self.cursor + at)
    }

    /// The `close` answering the `open` at `start`.
    fn closer_after(&self, start: usize, open: u8, close: u8) -> Option<usize> {
        let bytes = self.text.as_bytes();
        let mut at = start + 1;
        let mut depth = 1usize;
        while depth > 0 {
            let b = *bytes.get(at)?;
            depth += usize::from(b == open);
            depth -= usize::from(b == close);
            at += 1;
        }
        Some(at - 1)
    }

    /// The quoted text the cursor is in, else between the quotes
    /// before and after it, else the first pair after it.
    ///
    /// `i` with a count of its own takes the quotes too; `a` takes
    /// them and the blanks after them, or before when none.
    fn quote_object(&self, inner: bool, quote: char, n: usize) -> Option<Span> {
        let (start, end) =
            (self.line_start(self.cursor), self.line_end(self.cursor));
        let quotes = quotes(&self.text[start..end], start, quote);
        let col = self.cursor;
        let inside = quotes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|[a, b]| (*a, *b))
            .find(|(a, b)| *a <= col && col <= *b);
        let (open, close) = inside.or_else(|| around(&quotes, col))?;
        let (from, to) = match (inner, n) {
            (true, 1) => (open + 1, close),
            (true, _) => (open, close + 1),
            _ => self.with_blanks(start, end, open, close + 1),
        };
        Some((from, to, Extent::Exclusive))
    }

    /// `from..to` with the blanks after it on the line, or those
    /// before it when none follow.
    fn with_blanks(
        &self,
        start: usize,
        end: usize,
        from: usize,
        to: usize,
    ) -> (usize, usize) {
        let bytes = self.text.as_bytes();
        let blank = |i: usize| i < end && matches!(bytes[i], b' ' | b'\t');
        let mut after = to;
        while blank(after) {
            after += 1;
        }
        if after > to {
            return (from, after);
        }
        let mut before = from;
        while before > start && blank(before - 1) {
            before -= 1;
        }
        (before, to)
    }

    /// nvim's `linewhite`: the line holds nothing but blanks.
    fn blank_line(&self, line: usize) -> bool {
        self.line_at(line)
            .is_some_and(|s| self.first_non_blank(s) == self.line_end(s))
    }

    /// The first line of the run of `blank` lines `line` is in.
    fn run_start(&self, mut line: usize, blank: bool) -> usize {
        while line > 0 && self.blank_line(line - 1) == blank {
            line -= 1;
        }
        line
    }

    /// The last line of the run of `blank` lines `line` is in.
    fn run_end(&self, mut line: usize, blank: bool) -> usize {
        let count = self.line_count();
        while line + 1 < count && self.blank_line(line + 1) == blank {
            line += 1;
        }
        line
    }

    /// `ip`: the run of lines like the cursor's, blank or not; `ap`
    /// adds the blank lines after, or before when none follow. A
    /// count takes one run more for each `ip`, two for each `ap`.
    fn paragraph_object(&self, inner: bool, n: usize) -> Option<Span> {
        let here = self.position().0;
        let kind = self.blank_line(here);
        let first = self.run_start(here, kind);
        let last = self.run_end(here, kind);
        let runs = if inner { n } else { 2 * n };
        if let Some(end) = self.grown_runs(last, kind, runs) {
            return self.line_span(first, end);
        }
        // `ap` with nothing after it takes the run before instead,
        // or stands alone when there is none; a count that runs out
        // of lines takes nothing at all.
        if inner || n > 1 {
            return None;
        }
        let before = first.checked_sub(1);
        let first = before.map_or(first, |line| self.run_start(line, !kind));
        self.line_span(first, last)
    }

    /// The last line after `runs - 1` runs more, blank and not in
    /// turn, past the run ending at `last`; `None` when the lines
    /// run out before the runs do.
    fn grown_runs(
        &self,
        last: usize,
        kind: bool,
        runs: usize,
    ) -> Option<usize> {
        let (mut last, mut kind, mut left) = (last, kind, runs);
        while left > 1 && last + 1 < self.line_count() {
            kind = !kind;
            last = self.run_end(last + 1, kind);
            left -= 1;
        }
        (left == 1).then_some(last)
    }

    /// Lines `first` through `last` as a span: from the first's
    /// start to the last's end.
    fn line_span(&self, first: usize, last: usize) -> Option<Span> {
        let from = self.line_at(first)?;
        let to = self.line_end(self.line_at(last)?);
        Some((from, to, Extent::Linewise))
    }
}
