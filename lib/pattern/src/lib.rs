#![no_std]
#![forbid(unsafe_code)]
//! Patterns in Vim's dialect, matched by hand: literals, `.`, `*`,
//! `\+`, `\?`, `^`, `$`, `[...]`, `\<`, `\>`, `\d \w \s` and their
//! negations, `\c`, `\C`, `\(...\)`, `\|`, `\n`; magic as nvim's
//! default. `\v \V \m \M` are refused. Anything else parses to
//! nothing, and the refusal is the caller's to word. Nesting deeper
//! than a pattern written by hand ever does is refused with it, and
//! a search that would cost more than the budget below answers
//! [`Spent`] rather than no match: the matcher did not find out, and
//! says so.

extern crate alloc;

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

/// A search that cost more than this matcher will spend: the pattern
/// is one it will not answer for on this text, which is neither a
/// match nor the absence of one. Saying so is the caller's.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Spent;

/// What [`Pattern::scan`] answered.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Scan {
    /// A match, `caps[0]` the whole of it.
    Match(usize, usize, Captures),
    /// No match starting at or before `upto`.
    None,
    /// The budget ran out before an attempt at `at` was made.
    Paused(usize),
}

/// What one attempt at one offset carries: the captures placed so
/// far, the steps it has spent, and how many groups it stands inside.
struct Attempt {
    caps: Captures,
    steps: usize,
    depth: usize,
}

/// The rest of the pattern, behind the piece being matched. A piece
/// that can match several ways is given it, so that it gives back
/// what only the rest rejects; the end of the whole match is what it
/// answers.
type Cont<'a> = &'a dyn Fn(usize, &mut Attempt) -> Option<usize>;

/// How deep `\(` may nest, which is how deep the parser recurses. A
/// pattern written by hand nests a handful deep at most; one that
/// nests further is one the parser would run the stack out on, and is
/// refused as anything else outside the subset is.
const DEPTH: usize = 32;

/// How many groups one attempt may stand inside at once, which is how
/// deep the matcher recurses: every `\(...\)`, and every turn of one
/// under a quantifier, is matched with the rest of the pattern behind
/// it and so stands on the stack while that rest is tried.
const LOOPS: usize = 256;

/// What one attempt may spend, counted in the two things that can
/// multiply: a piece tried at a place the rest of the pattern gave
/// back to, and a turn of a group. Walking a plain atom along the
/// text is not counted, so `.*x` costs two steps on a line of any
/// length, while `\(a*\)*b` spends this on a line of thirty `a`s and
/// answers [`Spent`].
const STEPS: usize = 400_000;

struct Parser<'a> {
    rest: &'a str,
    groups: usize,
    /// How many `\(` are open, which [`DEPTH`] bounds.
    depth: usize,
    /// `\c` or `\C` seen.
    case: Option<bool>,
}

impl Attempt {
    /// One step of the budget, `None` once [`STEPS`] are gone.
    fn spend(&mut self) -> Option<()> {
        self.steps += 1;
        (self.steps <= STEPS).then_some(())
    }

    /// Whether the attempt ran out before it found out.
    const fn spent(&self) -> bool {
        self.steps > STEPS
    }

    /// What an attempt with no match to show answers: no match, or
    /// that it never found out.
    const fn missed<T>(&self) -> Result<Option<T>, Spent> {
        if self.spent() { Err(Spent) } else { Ok(None) }
    }

    /// One group deeper, `None` at [`LOOPS`]. An attempt that
    /// recursed past that would run the stack out, so it spends the
    /// whole budget and answers as a spent one does.
    const fn deeper(&mut self) -> Option<()> {
        if self.depth == LOOPS {
            self.steps = STEPS + 1;
            return None;
        }
        self.depth += 1;
        Some(())
    }
}

/// A turn of a group that took `span`, matched with `rest` behind
/// it: the capture stands while `rest` runs and is given back to what
/// it held when `rest` refuses the match, so a turn that was rejected
/// leaves nothing of itself behind. A turn inside it that stood keeps
/// what it captured, which is the last turn's, as Vim keeps it.
fn captured(
    number: usize,
    span: (usize, usize),
    attempt: &mut Attempt,
    rest: Cont,
) -> Option<usize> {
    let held = attempt.caps[number];
    attempt.caps[number] = Some(span);
    let found = rest(span.1, attempt);
    if found.is_none() {
        attempt.caps[number] = held;
    }
    found
}

