//! `w`, `b` and `e` as Vim's search.c has them: three classes of
//! char, a line's end counting as a blank, an empty line as a word.

use crate::{Effect, Vim};

/// Blank, keyword or other; a line's end and the text's end are blank.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Class {
    Blank,
    Keyword,
    Other,
}

fn class(c: Option<char>) -> Class {
    match c {
        None | Some(' ' | '\t' | '\n') => Class::Blank,
        Some(c) if c.is_alphanumeric() || c == '_' || c as u32 > 191 => {
            Class::Keyword
        }
        Some(_) => Class::Other,
    }
}

impl Vim {
    fn class_at(&self, at: usize) -> Class {
        class(self.text[at.min(self.text.len())..].chars().next())
    }

    /// The next char boundary, or the end of the text.
    fn forward(&self, at: usize) -> usize {
        let c = self.text[at..].chars().next();
        c.map_or(at, |c| at + c.len_utf8())
    }

    /// `at` is an empty line: on a `\n` that starts its line.
    fn empty_line(&self, at: usize) -> bool {
        self.text[at..].starts_with('\n') && self.line_start(at) == at
    }

    /// Forward over chars of `class` from `at`; blanks stop at an
    /// empty line, since that is a word.
    fn skip(&self, mut at: usize, class: Class) -> usize {
        let word = |at| class == Class::Blank && self.empty_line(at);
        let len = self.text.len();
        while at < len && self.class_at(at) == class && !word(at) {
            at = self.forward(at);
        }
        at
    }

    /// Forward over blanks, empty lines included.
    fn skip_blanks(&self, mut at: usize) -> usize {
        while at < self.text.len() && self.class_at(at) == Class::Blank {
            at = self.forward(at);
        }
        at
    }

    /// Backward over chars of `class` before `at`.
    fn skip_back(&self, mut at: usize, class: Class) -> usize {
        while at > 0 && self.class_at(self.prev_char(at)) == class {
            at = self.prev_char(at);
        }
        at
    }

    /// Backward over blanks from `at`, stopping on an empty line or
    /// at the start.
    fn back_over_blanks(&self, mut at: usize) -> usize {
        let blank = |at| self.class_at(at) == Class::Blank;
        while at > 0 && blank(at) && !self.empty_line(at) {
            at = self.prev_char(at);
        }
        at
    }

    /// `w`: the start of the next word, an empty line counting as one.
    pub(crate) fn word_forward(&mut self) -> Effect {
        if self.cursor >= self.text.len() {
            return Effect::Nothing;
        }
        let start = self.class_at(self.cursor);
        let mut at = self.forward(self.cursor);
        if start != Class::Blank {
            at = self.skip(at, start);
        }
        self.go_clamped(self.skip(at, Class::Blank))
    }

    /// `e`: the end of the word, or of the next one when on an end.
    pub(crate) fn word_end(&mut self) -> Effect {
        if self.cursor >= self.text.len() {
            return Effect::Nothing;
        }
        let start = self.class_at(self.cursor);
        let mut at = self.forward(self.cursor);
        if start == Class::Blank || self.class_at(at) != start {
            at = self.skip_blanks(at);
        }
        at = self.skip(at, self.class_at(at));
        self.go_clamped(self.prev_char(at))
    }

    /// `b`: the start of the word before, an empty line counting as
    /// one.
    pub(crate) fn word_back(&mut self) -> Effect {
        if self.cursor == 0 {
            return Effect::Nothing;
        }
        let start = self.class_at(self.cursor);
        let mut at = self.prev_char(self.cursor);
        if start == Class::Blank || self.class_at(at) != start {
            at = self.back_over_blanks(at);
        }
        let word = self.class_at(at);
        if word == Class::Blank {
            return self.go(at);
        }
        self.go(self.skip_back(at, word))
    }
}
