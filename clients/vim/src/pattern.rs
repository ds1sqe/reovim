//! The part of Vim's patterns a person types without thinking, matched
//! by hand: literals, `.`, `*`, `\+`, `\?`, `^`, `$`, `[...]`, `\<`,
//! `\>`, `\d \w \s` and their negations, `\c`, `\C`, `\(...\)`,
//! `\|`, `\n`; magic as nvim's default. Anything else is E486.

use alloc::string::String;
use alloc::vec::Vec;

/// One thing to match.
#[derive(Clone, Debug)]
enum Atom {
    Char(char),
    Any,
    /// Ranges, and whether the set is what to avoid.
    Class(Vec<(char, char)>, bool),
    /// `\d`, `\w`, `\s`, negated by the bool.
    Digit(bool),
    Word(bool),
    Space(bool),
    LineStart,
    LineEnd,
    WordStart,
    WordEnd,
    /// `\(...\)`: alternatives and the capture's number.
    Group(Vec<Vec<Piece>>, usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Quant {
    One,
    Star,
    Plus,
    Opt,
}

#[derive(Clone, Debug)]
struct Piece {
    atom: Atom,
    quant: Quant,
}

/// A parsed pattern.
#[derive(Clone, Debug)]
pub struct Pattern {
    alts: Vec<Vec<Piece>>,
    ignore_case: bool,
    groups: usize,
}

/// What a match captured: the whole, then `\1` to `\9`.
pub type Captures = Vec<Option<(usize, usize)>>;

struct Parser<'a> {
    rest: &'a str,
    groups: usize,
    /// `\c` or `\C` seen.
    case: Option<bool>,
}

impl Pattern {
    /// `source` parsed; `ignorecase` and `smartcase` as the options
    /// have them, `\c` and `\C` overriding.
    pub fn parse(
        source: &str,
        ignorecase: bool,
        smartcase: bool,
    ) -> Option<Self> {
        let mut parser = Parser { rest: source, groups: 0, case: None };
        let alts = parser.alternatives()?;
        if !parser.rest.is_empty() {
            return None;
        }
        let upper = source.chars().any(char::is_uppercase);
        let folded = parser.case.unwrap_or(ignorecase && !(smartcase && upper));
        Some(Self { alts, ignore_case: folded, groups: parser.groups })
    }

    /// The leftmost match starting at or after `from`.
    pub fn find(
        &self,
        text: &str,
        from: usize,
    ) -> Option<(usize, usize, Captures)> {
        let mut at = from;
        let mut found = self.match_at(text, at);
        while found.is_none() {
            let c = text[at..].chars().next()?;
            at += c.len_utf8();
            found = self.match_at(text, at);
        }
        let (end, caps) = found?;
        Some((at, end, caps))
    }

    /// A match starting exactly at `at`: its end and captures.
    pub fn match_at(&self, text: &str, at: usize) -> Option<(usize, Captures)> {
        let mut caps = alloc::vec![None; self.groups + 1];
        let end = self.alts.iter().find_map(|alt| {
            caps.iter_mut().skip(1).for_each(|c| *c = None);
            self.seq(text, alt, 0, at, &mut caps)
        })?;
        caps[0] = Some((at, end));
        Some((end, caps))
    }

    /// `pieces[i..]` matched at `at`, backtracking through quantifiers.
    fn seq(
        &self,
        text: &str,
        pieces: &[Piece],
        i: usize,
        at: usize,
        caps: &mut Captures,
    ) -> Option<usize> {
        let Some(piece) = pieces.get(i) else { return Some(at) };
        let (min, max) = match piece.quant {
            Quant::One => (1, 1),
            Quant::Star => (0, usize::MAX),
            Quant::Plus => (1, usize::MAX),
            Quant::Opt => (0, 1),
        };
        let ends = self.reach(text, &piece.atom, at, max, caps);
        if ends.len() <= min {
            return None;
        }
        ends.iter()
            .skip(min)
            .rev()
            .find_map(|&end| self.seq(text, pieces, i + 1, end, caps))
    }