/// The rest of the pattern tried at `end`, one step of the budget
/// spent on the try. Every place a piece gives back to is one of
/// these, and giving back is what can multiply.
fn given(attempt: &mut Attempt, end: usize, rest: Cont) -> Option<usize> {
    attempt.spend()?;
    rest(end, attempt)
}

/// How many turns are behind one that began at `at` and ended at
/// `end`: a turn that matched nothing is the last one there is, so it
/// counts as all `max` of them.
const fn counted(taken: usize, max: usize, at: usize, end: usize) -> usize {
    if end == at { max } else { taken + 1 }
}

/// How few and how many times a piece may match.
const fn bounds(quant: Quant) -> (usize, usize) {
    match quant {
        Quant::One => (1, 1),
        Quant::Star => (0, usize::MAX),
        Quant::Plus => (1, usize::MAX),
        Quant::Opt => (0, 1),
    }
}

/// `at`, past its char; unmoved at the text's end, a char's length
/// there being 0.
fn onward(text: &str, at: usize) -> usize {
    at + text[at..].chars().next().map_or(0, char::len_utf8)
}

/// One attempt at `at`, on `budget`'s account: its match, or `None`
/// on a miss that was not spent.
fn attempted(
    pattern: &Pattern,
    text: &str,
    at: usize,
    budget: &mut usize,
) -> Result<Option<(usize, usize, Captures)>, Spent> {
    let mut attempt = pattern.attempt();
    let end = pattern.matched(text, at, &mut attempt);
    *budget = budget.saturating_sub(attempt.steps.max(1));
    let Some(end) = end else {
        return if attempt.spent() { Err(Spent) } else { Ok(None) };
    };
    attempt.caps[0] = Some((at, end));
    Ok(Some((at, end, attempt.caps)))
}

/// `from` walked to `upto` inclusive, on `budget`'s account, one
/// attempt at a time: a match or the budget running out settles it;
/// a miss goes on to the next char, `onward` alone deciding where
/// that is, so the loop's own condition is what keeps an attempt from
/// ever starting past `upto`, whatever its char boundaries.
///
/// # Errors
///
/// [`Spent`] when an attempt ran its own budget out before it found
/// out whether it matches.
fn walk(
    pattern: &Pattern,
    text: &str,
    from: usize,
    upto: usize,
    budget: &mut usize,
) -> Result<Scan, Spent> {
    let upto = upto.min(text.len());
    let mut at = from;
    while at <= upto {
        if *budget == 0 {
            return Ok(Scan::Paused(at));
        }
        if let Some((start, end, caps)) = attempted(pattern, text, at, budget)?
        {
            return Ok(Scan::Match(start, end, caps));
        }
        let next = onward(text, at);
        if next == at {
            break;
        }
        at = next;
    }
    Ok(Scan::None)
}

impl Pattern {
    /// `source` parsed; `ignorecase` and `smartcase` as the options
    /// have them, `\c` and `\C` overriding.
    pub fn parse(
        source: &str,
        ignorecase: bool,
        smartcase: bool,
    ) -> Option<Self> {
        let mut parser =
            Parser { rest: source, groups: 0, depth: 0, case: None };
        let alts = parser.alternatives()?;
        if !parser.rest.is_empty() {
            return None;
        }
        let upper = source.chars().any(char::is_uppercase);
        let folded = parser.case.unwrap_or(ignorecase && !(smartcase && upper));
        Some(Self { alts, ignore_case: folded, groups: parser.groups })
    }

    /// The leftmost match starting at or after `from`, or `Ok(None)`
    /// when there is none; [`Spent`] when the pattern cost more than
    /// a search will spend, which is not the same answer.
    ///
    /// # Errors
    ///
    /// [`Spent`] once one offset has run the budget out: it ran out
    /// on the pattern and not on that offset, and no offset after it
    /// would fare any better.
    pub fn find(
        &self,
        text: &str,
        from: usize,
    ) -> Result<Option<(usize, usize, Captures)>, Spent> {
        let mut at = from;
        loop {
            let mut budget = usize::MAX;
            at = match self.scan(text, at, text.len(), &mut budget)? {
                Scan::Match(at, end, caps) => return Ok(Some((at, end, caps))),
                Scan::None => return Ok(None),
                Scan::Paused(resume) => resume,
            };
        }
    }

