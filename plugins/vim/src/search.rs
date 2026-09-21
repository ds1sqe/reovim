//! `/`, `?`, `n`, `N`, `*`, `#`: the last pattern, the list the search
//! Module fills for it, and `hlsearch`'s marks.
//!
//! Nothing here matches anything. A key that cannot be answered from
//! the list Vim holds leaves as [`Effect::Search`] and comes back
//! through [`Vim::matched`], which runs the same motion again.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write as _;

use reovim_search_api::LINE;

use crate::{Effect, Motion, Pending, Vim};

/// The most spans a `View` carries (clients/core/api's declaration).
pub const SPANS: usize = 4096;

/// What `:set` holds: the search's options, and the personality.
///
/// A row of booleans is what `:set` is, one field per name it takes,
/// which is why the lint against four of them is off here.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    pub ignorecase: bool,
    pub smartcase: bool,
    pub hlsearch: bool,
    /// `:set vi`: nvim's way wherever reovim's default has one of its
    /// own, which is every row of [`crate::DIVERGENCES`].
    pub vi: bool,
}

/// The last search: its pattern, its direction, and whether `:noh`
/// has hidden its marks.
#[derive(Clone, Debug, Default)]
pub struct Search {
    pub pattern: String,
    pub forward: bool,
    pub stale: bool,
    pub options: Options,
}

/// What Vim asks the search Module for: the key of a list, and where
/// that list was begun.
///
/// `from` is the list's own, not the cursor's: a list begun at one
/// place and read on from there stays one list, so the run the
/// lookup reads an entry out of is the run it was filled into.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Want {
    pub pattern: String,
    pub ignorecase: bool,
    pub smartcase: bool,
    pub from: u64,
    pub epoch: u64,
}

/// How far the list goes, as the last read of `matches` left it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Filled {
    /// More to come: the fill has not gone through the text.
    #[default]
    More,
    /// Everything the pattern matches is held.
    Whole,
    /// A spent offset, or a line longer than the fill will take.
    Spent,
    /// A pattern the matcher has not, or a window that is not UTF-8.
    Refused,
    /// This Client serves no `/self/search`.
    Absent,
}

/// The motion a search key left waiting, with the origin the door
/// found it in: the turn that runs it again is the turn that key
/// would have had.
#[derive(Clone, Copy, Debug)]
pub struct Waiting {
    pub motion: Motion,
    pub n: usize,
    /// `macros.fed` and `repeat.replaying`, in that order.
    pub fed: (bool, bool),
}

/// The list Vim has read of one view's `matches`, and nothing more.
///
/// It is never patched: [`Vim::marked`] drops it where positions
/// move, and the fill begins again. The two runs are the fill's own
/// order — from the start of `from`'s line to the text's end, then
/// around from 0 back to it — so each ascends and `before` holds the
/// earlier bytes.
#[derive(Debug, Default)]
pub struct Found {
    /// The pattern, `ignorecase` and `smartcase` this list is of.
    pub key: Option<(String, bool, bool)>,
    pub epoch: u64,
    pub from: usize,
    /// The entries at or past the start of `from`'s line.
    pub after: Vec<(usize, usize)>,
    /// The entries before it, which the fill found after the wrap.
    pub before: Vec<(usize, usize)>,
    pub filled: Filled,
    /// The `at` of each entry nvim's own per-line search never finds,
    /// in the text's order: what the count leaves out. Filled where
    /// the list turns whole, and nowhere else.
    pub skipped: Vec<usize>,
    pub waiting: Option<Waiting>,
    /// `done`, `total` and what is left, as the last read of the
    /// view's `progress` left them: the row's three numbers.
    pub progress: Option<(u64, u64, Option<u64>)>,
    /// The way a landing over a list still filling searched. The row
    /// stands until the next key, which is what clears it.
    pub shown: Option<bool>,
}

/// What the lookup made of the list: where to land, that there is
/// nowhere, or that the list does not say yet.
pub enum Looked {
    At(usize),
    None,
    Unknown,
}

