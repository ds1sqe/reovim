//! Text objects: a span around the cursor without a motion. `iw aw
//! iW aW`, the bracket pairs, the quotes, `ip ap`.

use alloc::vec::Vec;

use crate::Vim;
use crate::operator::{Extent, Span};

/// A word for `iw` and `aw`: same class; for `iW` and `aW`: not blank.
fn same(big: bool, a: Option<char>, b: Option<char>) -> bool {
    let blank = |c: Option<char>| matches!(c, None | Some(' ' | '\t' | '\n'));
    if big { blank(a) == blank(b) } else { crate::word::same_class(a, b) }
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
    /// Vim adds around it.
    pub(crate) fn object_span(
        &self,
        inner: bool,
        kind: char,
        n: usize,
    ) -> Option<Span> {
        match kind {
            'w' | 'W' => self.word_object(inner, kind == 'W'),
            '(' | ')' | 'b' => self.bracket_object(inner, b'(', b')'),
            '[' | ']' => self.bracket_object(inner, b'[', b']'),
            '{' | '}' | 'B' => self.bracket_object(inner, b'{', b'}'),
            '<' | '>' => self.bracket_object(inner, b'<', b'>'),
            '"' | '\'' | '`' => self.quote_object(inner, kind, n),
            'p' => self.paragraph_object(inner),
            _ => None,
        }
    }

    /// The run of chars like the one at `at` on its line: `(from, to)`.
    fn word_run(&self, at: usize, big: bool) -> (usize, usize) {
        let (start, end) = (self.line_start(at), self.line_end(at));
        let here = self.char_at(at);
        let mut from = at;
        while from > start
            && same(big, self.char_at(self.prev_char(from)), here)
        {
            from = self.prev_char(from);
        }
        let mut to = at;
        while to < end && same(big, self.char_at(to), here) {
            to = self.forward_char(to);
        }
        (from, to)
    }

    /// `iw`: the word or the blanks under the cursor. `aw`: the word
    /// with the blanks after it, or before it when none follow; on
    /// blanks, the blanks with the word after them.
    fn word_object(&self, inner: bool, big: bool) -> Option<Span> {
        let end = self.line_end(self.cursor);
        if self.cursor >= end {
            return None;
        }
        let blank = |c| matches!(c, Some(' ' | '\t'));
        let (mut from, mut to) = self.word_run(self.cursor, big);
        let on_blank = blank(self.char_at(self.cursor));
        if inner {
            return Some((from, to, Extent::Exclusive));
        }
        let before = self.prev_char(from);
        if on_blank || blank(self.char_at(to)) {
            to = self.word_run(to, big).1.min(end);
        } else if from > self.line_start(from) && blank(self.char_at(before)) {
            from = self.word_run(before, big).0;
        }
        Some((from, to, Extent::Exclusive))
    }

    /// The innermost `open`..`close` pair around the cursor.
    fn bracket_object(&self, inner: bool, open: u8, close: u8) -> Option<Span> {
        let start = self.opener_before(open, close)?;
        let end = self.closer_after(start, open, close)?;
        if !inner {
            return Some((start, end + 1, Extent::Exclusive));
        }
        // Brackets on lines of their own hold whole lines.
        let after_open = start + 1;
        let own_lines = self.text[after_open..].starts_with('\n')
            && self.first_non_blank(end) == end
            && self.line_start(end) > after_open + 1;
        if own_lines {
            return Some((
                after_open + 1,
                self.line_start(end) - 1,
                Extent::Linewise,
            ));
        }
        Some((after_open, end, Extent::Exclusive))
    }

    /// The `open` at or before the cursor that no `close` between
    /// answers; a `close` under the cursor pairs with its own.
    fn opener_before(&self, open: u8, close: u8) -> Option<usize> {
        let bytes = self.text.as_bytes();
        let mut at = self.cursor;
        let mut depth = 0usize;
        while *bytes.get(at)? != open || depth > 0 {
            depth += usize::from(bytes[at] == close && at != self.cursor);
            depth = depth.saturating_sub(usize::from(bytes[at] == open));
            at = at.checked_sub(1)?;
        }
        Some(at)
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

    fn blank_line(&self, line: usize) -> bool {
        self.line_at(line).is_some_and(|s| self.line_end(s) == s)
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
    /// adds the blank lines after, or before when none follow.
    fn paragraph_object(&self, inner: bool) -> Option<Span> {
        let here = self.position().0;
        let kind = self.blank_line(here);
        let mut first = self.run_start(here, kind);
        let mut last = self.run_end(here, kind);
        let follows = last + 1 < self.line_count();
        if !inner && follows {
            last = self.run_end(last + 1, !kind);
        } else if !inner && first > 0 {
            first = self.run_start(first - 1, !kind);
        }
        let from = self.line_at(first)?;
        let to = self.line_at(last)?;
        Some((from, to, Extent::Linewise))
    }
}