    /// One offset at a time from `from` to `upto`, on `budget`'s
    /// account, stopping to say where it got to rather than walking
    /// past its bound.
    ///
    /// `upto` is the last offset a match may start at, inclusive
    /// since an empty match or a line's `$` may stand exactly there.
    ///
    /// [`Scan::Paused`] promises nothing was tried at `at`: a budget
    /// of 1 or more always walks at least one offset, so a chain of
    /// calls from each `Paused` ends. One call may overshoot its
    /// budget by one attempt.
    ///
    /// # Errors
    ///
    /// [`Spent`] when an attempt ran its own budget out before it
    /// found out whether it matches.
    pub fn scan(
        &self,
        text: &str,
        from: usize,
        upto: usize,
        budget: &mut usize,
    ) -> Result<Scan, Spent> {
        walk(self, text, from, upto, budget)
    }

    /// A match starting exactly at `at`: its end and captures.
    ///
    /// # Errors
    ///
    /// [`Spent`] when the attempt ran the budget out before it found
    /// out whether there is a match here.
    pub fn match_at(
        &self,
        text: &str,
        at: usize,
    ) -> Result<Option<(usize, Captures)>, Spent> {
        let mut attempt = self.attempt();
        let Some(end) = self.matched(text, at, &mut attempt) else {
            return attempt.missed();
        };
        attempt.caps[0] = Some((at, end));
        Ok(Some((end, attempt.caps)))
    }

    /// One attempt's room: a capture for the whole and one per group,
    /// and nothing of the budget spent yet.
    fn attempt(&self) -> Attempt {
        let caps = alloc::vec![None; self.groups + 1];
        Attempt { caps, steps: 0, depth: 0 }
    }

    /// The end of a match starting exactly at `at`, the alternatives
    /// tried in turn on `attempt`'s budget.
    fn matched(
        &self,
        text: &str,
        at: usize,
        attempt: &mut Attempt,
    ) -> Option<usize> {
        let whole = |end: usize, _: &mut Attempt| Some(end);
        self.alts.iter().find_map(|alt| {
            attempt.caps.iter_mut().skip(1).for_each(|c| *c = None);
            self.seq(text, alt, at, attempt, &whole)
        })
    }

    /// `pieces` matched at `at`, and then `rest`.
    fn seq(
        &self,
        text: &str,
        pieces: &[Piece],
        at: usize,
        attempt: &mut Attempt,
        rest: Cont,
    ) -> Option<usize> {
        let Some((piece, tail)) = pieces.split_first() else {
            return rest(at, attempt);
        };
        let onward = |end: usize, attempt: &mut Attempt| -> Option<usize> {
            self.seq(text, tail, end, attempt, rest)
        };
        self.piece(text, piece, at, attempt, &onward)
    }

    /// One `piece` at `at`, its quantifier greedy, and then `rest`.
    /// A group is matched with `rest` behind it, the rest of the
    /// pattern being what says which alternative and which length of
    /// it the group must give back to.
    ///
    /// The budget is spent here, one step for the piece and one for
    /// every further place the rest is tried at. Walking the atom
    /// along the text in [`Pattern::reach`] is not a step: it is
    /// linear in the line and cannot multiply.
    fn piece(
        &self,
        text: &str,
        piece: &Piece,
        at: usize,
        attempt: &mut Attempt,
        rest: Cont,
    ) -> Option<usize> {
        attempt.spend()?;
        if matches!(piece.atom, Atom::Group(..)) {
            return self.repeated(text, piece, at, 0, attempt, rest);
        }
        let (min, max) = bounds(piece.quant);
        let ends = self.reach(text, &piece.atom, at, max);
        if ends.len() <= min {
            return None;
        }
        ends.iter().skip(min).rev().find_map(|&end| given(attempt, end, rest))
    }

    /// A `\(...\)` under its quantifier with `taken` turns behind it:
    /// one more, greedily, and then `rest`. A turn that matched
    /// nothing is the last one, which is what ends a group that
    /// matches the empty string under `*`.
    fn repeated(
        &self,
        text: &str,
        piece: &Piece,
        at: usize,
        taken: usize,
        attempt: &mut Attempt,
        rest: Cont,
    ) -> Option<usize> {
        let Atom::Group(alts, number) = &piece.atom else { return None };
        let (min, max) = bounds(piece.quant);
        attempt.spend()?;
        let again = |end: usize, attempt: &mut Attempt| -> Option<usize> {
            let taken = counted(taken, max, at, end);
            self.repeated(text, piece, end, taken, attempt, rest)
        };
        let longer = (taken < max)
            .then(|| self.group(text, alts, *number, at, attempt, &again))
            .flatten();
        longer.or_else(|| {
            (taken >= min).then(|| given(attempt, at, rest)).flatten()
        })
    }