/// Where a forward search compares an entry: one char back for one
/// that starts on its line's end, as nvim compares it, so the cursor
/// beside it passes it.
fn key_of(at: usize, text: &str) -> usize {
    let ends = at >= text.len() || text.as_bytes()[at] == b'\n';
    if !ends {
        return at;
    }
    at - text[..at].chars().next_back().map_or(0, char::len_utf8)
}

impl Found {
    /// Where the fill began: the start of `from`'s own line, which is
    /// the bound between the two runs.
    fn start(&self, text: &str) -> usize {
        let from = self.from.min(text.len());
        text[..from].rfind('\n').map_or(0, |i| i + 1)
    }

    /// Every entry read, in the text's own order.
    fn ordered(&self) -> impl Iterator<Item = usize> + '_ {
        self.before.iter().chain(&self.after).map(|(at, _)| *at)
    }

    /// The `n`th entry from `cursor` in `forward`'s direction, as
    /// nvim finds it.
    pub(crate) fn nth(
        &self,
        cursor: usize,
        forward: bool,
        n: usize,
        text: &str,
    ) -> Looked {
        let start = self.start(text);
        let whole = self.filled == Filled::Whole;
        let mut at = cursor;
        for _ in 0..n.max(1) {
            match self.step(at, forward, whole, start, text) {
                Looked::At(next) => at = next,
                other => return other,
            }
        }
        Looked::At(at)
    }

    /// One step, in `forward`'s direction.
    fn step(
        &self,
        at: usize,
        forward: bool,
        whole: bool,
        start: usize,
        text: &str,
    ) -> Looked {
        if forward {
            self.onward(at, whole, start, text)
        } else {
            self.backward(at, whole)
        }
    }

    /// Forward, an entry in `at`'s own run is final as read: the fill
    /// goes in order, so it found everything between the two.
    /// Anything past that run's end needs the whole list.
    fn onward(
        &self,
        at: usize,
        whole: bool,
        start: usize,
        text: &str,
    ) -> Looked {
        let next = self.ordered().find(|e| key_of(*e, text) > at);
        if let Some(entry) = next
            && (entry >= start) == (at >= start)
        {
            return Looked::At(entry);
        }
        if !whole {
            return Looked::Unknown;
        }
        // Nothing after it: around to the top again.
        next.or_else(|| self.ordered().next()).map_or(Looked::None, Looked::At)
    }

    /// Which match the cursor is on and of how many, as nvim's
    /// `searchcount()` counts them: the entries that start at or
    /// before the cursor, and how many there are, each less the ones
    /// nvim never finds.
    ///
    /// `None` until the list is whole: a rank is the entries before
    /// it, and the ones before `from`'s own line come in after the
    /// wrap. Nothing is walked — both runs and `skipped` ascend.
    fn rank(&self, cursor: usize) -> Option<(usize, usize)> {
        if self.filled != Filled::Whole {
            return None;
        }
        let upto = |run: &[(usize, usize)]| {
            run.partition_point(|(at, _)| *at <= cursor)
        };
        let passed = self.skipped.partition_point(|at| *at <= cursor);
        let current = upto(&self.before) + upto(&self.after) - passed;
        let total = self.before.len() + self.after.len() - self.skipped.len();
        Some((current, total))
    }

    /// Backward needs the whole list: the fill has one direction, so
    /// nothing before the cursor is final until it has gone through.
    fn backward(&self, at: usize, whole: bool) -> Looked {
        if !whole {
            return Looked::Unknown;
        }
        let before = self.ordered().filter(|e| *e < at).last();
        let found = before.or_else(|| self.ordered().last());
        found.map_or(Looked::None, Looked::At)
    }
}

impl Vim {
    /// E486 for the last pattern; nothing moved.
    pub(crate) fn not_found(&mut self) -> Effect {
        let pattern = self.search.pattern.clone();
        self.word(&alloc::format!("E486: Pattern not found: {pattern}"))
    }

    /// E363 for a pattern the matcher will not spend what it costs
    /// on; nothing moved.
    pub(crate) fn too_costly(&mut self) -> Effect {
        let pattern = self.search.pattern.clone();
        self.word(&alloc::format!(
            "E363: pattern costs more than the matcher will spend: {pattern}"
        ))
    }

    /// A word a search says: through `error`, so it rings, and
    /// nothing moved.
    fn word(&mut self, message: &str) -> Effect {
        self.error(message);
        Effect::Nothing
    }