    /// Every end `atom` reaches from `at`, greedily, up to `max`
    /// times; `at` itself first. A zero-width atom matches once.
    fn reach(
        &self,
        text: &str,
        atom: &Atom,
        at: usize,
        max: usize,
        caps: &mut Captures,
    ) -> Vec<usize> {
        let mut ends = alloc::vec![at];
        let mut here = at;
        let mut next = self.atom(text, atom, here, caps);
        while let Some(end) = next.filter(|_| ends.len() <= max) {
            ends.push(end);
            next = (end != here)
                .then(|| self.atom(text, atom, end, caps))
                .flatten();
            here = end;
        }
        ends
    }

    fn atom(
        &self,
        text: &str,
        atom: &Atom,
        at: usize,
        caps: &mut Captures,
    ) -> Option<usize> {
        let c = text[at..].chars().next();
        let step = |ok: bool| {
            (ok && c != Some('\n')).then(|| at + c.map_or(0, char::len_utf8))
        };
        let before = text[..at].chars().next_back();
        match atom {
            Atom::Char('\n') => (c == Some('\n')).then_some(at + 1),
            Atom::Char(want) => step(c.is_some_and(|c| self.same(c, *want))),
            Atom::Any => step(c.is_some()),
            Atom::Class(ranges, negated) => {
                step(c.is_some_and(|c| self.in_class(ranges, c) != *negated))
            }
            Atom::Digit(negated) => {
                step(c.is_some_and(|c| c.is_ascii_digit() != *negated))
            }
            Atom::Word(negated) => {
                step(c.is_some_and(|c| keyword(c) != *negated))
            }
            Atom::Space(negated) => {
                step(c.is_some_and(|c| (c == ' ' || c == '\t') != *negated))
            }
            Atom::LineStart => (at == 0 || before == Some('\n')).then_some(at),
            Atom::LineEnd => (c.is_none() || c == Some('\n')).then_some(at),
            Atom::WordStart => (c.is_some_and(keyword)
                && !before.is_some_and(keyword))
            .then_some(at),
            Atom::WordEnd => (before.is_some_and(keyword)
                && !c.is_some_and(keyword))
            .then_some(at),
            Atom::Group(alts, number) => {
                self.group(text, alts, *number, at, caps)
            }
        }
    }

    fn group(
        &self,
        text: &str,
        alts: &[Vec<Piece>],
        number: usize,
        at: usize,
        caps: &mut Captures,
    ) -> Option<usize> {
        let end =
            alts.iter().find_map(|alt| self.seq(text, alt, 0, at, caps))?;
        caps[number] = Some((at, end));
        Some(end)
    }

    fn same(&self, a: char, b: char) -> bool {
        a == b || (self.ignore_case && fold(a) == fold(b))
    }