    /// One turn of a `\(...\)` at `at`: the alternatives tried in
    /// order, each with `rest` behind it.
    fn group(
        &self,
        text: &str,
        alts: &[Vec<Piece>],
        number: usize,
        at: usize,
        attempt: &mut Attempt,
        rest: Cont,
    ) -> Option<usize> {
        attempt.deeper()?;
        let turn = |end: usize, attempt: &mut Attempt| -> Option<usize> {
            captured(number, (at, end), attempt, rest)
        };
        let end =
            alts.iter().find_map(|alt| self.seq(text, alt, at, attempt, &turn));
        attempt.depth -= 1;
        end
    }

    /// Every end `atom` reaches from `at`, greedily, up to `max`
    /// times; `at` itself first. A zero-width atom matches once.
    fn reach(
        &self,
        text: &str,
        atom: &Atom,
        at: usize,
        max: usize,
    ) -> Vec<usize> {
        let mut ends = alloc::vec![at];
        let mut here = at;
        let mut next = self.one(text, atom, here);
        while let Some(end) = next.filter(|_| ends.len() <= max) {
            ends.push(end);
            next = (end != here).then(|| self.one(text, atom, end)).flatten();
            here = end;
        }
        ends
    }

    /// One atom that holds no pattern of its own, matched at `at`.
    fn one(&self, text: &str, atom: &Atom, at: usize) -> Option<usize> {
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
            Atom::Word(negated) => step(c.is_some_and(|c| word(c) != *negated)),
            Atom::Space(negated) => {
                step(c.is_some_and(|c| (c == ' ' || c == '\t') != *negated))
            }
            Atom::LineStart => (at == 0 || before == Some('\n')).then_some(at),
            Atom::LineEnd => (c.is_none() || c == Some('\n')).then_some(at),
            Atom::WordStart => {
                let here = c.map_or(0, class);
                (here > 1 && before.map_or(0, class) != here).then_some(at)
            }
            Atom::WordEnd => {
                let last = before.map_or(0, class);
                (last > 1 && c.map_or(0, class) != last).then_some(at)
            }
            // A group holds pieces of its own, so it is matched with
            // the rest of the pattern behind it, which `piece` does.
            Atom::Group(..) => None,
        }
    }

    fn same(&self, a: char, b: char) -> bool {
        a == b || (self.ignore_case && fold(a) == fold(b))
    }

    fn in_class(&self, ranges: &[(char, char)], c: char) -> bool {
        let hit = |c: char| ranges.iter().any(|(a, b)| (*a..=*b).contains(&c));
        hit(c) || (self.ignore_case && hit(fold(c)))
    }
}

/// `\w` and `\W`. nvim matches `[0-9A-Za-z_]` for `\w` and nothing
/// else, whatever the encoding, so every multibyte char is one char
/// that `\W` takes.
const fn word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// A char's class, as nvim's `utf_class` gives it and its
/// `charclass()` prints it. A class past 1 is a word char, and `\<`
/// and `\>` bound a run of *one* class rather than a run of word
/// chars, so `a漢b` is three words to nvim and to this. Blanks are
/// left with the punctuation at 1, the two being alike for the one
/// question asked here.
///
/// Under 0x100 the class is the default `'iskeyword'`,
/// `@,48-57,_,192-255`, which nvim 0.12.5 answers as the ASCII
/// letters, digits and `_`, plus `µ` and 0xC0-0xFF — not `ª` 0xAA
/// and not `º` 0xBA, which Unicode calls letters and nvim does not.
/// Past 0x100 the ranges below are nvim's own answers over the
/// blocks a search is written in; anything else there is a letter,
/// as every letter, syllable and ideograph recorded was.
fn class(c: char) -> u32 {
    if word(c) || c == '\u{b5}' || ('\u{c0}'..='\u{ff}').contains(&c) {
        return 2;
    }
    if c < '\u{100}' {
        return 1;
    }
    CLASSES
        .iter()
        .find(|(lo, hi, _)| (*lo..=*hi).contains(&c))
        .map_or(2, |(_, _, class)| *class)
}