    /// Which way `n` and `N` go: the last search's own direction, and
    /// its reverse for `N`. The motion runs by it and the row leads
    /// with it, so `N` after `/foo` says `?foo` as nvim's message
    /// does.
    pub(crate) const fn again(&self, reversed: bool) -> bool {
        self.search.forward != reversed
    }

    /// Whether a motion is waiting for entries it has not been handed
    /// yet, which is what a frame that takes no key is doing.
    #[must_use]
    pub const fn waiting(&self) -> bool {
        self.found.waiting.is_some()
    }

    /// `<C-c>`, or `<Esc>` as the next key, while a search waits: the
    /// wait dropped where it stands and the command behind it with
    /// it. The bell drops what a macro fed; nothing is said, nothing
    /// has moved, and the view stays open for whoever still wants it.
    pub fn interrupted(&mut self) {
        self.found.waiting = None;
        self.pending = Pending::default();
        self.repeat.record.clear();
        self.repeat.first = false;
        self.ring();
    }

    /// How far the fill of the list Vim holds has gone: the two
    /// numbers of the view's `progress` and the time its own stamps
    /// leave, which the Plugin reads after each slice.
    pub const fn filling(&mut self, done: u64, total: u64, eta: Option<u64>) {
        self.found.progress = Some((done, total, eta));
    }

    /// The row a search shows: the way it searched and the pattern,
    /// then which match the cursor is on and of how many.
    ///
    /// While the list fills the first number is `?`, the second is
    /// what is held so far with a `+`, and how far the fill has gone
    /// and how long that leaves follow. Once the count answers it is
    /// that pair alone.
    pub(crate) fn row(&self) -> String {
        let forward = match self.found.waiting.map(|waiting| waiting.motion) {
            Some(Motion::Search { forward } | Motion::Star { forward }) => {
                forward
            }
            Some(Motion::SearchAgain { reversed }) => self.again(reversed),
            _ => self.found.shown.unwrap_or(self.search.forward),
        };
        let lead = if forward { '/' } else { '?' };
        let pattern = &self.search.pattern;
        let mut row = alloc::format!("{lead}{pattern}  ");
        if let Some((current, total)) = self.counted() {
            let _ = write!(row, "[{current}/{total}]");
            return row;
        }
        let held = self.found.after.len() + self.found.before.len();
        let _ = write!(row, "[?/{held}+]");
        let Some((done, total, eta)) = self.found.progress else { return row };
        if total > 0 {
            // Thousandths, so `42.005%` is never read as `42.5%`.
            let scaled = u128::from(done) * 100_000 / u128::from(total);
            let _ = write!(row, "  {}.{:03}%", scaled / 1000, scaled % 1000);
        }
        if let Some(eta) = eta {
            let _ = write!(row, "  ~{}s", eta.div_ceil(1_000_000_000));
        }
        row
    }

    /// Which match the cursor is on, and of how many, as nvim's
    /// `searchcount()` says it with no cap: the row's two numbers,
    /// and what a corpus case compares.
    ///
    /// `None` while the list is not the last pattern's under today's
    /// `ic` and `scs`, or not yet whole. A pattern nothing matches is
    /// `Some((0, 0))`, as nvim's is.
    #[must_use]
    pub fn counted(&self) -> Option<(usize, usize)> {
        if self.found.key.as_ref() != Some(&self.keyed()) {
            return None;
        }
        self.found.rank(self.cursor)
    }

    /// The entries nvim's own search never finds: one on a line's end
    /// that the entry before it reached, nvim searching a line from
    /// column 0, each match from the end of the one before it, and
    /// stopping where that reaches the line's end.
    ///
    /// The fill anchors every line, so it holds them all; the count
    /// is nvim's, so it passes them.
    fn skipping(&self) -> Vec<usize> {
        let entries = || self.found.before.iter().chain(&self.found.after);
        entries()
            .zip(entries().skip(1))
            .filter(|(before, entry)| self.reaches(**before, entry.0))
            .map(|(_, entry)| entry.0)
            .collect()
    }

