//! `/`, `?`, `n`, `N`, `*`, `#`: the last pattern, a match found the
//! way nvim finds it, and `hlsearch`'s marks.

use alloc::string::String;
use alloc::vec::Vec;

use reovim_pattern::Pattern;

use crate::{Effect, Vim};

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

impl Vim {
    /// E486 for the last pattern; nothing moved.
    pub(crate) fn not_found(&mut self) -> Effect {
        let pattern = &self.search.pattern;
        self.message = alloc::format!("E486: Pattern not found: {pattern}");
        Effect::Nothing
    }

    /// The last pattern as a `Pattern`, or E486.
    fn pattern(&mut self) -> Option<Pattern> {
        let Search { pattern, options, .. } = &self.search;
        let parsed =
            Pattern::parse(pattern, options.ignorecase, options.smartcase);
        if parsed.is_none() {
            self.not_found();
        }
        parsed
    }

    /// `/` or `?` run, or `n` and `N`: the cursor to the `n`th match
    /// in `forward`'s direction, wrapping; E486 when there is none.
    pub(crate) fn search(&mut self, forward: bool, n: usize) -> Effect {
        let Some(pattern) = self.pattern() else { return Effect::Nothing };
        let mut at = Some(self.cursor);
        for _ in 0..n {
            at = at.and_then(|from| self.nth(&pattern, from, forward));
        }
        let Some(at) = at else { return self.not_found() };
        self.search.stale = false;
        self.go(at);
        Effect::Moved
    }

    /// Every match starting from `from` up to `limit`, each found
    /// after the one before: past its end, or one char on for an
    /// empty one.
    fn matches(
        &self,
        pattern: &Pattern,
        from: usize,
        limit: usize,
    ) -> Vec<usize> {
        let mut found = Vec::new();
        let mut col = from;
        let mut next = pattern.find(&self.text, col);
        while let Some((start, end, _)) = next.filter(|m| m.0 <= limit) {
            found.push(start);
            col = self.past(start, end);
            next = (col <= limit && start < self.text.len())
                .then(|| pattern.find(&self.text, col))
                .flatten();
        }
        found
    }

    fn nth(
        &self,
        pattern: &Pattern,
        from: usize,
        forward: bool,
    ) -> Option<usize> {
        if forward {
            self.next_match(pattern, from)
        } else {
            self.prev_match(pattern, from)
        }
    }

    /// Where to look after a match: its end, or the next char for an
    /// empty one.
    fn past(&self, start: usize, end: usize) -> usize {
        if end > start { end } else { self.forward_char(start) }
    }

    /// The first match after `cursor`, as nvim's `cpo-c` has it: a
    /// match at or before the cursor is passed by continuing from its
    /// end; then the lines after; then from the top.
    fn next_match(&self, pattern: &Pattern, cursor: usize) -> Option<usize> {
        let line_end = self.line_end(cursor);
        let line_start = self.line_start(cursor);
        let on_line = self.matches(pattern, line_start, line_end);
        let later = on_line.into_iter().find(|start| *start > cursor);
        let after = (line_end < self.text.len())
            .then(|| pattern.find(&self.text, line_end + 1));
        later
            .or_else(|| after.flatten().map(|(start, _, _)| start))
            .or_else(|| pattern.find(&self.text, 0).map(|(start, _, _)| start))
    }

    /// The last match before `cursor`, else the last in the text.
    fn prev_match(&self, pattern: &Pattern, cursor: usize) -> Option<usize> {
        let all = self.matches(pattern, 0, self.text.len());
        let before = all.iter().rev().find(|s| **s < cursor);
        before.or_else(|| all.last()).copied()
    }

    /// `*` and `#`: the keyword under or after the cursor as the
    /// pattern, whole words only, then a search.
    pub(crate) fn star(&mut self, forward: bool, n: usize) -> Effect {
        let Some(word) = self.keyword_at_cursor() else {
            self.message = String::from("E348: No string under cursor");
            return Effect::Nothing;
        };
        self.search.pattern = alloc::format!("\\<{word}\\>");
        self.search.forward = forward;
        self.search(forward, n)
    }

    fn keyword_at_cursor(&self) -> Option<String> {
        let keyword = |c: char| c.is_alphanumeric() || c == '_';
        let start = self.line_start(self.cursor);
        let line = &self.text[start..self.line_end(self.cursor)];
        let col = self.cursor - start;
        let from = line[col..].find(keyword).map(|i| col + i)?;
        let first =
            line[..from].rfind(|c: char| !keyword(c)).map_or(0, |i| i + 1);
        let stop = line[from..]
            .find(|c: char| !keyword(c))
            .map_or(line.len(), |i| from + i);
        Some(line[first..stop].into())
    }

    /// `hlsearch`'s byte spans on the lines shown: every match of the
    /// last pattern, until `:noh`. An empty match takes the char it
    /// sits on, as nvim shows one.
    pub(crate) fn search_spans(
        &self,
        top: usize,
        height: usize,
    ) -> Vec<(usize, usize)> {
        let Search { pattern, options, stale, .. } = &self.search;
        if !options.hlsearch || *stale || pattern.is_empty() {
            return Vec::new();
        }
        let Some(pattern) =
            Pattern::parse(pattern, options.ignorecase, options.smartcase)
        else {
            return Vec::new();
        };
        let shown = |line: usize| line >= top && line < top + height;
        self.matches(&pattern, 0, self.text.len())
            .into_iter()
            .filter(|&start| shown(self.position_of(start).0))
            .map(|start| {
                let end = pattern
                    .match_at(&self.text, start)
                    .map_or(start, |(e, _)| e);
                (start, end.max(self.forward_char(start)))
            })
            .collect()
    }
}