/// The classes past 0x100 that are not a letter: what no word takes,
/// then the syllabaries, the ideographs and the emoji, each bounding
/// against the others and against a letter. A script's class is its
/// block's first char, as nvim's is.
const CLASSES: &[(char, char, u32)] = &[
    ('\u{300}', '\u{36f}', 1),        // the combining marks
    ('\u{2000}', '\u{206f}', 1),      // spaces, general punctuation
    ('\u{2e00}', '\u{2e7f}', 1),      // supplemental punctuation
    ('\u{3000}', '\u{3020}', 1),      // the ideographic punctuation
    ('\u{302a}', '\u{302f}', 1),      // the tone marks
    ('\u{3040}', '\u{309f}', 0x3040), // hiragana
    ('\u{30a0}', '\u{30ff}', 0x30a0), // katakana
    ('\u{3300}', '\u{9fff}', 0x4e00), // the ideographs
    ('\u{ac00}', '\u{d7a3}', 0xac00), // the hangul syllables
    ('\u{f900}', '\u{faff}', 0x4e00), // the ideographs, compatibility
    ('\u{fe00}', '\u{fe0f}', 1),      // the variation selectors
    ('\u{fe20}', '\u{fe6b}', 1),      // half marks, compatibility forms
    ('\u{ff00}', '\u{ff0f}', 1),      // the fullwidth punctuation
    ('\u{ff1a}', '\u{ff20}', 1),
    ('\u{ff3b}', '\u{ff40}', 1),
    ('\u{ff5b}', '\u{ff65}', 1),
    ('\u{ff9e}', '\u{ff9f}', 1),
    ('\u{1f000}', '\u{1faff}', 3), // the emoji
];

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
            '+'
            | '?'
            | '|'
            | ')'
            | '{'
            | '='
            | '@'
            | 'z'
            | '%'
            | '_'
            // `\1` to `\9` are Vim's backreferences, which the subset
            // has not got; a literal digit is not what they mean.
            | '1'..='9'
            // `\v \V \m \M` set how the rest of the pattern is read,
            // which a literal letter does not; refused rather than
            // read as one. `\\v` is still a backslash and a `v`.
            | 'v'
            | 'V'
            | 'm'
            | 'M' => {
                return Err(());
            }
            c => Atom::Char(c),
        };
        Ok(Some(atom))
    }

    /// `\(...\)` after its `\(`; nothing past [`DEPTH`] of them.
    fn group(&mut self) -> Option<Atom> {
        self.groups += 1;
        self.depth += 1;
        let number = self.groups;
        let alts = (self.depth <= DEPTH).then(|| self.alternatives())??;
        self.rest = self.rest.strip_prefix("\\)")?;
        self.depth -= 1;
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
#[must_use]
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

#[cfg(test)]
mod tests {
    use alloc::string::String;
    use alloc::vec;

    use super::{Captures, Pattern, Scan, Spent, expand};

    fn parse(source: &str) -> Pattern {
        Pattern::parse(source, false, false).unwrap()
    }

    /// `scan` walked by hand from each `Scan::Paused(at)`, a fresh
    /// budget each time, answering as `find` does: `find`'s own loop,
    /// done outside it, so a caller of `scan` can be checked against
    /// `find` at any budget.
    fn chained(
        pattern: &Pattern,
        text: &str,
        from: usize,
        budget: usize,
    ) -> Result<Option<(usize, usize, Captures)>, Spent> {
        let mut at = from;
        loop {
            let mut left = budget;
            match pattern.scan(text, at, text.len(), &mut left)? {
                Scan::Match(at, end, caps) => return Ok(Some((at, end, caps))),
                Scan::None => return Ok(None),
                Scan::Paused(resume) => at = resume,
            }
        }
    }

    /// `pattern.find(text, from)`, after checking that `chained`
    /// agrees at a few budgets: every pattern the crate tests goes
    /// through here, so `scan`'s bounded walk is exercised the way
    /// every caller of `find` uses it.
    fn find(
        pattern: &Pattern,
        text: &str,
        from: usize,
    ) -> Result<Option<(usize, usize, Captures)>, Spent> {
        let answer = pattern.find(text, from);
        for budget in [1, 7, 1000] {
            assert_eq!(
                chained(pattern, text, from, budget),
                answer,
                "budget {budget}"
            );
        }
        answer
    }

    fn found(pattern: &str, text: &str) -> Option<(usize, usize)> {
        let pattern = parse(pattern);
        let found = find(&pattern, text, 0).expect("a budget to spare");
        found.map(|(a, b, _)| (a, b))
    }

    fn folded(pattern: &str, ic: bool, scs: bool, text: &str) -> Option<usize> {
        let pattern = Pattern::parse(pattern, ic, scs)?;
        find(&pattern, text, 0).expect("a budget to spare").map(|m| m.0)
    }

    #[test]
    fn the_subset_parses_and_the_rest_is_refused() {
        for bad in [
            "a\\{2}",
            "\\zs",
            "\\(ab",
            "[ab",
            "a\\@!",
            "\\%d",
            "\\(a\\)\\1",
            "\\vfoo",
            "\\Vfoo",
            "\\mfoo",
            "\\Mfoo",
        ] {
            assert!(Pattern::parse(bad, false, false).is_none(), "{bad}");
        }
        assert_eq!(
            found("\\\\v", "a\\v"),
            Some((1, 3)),
            "a backslash, then a v"
        );
        let good = [
            "a.c",
            "^ab$",
            "[a-z_]\\+",
            "\\d\\D\\w\\W\\s\\S",
            "\\<w\\>",
            "\\(a\\|b\\)*",
            "a\\?b",
            "x\\n",
            "\\t",
            "",
            "$",
            "a^",
        ];
        for pattern in good {
            assert!(
                Pattern::parse(pattern, false, false).is_some(),
                "{pattern}"
            );
        }
    }

    #[test]
    fn a_scan_stops_at_its_bounds() {
        let c = parse("c");
        assert_eq!(c.scan("abcabc", 0, 1, &mut 10), Ok(Scan::None));
        assert_eq!(
            c.scan("abcabc", 0, 2, &mut 10),
            Ok(Scan::Match(2, 3, vec![Some((2, 3))]))
        );
        let bc = parse("bc");
        assert_eq!(
            bc.scan("abcabc", 0, 1, &mut 10),
            Ok(Scan::Match(1, 3, vec![Some((1, 3))])),
            "a match may end past `upto`"
        );
        let star = parse("x*");
        assert_eq!(
            star.scan("abc", 3, 3, &mut 10),
            Ok(Scan::Match(3, 3, vec![Some((3, 3))])),
            "an empty match at the text's end"
        );
        let z = parse("z");
        let mut budget = 2;
        assert_eq!(z.scan("abc", 0, 3, &mut budget), Ok(Scan::Paused(2)));
        assert_eq!(budget, 0, "a budget of 2 spends to the pause");
        let mut spent = 0;
        assert_eq!(
            z.scan("abc", 0, 3, &mut spent),
            Ok(Scan::Paused(0)),
            "a budget of 0 tries nothing"
        );
        assert_eq!(
            z.scan("abc", 4, 3, &mut 10),
            Ok(Scan::None),
            "`from` past `upto`"
        );
        // "aéb": a=0 (1B), é=1 (2B, bytes 1..3), b=3 (1B); `upto` 2
        // falls inside é, so the walk must not step past it to try an
        // attempt at 3, where "b" would otherwise match.
        let b = parse("b");
        assert_eq!(
            b.scan("a\u{e9}b", 0, 2, &mut 10),
            Ok(Scan::None),
            "a bound inside a char is never stepped past"
        );
    }

    #[test]
    fn atoms_match_what_they_name() {
        assert_eq!(found("a.c", "xabcx"), Some((1, 4)));
        assert_eq!(found("a.c", "a\nc"), None, "`.` is not a newline");
        assert_eq!(found("^ab", "xab\nab"), Some((4, 6)));
        assert_eq!(found("b$", "ab\nb"), Some((1, 2)));
        assert_eq!(found("a^", "a^"), Some((0, 2)), "`^` later is a char");
        assert_eq!(found("[b-d]\\+", "abcde"), Some((1, 4)));
        assert_eq!(found("[^a-c]", "abcd"), Some((3, 4)));
        assert_eq!(found("[]a]", "x]"), Some((1, 2)), "a first `]` is one");
        assert_eq!(found("[a\\]]", "]"), Some((0, 1)), "escaped in a class");
        assert_eq!(found("\\d\\+", "ab12c"), Some((2, 4)));
        assert_eq!(found("\\D", "12c"), Some((2, 3)));
        assert_eq!(found("\\w\\+", "--ab_1-"), Some((2, 6)));
        assert_eq!(found("\\W", "ab-"), Some((2, 3)));
        assert_eq!(found("\\s\\S", "a \tb"), Some((2, 4)));
        assert_eq!(found("\\<is\\>", "this is"), Some((5, 7)));
        assert_eq!(found("x\\n", "ax\nb"), Some((1, 3)));
        assert_eq!(found("\\t", "a\tb"), Some((1, 2)));
        assert_eq!(found("a\\.b", "a.b ab"), Some((0, 3)));
        assert_eq!(found("é.", "aéb"), Some((1, 4)), "chars, not bytes");
    }

    /// Recorded from nvim 0.12.5: `\w` is `[0-9A-Za-z_]` and nothing
    /// else, whatever the encoding, so every multibyte char is one
    /// char that `\W` takes.
    #[test]
    fn a_word_char_is_ascii_as_nvims_is() {
        assert_eq!(found("\\wb", "aéb abc"), Some((5, 7)), "not `éb`");
        assert_eq!(found("\\w", "é"), None, "`é` is not `\\w`");
        assert_eq!(found("\\w", "漢字ab"), Some((6, 7)));
        assert_eq!(found("\\W", "ab漢"), Some((2, 5)), "a char, not a byte");
        assert_eq!(found("\\w\\+", "a漢b"), Some((0, 1)), "a run stops at it");
        assert_eq!(found("\\w", "_9Z"), Some((0, 1)));
    }

    /// `\<` and `\>` are nvim's, which compares the two chars'
    /// classes rather than asking twice whether a char is a keyword
    /// char. Recorded from nvim 0.12.5, whose `substitute(s, '\<',
    /// '|', 'g')` marks `a漢b` as `|a|漢|b`, `aéb` as `|aéb` and
    /// `aªb` as `|aª|b`.
    #[test]
    fn a_word_bound_is_nvims_keyword_class() {
        assert_eq!(found("\\<is\\>", "this is"), Some((5, 7)));
        assert_eq!(found("\\<é", "aé é"), Some((4, 6)), "`é` is a letter");
        assert_eq!(found("\\<ß", "aß ß"), Some((4, 6)), "so is `ß`");
        assert_eq!(found("\\<ª", "xª ª"), None, "`ª` is punctuation");
        assert_eq!(found("\\<µ", "aµ µ"), Some((4, 6)), "`µ` is a letter");
        assert_eq!(found("\\<漢", "a漢 漢"), Some((1, 4)), "another class");
        assert_eq!(found("\\<가", "a가 가"), Some((1, 4)));
        assert_eq!(found("\\<😀", "a😀 😀"), Some((1, 5)));
        assert_eq!(found("\\<。", "a。 。"), None, "`。` is punctuation");
        assert_eq!(found("\\<b", "a漢b"), Some((4, 5)), "a class ends a word");
        assert_eq!(found("漢\\>", "a漢b"), Some((1, 4)), "and starts one");
        assert_eq!(found("a\\>", "aéb"), None, "`é` is the same class");
    }

    #[test]
    fn quantifiers_backtrack_and_zero_width_atoms_match_once() {
        assert_eq!(found("a*ab", "aaab"), Some((0, 4)));
        assert_eq!(found("a\\+", "baa"), Some((1, 3)));
        assert_eq!(found("ab\\?c", "ac abc"), Some((0, 2)));
        assert_eq!(found("ab\\=c", "abc"), Some((0, 3)));
        assert_eq!(found("^*a", "a"), Some((0, 1)));
        assert_eq!(found("x*", "abc"), Some((0, 0)), "an empty match");
        let star = parse("a*");
        let (start, end, caps) = find(&star, "bbb", 1).unwrap().unwrap();
        assert_eq!((start, end, caps), (1, 1, vec![Some((1, 1))]));
        assert_eq!(find(&parse("z"), "abc", 0), Ok(None));
        assert_eq!(parse("b").match_at("abc", 0), Ok(None), "exactly there");
        let at = parse("b").match_at("abc", 1).unwrap().map(|m| m.0);
        assert_eq!(at, Some(2));
    }

    #[test]
    fn groups_alternate_and_capture() {
        let pattern = parse("\\(foo\\|ba\\(r\\|z\\)\\)!");
        let (start, end, caps) = find(&pattern, "xbaz!", 0).unwrap().unwrap();
        assert_eq!((start, end), (1, 5));
        assert_eq!(caps, vec![Some((1, 5)), Some((1, 4)), Some((3, 4))]);
        let text = expand("[&|\\1|\\2|\\0]", "xbaz!", &caps);
        assert_eq!(text, "[baz!|baz|z|baz!]");
        let (_, _, caps) = find(&pattern, "foo!", 0).unwrap().unwrap();
        assert_eq!(caps[2], None, "an alternative not taken holds nothing");
        assert_eq!(expand("\\2-", "foo!", &caps), "-");
        assert_eq!(found("a\\|b", "cb"), Some((1, 2)));
        assert_eq!(found("\\(ab\\)*c", "ababc"), Some((0, 5)));
    }

    #[test]
    fn a_group_gives_back_what_follows_it_needs() {
        assert_eq!(found("\\(a\\|ab\\)c", "abc"), Some((0, 3)));
        assert_eq!(found("\\(a\\|ab\\)\\(c\\|bc\\)", "abc"), Some((0, 3)));
        assert_eq!(found("\\(a*\\)ab", "aab"), Some((0, 3)));
        assert_eq!(found("\\(a\\|aa\\)*$", "aaa"), Some((0, 3)));
        let caps = find(&parse("\\(a*\\)ab"), "aab", 0).unwrap().unwrap().2;
        assert_eq!(caps[1], Some((0, 1)), "the group gave back an `a`");
    }

    #[test]
    fn a_rejected_attempt_leaves_its_captures_as_they_were() {
        let found = find(&parse("\\(\\(a\\)x\\|ab\\)"), "ab", 0);
        let caps = found.unwrap().unwrap().2;
        assert_eq!(caps[2], None, "the alternative that failed holds none");
        assert_eq!(expand("[\\2]", "ab", &caps), "[]");
    }

    #[test]
    fn a_long_plain_line_costs_no_budget() {
        let mut line: String = core::iter::repeat_n('a', 1 << 20).collect();
        line.push('x');
        let end = line.len();
        assert_eq!(found(".*x", &line), Some((0, end)), "a megabyte of `.`");
        assert_eq!(found("a*x", &line), Some((0, end)));
        assert_eq!(found("\\w\\+x", &line), Some((0, end)));
    }

    #[test]
    fn a_pattern_that_ran_out_is_not_a_pattern_that_is_missing() {
        let text: String = core::iter::repeat_n('a', 30).collect();
        let spent = find(&parse("\\(a*\\)*b"), &text, 0);
        let missing = find(&parse("zzz"), &text, 0);
        assert_ne!(spent, missing, "a spent attempt must not read as none");
    }

    #[test]
    fn what_no_budget_can_match_says_spent_and_does_not_hang() {
        let text: String = core::iter::repeat_n('a', 30).collect();
        assert_eq!(find(&parse("\\(a*\\)*b"), &text, 0), Err(Spent));
        assert_eq!(found("\\(a*\\)*b", "aaab"), Some((0, 4)));
        assert_eq!(found("\\(x*\\)*", "abc"), Some((0, 0)), "empty, once");
        assert_eq!(found("\\(\\)*x", "x"), Some((0, 1)));
        let deep: String = core::iter::repeat_n("\\(", 500)
            .chain(core::iter::once("a"))
            .chain(core::iter::repeat_n("\\)", 500))
            .collect();
        assert!(Pattern::parse(&deep, false, false).is_none(), "too deep");
        let long: String = core::iter::repeat_n('a', 1000).collect();
        let turns = find(&parse("\\(a\\)*b"), &long, 0);
        assert_eq!(turns, Err(Spent), "more turns than the stack holds");
        assert_eq!(found("a*b", &long), None, "a plain atom has no turns");
    }

    #[test]
    fn replacements_expand_their_escapes() {
        let caps = vec![Some((0, 2))];
        let text = expand("x\\ry\\n\\t\\\\\\&", "ab", &caps);
        assert_eq!(text, String::from("x\ny\t\\&"));
        assert_eq!(expand("\\", "ab", &caps), "\\", "a last backslash stays");
        assert_eq!(expand("\\9", "ab", &caps), "", "an unset group is empty");
        assert_eq!(expand("\\q", "ab", &caps), "q", "any other is itself");
    }

    #[test]
    fn case_folds_by_option_or_flag() {
        assert_eq!(folded("ab", true, false, "xAB"), Some(1));
        assert_eq!(folded("Ab", true, true, "ab"), None, "smartcase");
        assert_eq!(folded("ab", true, true, "AB"), Some(0));
        assert_eq!(folded("\\cAB", false, false, "ab"), Some(0));
        assert_eq!(folded("\\Cab", true, false, "AB"), None);
        assert_eq!(folded("[a-c]", true, false, "B"), Some(0), "a class too");
        assert_eq!(folded("ab", false, false, "AB"), None);
    }
}