    /// Whether `at` is a line's end that the entry before it reached,
    /// the two standing on the one line. nvim's search of that line
    /// goes on from where `before` ended and stops where it reaches
    /// the end, so it never finds what stands there.
    fn reaches(&self, before: (usize, usize), at: usize) -> bool {
        let (from, len) = before;
        let ends = at >= self.text.len() || self.text.as_bytes()[at] == b'\n';
        ends && (from + len).max(self.forward_char(from)) == at
            && !self.text[from..at].contains('\n')
    }

    /// The key of the list the options and the pattern name now.
    fn keyed(&self) -> (String, bool, bool) {
        let Search { pattern, options, .. } = &self.search;
        (pattern.clone(), options.ignorecase, options.smartcase)
    }

    /// A list of `key` begun at `from`: nothing read yet, and more to
    /// come.
    fn begun(&mut self, key: (String, bool, bool), from: usize) {
        self.found.key = Some(key);
        self.found.from = from;
        self.found.after.clear();
        self.found.before.clear();
        self.found.skipped.clear();
        self.found.filled = Filled::More;
    }

    /// The list Vim holds as something to ask for.
    fn asking(&self) -> Want {
        let (pattern, ignorecase, smartcase) =
            self.found.key.clone().unwrap_or_default();
        Want {
            pattern,
            ignorecase,
            smartcase,
            from: self.found.from as u64,
            epoch: self.found.epoch,
        }
    }