    fn in_class(&self, ranges: &[(char, char)], c: char) -> bool {
        let hit = |c: char| ranges.iter().any(|(a, b)| (*a..=*b).contains(&c));
        hit(c) || (self.ignore_case && hit(fold(c)))
    }
}

fn keyword(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

impl Parser<'_> {
    fn alternatives(&mut self) -> Option<Vec<Vec<Piece>>> {
        let mut alts = alloc::vec![self.sequence()?];
        while let Some(rest) = self.rest.strip_prefix("\\|") {
            self.rest = rest;
            alts.push(self.sequence()?);
        }
        Some(alts)
    }

    fn at_end(&self) -> bool {
        self.rest.is_empty()
            || self.rest.starts_with("\\|")
            || self.rest.starts_with("\\)")
    }

    /// Pieces up to `\\|`, `\\)` or the end.
    fn sequence(&mut self) -> Option<Vec<Piece>> {
        let mut pieces = Vec::new();
        while !self.at_end() {
            let atom = self.atom(pieces.is_empty()).ok()?;
            pieces.extend(atom.map(|atom| Piece { atom, quant: self.quant() }));
        }
        Some(pieces)
    }

    fn take(&mut self) -> Option<char> {
        let c = self.rest.chars().next()?;
        self.rest = &self.rest[c.len_utf8()..];
        Some(c)
    }

    /// One atom, or `None` inside `Ok` for `\\c` and `\\C`, which are
    /// flags, not atoms; `Err` for what the subset does not have.
    fn atom(&mut self, first: bool) -> Result<Option<Atom>, ()> {
        let c = self.take().ok_or(())?;
        let atom = match c {
            '.' => Atom::Any,
            '^' if first => Atom::LineStart,
            '$' if self.at_end() => Atom::LineEnd,
            '[' => self.class().ok_or(())?,
            '\\' => return self.escaped(),
            c => Atom::Char(c),
        };
        Ok(Some(atom))
    }

    fn escaped(&mut self) -> Result<Option<Atom>, ()> {
        let c = self.take().ok_or(())?;
        let atom = match c {
            'd' => Atom::Digit(false),
            'D' => Atom::Digit(true),
            'w' => Atom::Word(false),
            'W' => Atom::Word(true),
            's' => Atom::Space(false),
            'S' => Atom::Space(true),
            '<' => Atom::WordStart,
            '>' => Atom::WordEnd,
            'n' => Atom::Char('\n'),
            't' => Atom::Char('\t'),
            'c' | 'C' => {
                self.case = Some(c == 'c');
                return Ok(None);
            }
            '(' => self.group().ok_or(())?,
            '+' | '?' | '|' | ')' | '{' | '=' | '@' | 'z' | '%' | '_' => {
                return Err(());
            }
            c => Atom::Char(c),
        };
        Ok(Some(atom))
    }

    fn group(&mut self) -> Option<Atom> {
        self.groups += 1;
        let number = self.groups;
        let alts = self.alternatives()?;
        self.rest = self.rest.strip_prefix("\\)")?;
        Some(Atom::Group(alts, number))
    }

    fn quant(&mut self) -> Quant {
        let (quant, len) = if self.rest.starts_with('*') {
            (Quant::Star, 1)
        } else if self.rest.starts_with("\\+") {
            (Quant::Plus, 2)
        } else if self.rest.starts_with("\\?") || self.rest.starts_with("\\=") {
            (Quant::Opt, 2)
        } else {
            (Quant::One, 0)
        };
        self.rest = &self.rest[len..];
        quant
    }

    /// `[...]` after its `[`.
    fn class(&mut self) -> Option<Atom> {
        let negated = self.rest.starts_with('^');
        self.rest = self.rest.strip_prefix('^').unwrap_or(self.rest);
        let mut ranges = Vec::new();
        let mut c = self.take()?;
        while c != ']' || ranges.is_empty() {
            let lo = self.unescaped(c)?;
            let hi = self.range_end().unwrap_or(lo);
            ranges.push((lo, hi));
            c = self.take()?;
        }
        Some(Atom::Class(ranges, negated))
    }

    /// `c`, or the char after it when it is a backslash.
    fn unescaped(&mut self, c: char) -> Option<char> {
        if c == '\\' { self.take() } else { Some(c) }
    }

    /// `-x` after a range's start, unless the `x` is the closing `]`.
    fn range_end(&mut self) -> Option<char> {
        let rest = self.rest.strip_prefix('-')?;
        let hi = rest.chars().next().filter(|hi| *hi != ']')?;
        self.rest = &rest[hi.len_utf8()..];
        Some(hi)
    }
}

/// `replacement` for a match with `caps` in `text`: `&` and `\0` the
/// whole, `\1`..`\9` the groups, `\r` a newline, `\&` and `\\` as
/// themselves.
pub fn expand(replacement: &str, text: &str, caps: &Captures) -> String {
    let mut out = String::new();
    let mut chars = replacement.chars();
    while let Some(c) = chars.next() {
        match c {
            '&' => out.push_str(capture(text, caps, 0)),
            '\\' => out.push_str(&escape(chars.next(), text, caps)),
            c => out.push(c),
        }
    }
    out
}

/// What `\x` in a replacement stands for.
fn escape(x: Option<char>, text: &str, caps: &Captures) -> String {
    match x {
        Some(d @ '0'..='9') => {
            capture(text, caps, d as usize - '0' as usize).into()
        }
        Some('r') => String::from("\n"),
        Some('n') => String::new(),
        Some('t') => String::from("\t"),
        Some(other) => String::from(other),
        None => String::from("\\"),
    }
}

fn capture<'a>(text: &'a str, caps: &Captures, n: usize) -> &'a str {
    caps.get(n).copied().flatten().map_or("", |(a, b)| &text[a..b])
}