    /// The epoch the list Vim holds is of: what an answer must name
    /// to be this list's.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.found.epoch
    }

    /// The door every search key goes through, at the head of
    /// `motion` and of `visual_motion`, before either takes anything:
    /// `Some(Effect::Search)` where the list cannot answer yet, and
    /// `None` where the motion goes on as it always has, to land or
    /// to say its word.
    ///
    /// The lookup is asked before any word, so an entry read before a
    /// slice failed is landed on rather than said over.
    pub(crate) fn wanting(
        &mut self,
        motion: Motion,
        n: usize,
    ) -> Option<Effect> {
        let forward = match motion {
            Motion::Search { forward } => forward,
            Motion::SearchAgain { reversed } => self.again(reversed),
            Motion::Star { forward } => {
                self.starred(forward).then_some(forward)?
            }
            _ => return None,
        };
        if self.lossy || self.search.pattern.is_empty() {
            return None;
        }
        let key = self.keyed();
        if self.found.key.as_ref() != Some(&key) {
            self.begun(key, self.cursor);
        }
        let looked = self.found.nth(self.cursor, forward, n, &self.text);
        let answered = !matches!(looked, Looked::Unknown);
        if answered || self.found.filled != Filled::More {
            return None;
        }
        self.found.waiting = Some(Waiting {
            motion,
            n,
            fed: (self.macros.fed, self.repeat.replaying),
        });
        Some(Effect::Search(self.asking()))
    }

    /// What one read of `matches` found, and how far the fill got.
    ///
    /// Another epoch is nobody's: the list it names was dropped. Else
    /// the entries join their runs, and the motion that waited runs
    /// again — whatever `filled` says, since the door reads it and
    /// answers again or lets the word be said.
    pub fn matched(
        &mut self,
        epoch: u64,
        entries: &[(usize, usize)],
        filled: Filled,
    ) -> Effect {
        if epoch != self.found.epoch {
            return Effect::Nothing;
        }
        let start = self.found.start(&self.text);
        let after = entries.iter().filter(|entry| entry.0 >= start);
        self.found.after.extend(after.copied());
        let before = entries.iter().filter(|entry| entry.0 < start);
        self.found.before.extend(before.copied());
        self.found.filled = filled;
        if filled == Filled::Whole {
            let passed = self.skipping();
            self.found.skipped = passed;
        }
        // A list nobody waits for — `hlsearch`'s — would else be
        // opened again at every edit, each paying the whole budget.
        if filled == Filled::Spent {
            self.search.stale = true;
        }
        let Some(waiting) = self.found.waiting.take() else {
            return Effect::Nothing;
        };
        self.turned(waiting)
    }

    /// The motion that waited, run in the turn its own key would have
    /// had: the origin it came with, the change it may make, and the
    /// message that makes `Nothing` a change.
    fn turned(&mut self, waiting: Waiting) -> Effect {
        let fed = core::mem::replace(&mut self.macros.fed, waiting.fed.0);
        let was = core::mem::replace(&mut self.repeat.replaying, waiting.fed.1);
        let before = self.position();
        let typed = self.mode == crate::Mode::Insert;
        let effect = self.motion(waiting.motion, waiting.n);
        let effect = self.around(before, typed, effect);
        self.repeat.replaying = was;
        self.macros.fed = fed;
        if !self.message.is_empty() && effect == Effect::Nothing {
            Effect::Moved
        } else {
            effect
        }
    }

    /// What a frame's `hlsearch` wants filled, `None` where it wants
    /// nothing: the marks are off, `:noh` hid them, there is no
    /// pattern, the text is not UTF-8, or the list is as full as it
    /// will get.
    ///
    /// A list it begins is from the first byte of the first line
    /// shown, so the window's own rows fill first.
    pub fn wants(&mut self) -> Option<Want> {
        // A waiting motion's list is what the frame must fill, and a
        // row shown over one still filling is what the next frame
        // moves: either comes before `hlsearch`'s own.
        let row =
            self.found.shown.is_some() && self.found.filled == Filled::More;
        if self.found.waiting.is_some() || row {
            return Some(self.asking());
        }
        let Search { pattern, options, stale, .. } = &self.search;
        if !options.hlsearch || *stale || pattern.is_empty() || self.lossy {
            return None;
        }
        if self.found.filled != Filled::More {
            return None;
        }
        let key = self.keyed();
        if self.found.key.as_ref() != Some(&key) {
            let top = self.line_at(self.top).unwrap_or(0);
            self.begun(key, top);
        }
        Some(self.asking())
    }

    /// `/` or `?` run, or `n` and `N`: the cursor onto the entry the
    /// lookup names, or the word that says why there is none.
    pub(crate) fn search(&mut self, forward: bool, n: usize) -> Effect {
        if self.lossy {
            return self
                .word("E486: this text is not UTF-8 and is not searched");
        }
        if self.search.pattern.is_empty() {
            return self.word("E35: No previous regular expression");
        }
        match self.found.nth(self.cursor, forward, n, &self.text) {
            Looked::At(at) => self.alighted(at, forward),
            Looked::None => self.not_found(),
            Looked::Unknown => self.unanswered(),
        }
    }

    /// The landing: the cursor onto `at`, and the row with it, whole
    /// list or not — a list still filling shows how far it has got,
    /// and a whole one which match this is. Nothing is shown under an
    /// operator: the edit would drop the list the row is of, and a
    /// text filled again for a row is a fill nobody asked for.
    fn alighted(&mut self, at: usize, forward: bool) -> Effect {
        let effect = self.onto(at);
        if !self.pending.running {
            self.found.shown = Some(forward);
        }
        effect
    }

    /// The cursor onto `at`: the entry itself, or the last char of
    /// its line where the entry is that line's end and no operator is
    /// running, which is where nvim shows the landing. The previous
    /// place is set where the landing, not the entry, is elsewhere.
    fn onto(&mut self, at: usize) -> Effect {
        let ends = at >= self.text.len() || self.text.as_bytes()[at] == b'\n';
        let to =
            if ends && !self.pending.running { self.last_char(at) } else { at };
        self.search.stale = false;
        if to != self.cursor {
            self.set_previous();
        }
        self.go(to);
        Effect::Moved
    }

    /// The word for a list that cannot answer, the number being the
    /// cause's. The Module does not tell its two causes apart, so the
    /// engine tells them by its own text.
    fn unanswered(&mut self) -> Effect {
        let pattern = self.search.pattern.clone();
        match self.found.filled {
            Filled::Spent if self.long_line() => self.word(&alloc::format!(
                "E363: a line is longer than 1 MiB: {pattern}"
            )),
            Filled::Spent => self.too_costly(),
            Filled::Absent => self.word(&alloc::format!(
                "E486: this Client has no /self/search: {pattern}"
            )),
            _ => self.not_found(),
        }
    }

    /// A line of more bytes than the fill will take, its own `\n`
    /// counted: what a cut would fake `^` and `$` at, so the Module
    /// refuses it for good.
    fn long_line(&self) -> bool {
        let line = usize::try_from(LINE).unwrap_or(usize::MAX);
        self.text.split_inclusive('\n').any(|one| one.len() > line)
    }

    /// `*` and `#`: the keyword under or after the cursor as the
    /// pattern, whole words only. False where there is none, which
    /// `star` says.
    fn starred(&mut self, forward: bool) -> bool {
        let Some(word) = self.keyword_at_cursor() else { return false };
        self.search.pattern = alloc::format!("\\<{word}\\>");
        self.search.forward = forward;
        true
    }

    /// `*` and `#` once the door has passed them: the pattern is set,
    /// and the search is a search.
    pub(crate) fn star(&mut self, forward: bool, n: usize) -> Effect {
        if !self.starred(forward) {
            self.error("E348: No string under cursor");
            return Effect::Nothing;
        }
        self.search(forward, n)
    }

    /// `gF`: the `PATH:LOCATOR` under the cursor opened as `:e`
    /// opens it, the Plugin splitting the name and landing on the
    /// rest. The name is the run of file-name chars at or after the
    /// cursor on its line, as nvim reads one by `'isfname'`, with `:`
    /// among them so the locator comes with it. There is no `gf`, and
    /// no count is read.
    pub(crate) fn file_under_cursor(&mut self) -> Effect {
        let Some(name) = self.name_at_cursor() else {
            self.error("E446: No file name under cursor");
            return Effect::Nothing;
        };
        Effect::Open(name, false)
    }

    /// The run of file-name chars at or after the cursor, on its line
    /// alone.
    fn name_at_cursor(&self) -> Option<String> {
        self.run_at_cursor(|c| {
            c.is_alphanumeric() || "/.-_+,#$%~=:".contains(c)
        })
    }

    fn keyword_at_cursor(&self) -> Option<String> {
        self.run_at_cursor(|c| c.is_alphanumeric() || c == '_')
    }

    /// The run of `held` chars the cursor stands in, or the next one
    /// after it on the same line: what `*` reads as a word and what
    /// `gF` reads as a name.
    fn run_at_cursor(&self, held: impl Fn(char) -> bool) -> Option<String> {
        let start = self.line_start(self.cursor);
        let line = &self.text[start..self.line_end(self.cursor)];
        let col = self.cursor - start;
        let from = line[col..].find(&held).map(|i| col + i)?;
        let first = line[..from].rfind(|c: char| !held(c)).map_or(0, |i| i + 1);
        let stop = line[from..]
            .find(|c: char| !held(c))
            .map_or(line.len(), |i| from + i);
        Some(line[first..stop].into())
    }

    /// `hlsearch`'s byte spans on the lines shown, read off the runs,
    /// each entry `skipped` names left out: the entries inside the
    /// window's bytes, each taking at least the char it sits on, as
    /// nvim shows an empty match. `room` is how many spans the View
    /// has left once the selection has taken its own.
    ///
    /// `skipped` is empty while the list fills, so a run still filling
    /// marks everything it holds; `begun` is the door that keeps it
    /// so, clearing it with the runs it begins again.
    pub(crate) fn search_spans(
        &self,
        top: usize,
        height: usize,
        room: usize,
    ) -> Vec<(usize, usize)> {
        let Search { pattern, options, stale, .. } = &self.search;
        if !options.hlsearch || *stale || pattern.is_empty() {
            return Vec::new();
        }
        let from = self.line_at(top).unwrap_or(0);
        let upto =
            self.line_at(top.saturating_add(height)).unwrap_or(self.text.len());
        let skipped = &self.found.skipped;
        let shown = |run: &Vec<(usize, usize)>| {
            let first = run.partition_point(|(at, _)| *at < from);
            let last = run.partition_point(|(at, _)| *at < upto);
            run[first..last]
                .iter()
                .copied()
                .filter(|(at, _)| skipped.binary_search(at).is_err())
                .collect::<Vec<_>>()
        };
        let mut marks = shown(&self.found.before);
        marks.extend(shown(&self.found.after));
        marks.truncate(room.min(SPANS));
        marks
            .into_iter()
            .map(|(at, len)| (at, (at + len).max(self.forward_char(at))))
            .collect()
    }
}
