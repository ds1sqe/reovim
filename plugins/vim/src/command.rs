//! The command line: `:` and what a person types after it, and the
//! `/` and `?` prompts that share it.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use reovim_client_sh::Act;
use reovim_pattern::{Pattern, Spent, expand};

use crate::{Effect, Ex, Key, Mode, Motion, Vim, Wild, minimal_edit};

/// Lines from 0, both ends included.
type Lines = (usize, usize);

impl Vim {
    /// `:`, `/` or `?`: the prompt opens over an empty line of the
    /// two, the mode to return to kept, and the want with it: nvim's
    /// `nv_colon` and `nv_search` leave `curswant` alone.
    pub(crate) fn prompt(&mut self, prompt: char, count: usize) -> Effect {
        self.keep_want = true;
        self.prompt.at = prompt;
        self.prompt.line().put(0, "");
        // `:` from a selection leaves it and names its lines.
        if prompt == ':' && matches!(self.mode, Mode::Visual(_)) {
            self.leave_visual();
            self.prompt.line().put(0, "'<,'>");
        }
        self.prompt.before = self.mode;
        self.mode = Mode::Command;
        self.pending.count = count;
        self.wild = None;
        Effect::Moved
    }

    /// One key on the command line, which is the shell's line editor:
    /// its acts, and before them the keys Vim reads itself, the two
    /// ways out of the prompt and the wildmenu's own. No key on the
    /// line moves the cursor, so every one keeps the want; what the
    /// line runs sets it where it lands, as nvim's ex commands do
    /// through `beginline`.
    pub(crate) fn command(&mut self, key: Key) -> Effect {
        self.keep_want = true;
        match key {
            // The line editor makes nothing of Esc, and keeps the line
            // on a Backspace that has nothing to take; here both leave
            // the prompt, as they always have.
            Key::Esc => return self.leave_prompt(),
            Key::Backspace if self.prompt.text().is_empty() => {
                return self.leave_prompt();
            }
            // The wildmenu's own two keys, while one is open.
            Key::Tab if self.wild.is_some() => return self.menu(true),
            Key::Ctrl('p') if self.wild.is_some() => return self.menu(false),
            _ => {}
        }
        // Every other key leaves the menu and is itself.
        self.wild = None;
        let key = if key == Key::Ctrl('j') { Key::Enter } else { key };
        match self.prompt.line().key(key) {
            Act::Nothing | Act::Clear => Effect::Nothing,
            Act::Redraw => Effect::Moved,
            Act::Complete => self.complete(),
            Act::Submit(line) => self.run(line),
            // Ctrl-C, and Ctrl-D on an empty line: Esc's way out.
            Act::Quit => self.leave_prompt(),
        }
    }

    /// Tab on the `:` line, which only the Client can answer: the names
    /// and the paths a completion offers are both in the tree. Nothing
    /// for `/` and `?`, which complete nothing.
    fn complete(&self) -> Effect {
        if self.prompt.at != ':' {
            return Effect::Nothing;
        }
        Effect::Complete(String::from(self.prompt.text()))
    }

    /// What Tab found where the first token stands: the names that may
    /// run there, which the wildmenu walks. One alone is kept as the
    /// one candidate, so the status row can say the usage line `form`
    /// gives for it, where an argument's one name is not kept at all.
    pub(crate) fn listed(
        &mut self,
        from: usize,
        found: Vec<String>,
        form: Option<String>,
    ) -> Effect {
        if found.is_empty() {
            return Effect::Nothing;
        }
        self.wild = Some(Wild { candidates: found, at: 0, from, form });
        self.take()
    }

    /// What Tab found over the tree, or among the options of `:set`:
    /// one name is simply put on the line, as the line's own `apply`
    /// puts it, since nothing is left to choose between and the next
    /// Tab goes on under a Container it named; several are the wildmenu.
    /// Nothing found leaves the line as the expansion left it.
    pub(crate) fn offer(&mut self, from: usize, found: Vec<String>) -> Effect {
        if found.is_empty() {
            return Effect::Moved;
        }
        let alone = found.len() == 1;
        self.wild = Some(Wild { candidates: found, at: 0, from, form: None });
        let effect = self.take();
        if alone {
            self.wild = None;
        }
        effect
    }

    /// The line with its first token written out as the name it stands
    /// for, which Tab does before it completes the argument, so `:w Ca`
    /// Tab is `:write Cargo.toml`.
    pub(crate) fn expand(&mut self, line: &str) {
        self.prompt.line().put(0, line);
    }

    /// Tab and Ctrl-P in the wildmenu: the next candidate or the one
    /// before it, wrapping at either end.
    fn menu(&mut self, forward: bool) -> Effect {
        let Some(wild) = &mut self.wild else { return Effect::Nothing };
        let last = wild.candidates.len();
        let step = if forward { 1 } else { last - 1 };
        wild.at = (wild.at + step) % last;
        self.take()
    }

    /// The line takes the candidate the menu stands on: the whole of
    /// it from the byte it replaces, and the space a completion
    /// leaves after it unless it ends in `/`, where the next Tab goes
    /// on under it.
    fn take(&mut self) -> Effect {
        let Some(wild) = &self.wild else { return Effect::Nothing };
        let text = &wild.candidates[wild.at];
        let end = if text.ends_with('/') { "" } else { " " };
        let (from, word) = (wild.from, alloc::format!("{text}{end}"));
        self.prompt.line().put(from, &word);
        Effect::Moved
    }

    /// The status word while a prompt is open: the line after its
    /// character, and after Tab the one word's argument form and help
    /// or the menu of the several, the current one in brackets.
    pub(crate) fn prompt_status(&self) -> String {
        let line = alloc::format!("{}{}", self.prompt.at, self.prompt.text());
        let mut parts = alloc::vec![line];
        let vi = self.search.options.vi;
        let said = self.wild.as_ref().map(|wild| found(wild, vi));
        parts.extend(said.unwrap_or_default());
        parts.join("  ")
    }

    /// Esc, Ctrl-C, or Backspace on an empty line: the prompt goes and
    /// the mode it was opened from comes back.
    fn leave_prompt(&mut self) -> Effect {
        self.leave_command();
        Effect::Moved
    }

    fn leave_command(&mut self) {
        self.mode = self.prompt.before;
        self.prompt.line().put(0, "");
        self.wild = None;
        self.pending.count = 0;
        self.pending.operator = None;
        self.repeat.record.clear();
    }

    /// The line run: a search for `/` and `?`, else a command.
    fn run(&mut self, line: String) -> Effect {
        self.mode = self.prompt.before;
        let count = core::mem::take(&mut self.pending.count);
        if self.prompt.at != ':' {
            let forward = self.prompt.at == '/';
            // A search is a motion: the want is set where it lands.
            self.keep_want = false;
            return self.searched(line, forward, count.max(1));
        }
        self.repeat.record.clear();
        if !self.macros.fed {
            self.macros.last_ex.clone_from(&line);
        }
        let effect = self.ex(&line);
        self.settle(&effect);
        effect
    }

    /// `/pat` or `?pat` entered: the last pattern unless empty, then
    /// the motion, for the cursor or the operator waiting.
    fn searched(&mut self, line: String, forward: bool, n: usize) -> Effect {
        if !line.is_empty() {
            self.search.pattern = line;
        }
        self.search.forward = forward;
        self.motion(Motion::Search { forward }, n)
    }

    /// One `:` line: the range off the front, and the rest of it.
    ///
    /// What the rest names is the Client's to say, every name a person
    /// can type being a Driver it resolves: a name under Vim's own
    /// `bin` is run there with the range and the bang it read, and a
    /// first token no such name begins is the line for clients/sh,
    /// whole and as it was typed. E492 comes back from there for a
    /// first token PATH has not, as it always did.
    pub(crate) fn ex(&mut self, line: &str) -> Effect {
        let line = line.trim();
        let Some((range, rest)) = self.range(line) else {
            self.error("E20: Mark not set");
            return Effect::Nothing;
        };
        let rest = rest.trim();
        if rest.is_empty() {
            return self.addressed(range, rest.len() < line.len());
        }
        Effect::Line(Ex {
            ranged: rest.len() < line.len(),
            from: range.0 as u64,
            to: range.1 as u64,
            line: String::from(rest),
        })
    }

    /// A `:` line with no word on it: the cursor to the last line the
    /// range names, at the wanted column, where `:12` and `:$` leave
    /// it; nothing at all for a line that named no range either.
    /// nvim's `do_one_cmd` ends a bare address with `beginline(BL_SOL
    /// | BL_FIX)`, which under `nostartofline` (its default) is
    /// `coladvance(curswant)` and leaves `curswant` alone. A bare
    /// address is not a jump: it pushes nothing of its own, so `'` is
    /// not this door's to set.
    fn addressed(&mut self, range: Lines, named: bool) -> Effect {
        if !named {
            return Effect::Moved;
        }
        let at = self.line_at(range.1).unwrap_or(self.cursor);
        self.go_clamped(self.at_screen_column(at, self.want))
    }

    /// `[range]` off the front: `%`, `'a,'b` (`'<,'>` among them, `<`
    /// and `>` being marks like any other), `N`, `.`, `$`, `A,B`; the
    /// current line alone when there is none. `None` for a mark
    /// named that is not set.
    fn range<'a>(&self, line: &'a str) -> Option<(Lines, &'a str)> {
        let here = self.position().0;
        let last = self.line_count() - 1;
        if let Some(rest) = line.strip_prefix('%') {
            return Some(((0, last), rest));
        }
        let mark = |name: char| {
            self.found_mark(name)
                .filter(|(elsewhere, _)| elsewhere.is_none())
                .map(|(_, p)| p.line)
        };
        let (from, rest) = address(line, here, last, mark)?;
        let Some(after_comma) = rest.strip_prefix(',') else {
            return Some(((from, from), rest));
        };
        let (to, rest) = address(after_comma, here, last, mark)?;
        Some(((from.min(to), from.max(to)), rest))
    }

    /// `:set` with its arguments, which `/self/vim/bin/set` runs: the
    /// search options, `no` off, long or short names, `incsearch` taken
    /// and left as nvim has it; E518 for the rest.
    ///
    /// The arguments are the rest of the line, whitespace and all, so
    /// they are split here and not by the shell.
    pub fn set(&mut self, args: &str) -> Effect {
        args.split_whitespace().all(|arg| self.set_one(arg));
        Effect::Moved
    }

    /// `:nohlsearch`, which `/self/vim/bin/nohlsearch` runs: the marks
    /// of the last search stop being drawn until the next one.
    pub const fn nohlsearch(&mut self) -> Effect {
        self.search.stale = true;
        Effect::Moved
    }

    /// One `:set` argument; false with E518 for one not known.
    fn set_one(&mut self, arg: &str) -> bool {
        let (name, on) =
            arg.strip_prefix("no").map_or((arg, true), |n| (n, false));
        if matches!(name, "is" | "incsearch") {
            return true;
        }
        let Some(option) = self.option(name) else {
            self.error(&alloc::format!("E518: Unknown option: {arg}"));
            return false;
        };
        *option = on;
        true
    }

    /// The option `name` names.
    fn option(&mut self, name: &str) -> Option<&mut bool> {
        let options = &mut self.search.options;
        let option = match name {
            "ic" | "ignorecase" => &mut options.ignorecase,
            "scs" | "smartcase" => &mut options.smartcase,
            "hls" | "hlsearch" => &mut options.hlsearch,
            // No short form, and no `compatible`: that is Vim's own
            // option with Vim's own meaning, which this is not.
            "vi" => &mut options.vi,
            _ => return None,
        };
        Some(option)
    }

    /// `:s/pat/rep/[g][i]` over `lines`, which
    /// `/self/vim/bin/substitute` runs: each line's replacements one
    /// edit, bottom-up so no offset moves under another, all one
    /// revision; the cursor on the last line changed.
    ///
    /// `lines` are lines from 0, both ends included, and `spec` is the
    /// rest of the line past the name, spaces and all, which is why it
    /// reaches this parser whole and not as tokens.
    pub fn substitute(&mut self, lines: Lines, spec: &str) -> Effect {
        let parts: Vec<&str> = split_unescaped(spec, '/');
        let (pattern, replacement) = (
            parts.first().copied().unwrap_or(""),
            parts.get(1).copied().unwrap_or(""),
        );
        let flags = parts.get(2).copied().unwrap_or("");
        let (global, ignore) = (flags.contains('g'), flags.contains('i'));
        let source = if pattern.is_empty() {
            self.search.pattern.clone()
        } else {
            pattern.into()
        };
        self.search.pattern.clone_from(&source);
        let options = self.search.options;
        let ignorecase = ignore || (options.ignorecase && !flags.contains('I'));
        let Some(pattern) =
            Pattern::parse(&source, ignorecase, options.smartcase && !ignore)
        else {
            return self.not_found();
        };
        let starts = (lines.0..=lines.1).rev().filter_map(|l| self.line_at(l));
        let changes: Result<Vec<_>, Spent> = starts
            .map(|start| (start, self.line_end(start)))
            .map(|(start, end)| {
                let line = &self.text[start..end];
                let done = substituted(line, &pattern, replacement, global);
                done.map(|text| text.map(|text| (start, end, text)))
            })
            .collect();
        let Ok(changes) = changes else { return self.too_costly() };
        let changes = changes.into_iter().flatten();
        let mut edits = Vec::new();
        // The cursor goes to the last line substituted, after any
        // newline a `\r` brought in; each line changed above moves it.
        // The edit kept for `moved` and `batch_span` is the narrowest
        // one the line's own rebuild admits, not the whole line: `[`
        // and `]` must land on what changed, not on a byte the
        // substitution only carried along unchanged.
        let mut last = None;
        for (start, end, mut text) in changes {
            self.owed_eol(start, end - start, &mut text);
            let edit = minimal_edit(
                start,
                &self.text[start..end],
                &text,
                !edits.is_empty(),
            );
            self.text.replace_range(start..end, &text);
            last = Some(last.map_or_else(
                || start + text.rfind('\n').map_or(0, |i| i + 1),
                |below| below + text.len() - (end - start),
            ));
            edits.push(edit);
        }
        if last.is_none() && self.global.busy {
            return Effect::Nothing;
        }
        let Some(line) = last else { return self.not_found() };
        // Where nvim's `do_sub` ends: `beginline(BL_WHITE | BL_FIX)`,
        // the want set there.
        self.cursor = self.first_non_blank(line);
        self.clamp();
        self.want = self.wanted();
        self.joining = false;
        Effect::Edits(edits)
    }
}

/// `line` with its matches replaced, or `None` when there are none,
/// or [`Spent`] when the matcher never found out. An empty match at
/// the line's end is not one after the first try.
fn substituted(
    line: &str,
    pattern: &Pattern,
    replacement: &str,
    global: bool,
) -> Result<Option<String>, Spent> {
    let mut out = String::new();
    let mut col = 0;
    let mut first = true;
    let mut found = pattern.find(line, col)?;
    while let Some((start, end, caps)) =
        found.filter(|m| first || m.0 < line.len())
    {
        out.push_str(&line[col..start]);
        out.push_str(&expand(replacement, line, &caps));
        first = false;
        let width = line[start..].chars().next().map_or(0, char::len_utf8);
        // An empty match keeps the char it sat on and moves past it.
        out.push_str(if end > start {
            ""
        } else {
            &line[start..start + width]
        });
        col = if end > start { end } else { start + width };
        found = (global && col < line.len())
            .then(|| pattern.find(line, col))
            .transpose()?
            .flatten();
    }
    if first {
        return Ok(None);
    }
    out.push_str(&line[col.min(line.len())..]);
    Ok(Some(out))
}

/// What Tab found, as the status row says it after the line: the usage
/// line of the one name that may run, as its declaration gives it;
/// several as nvim's wildmenu lists them, the one the line holds in
/// brackets.
///
/// This is where the `Tab on the : line` row of [`crate::DIVERGENCES`]
/// is: under `vi` one candidate is listed as several are, with no
/// usage line, which is nvim's way.
fn found(wild: &Wild, vi: bool) -> Vec<String> {
    if let ([_], Some(form)) = (&wild.candidates[..], &wild.form)
        && !vi
    {
        return alloc::vec![form.clone()];
    }
    wild.candidates
        .iter()
        .enumerate()
        .map(|(at, text)| {
            if at == wild.at {
                alloc::format!("[{text}]")
            } else {
                text.clone()
            }
        })
        .collect()
}

/// The options `:set` takes, as Tab writes them out: the long name of
/// each, which [`Vim::option`] knows by this and, where there is one,
/// by its short form. The short forms are not offered, a completion
/// that left `hls` on the line having written nothing out, and neither
/// is `incsearch`, which is taken and does nothing.
const OPTIONS: &[&str] = &["ignorecase", "smartcase", "hlsearch", "vi"];

/// The option names that begin as `word` does, the plain ones before
/// the `no` forms that turn each off.
pub fn options(word: &str) -> Vec<String> {
    let off = OPTIONS.iter().map(|name| alloc::format!("no{name}"));
    let all = OPTIONS.iter().map(ToString::to_string).chain(off);
    all.filter(|name| name.starts_with(word)).collect()
}

/// A `:` line past its range, as Vim's own rule reads it: `name[!] arg`,
/// the argument being everything left.
pub struct Typed<'a> {
    /// The run of letters at the front; empty for a line that begins
    /// with none, which no name of Vim's can be.
    pub name: &'a str,
    /// A `!` straight after the name.
    pub bang: bool,
    /// What follows, trimmed, which is one argument and never tokens:
    /// `:%s/^/> /` and `:w my file` both hold a space. `:s` keeps the
    /// `/` its pattern starts with, which is the one name that needs
    /// no space after it.
    pub arg: &'a str,
}

/// `line` read by that rule: the name, the bang on it, the argument.
pub fn typed(line: &str) -> Typed<'_> {
    let letters = line.bytes().take_while(u8::is_ascii_alphabetic).count();
    let (name, rest) = line.split_at(letters);
    let (bang, rest) =
        rest.strip_prefix('!').map_or((false, rest), |r| (true, r));
    Typed { name, bang, arg: rest.trim() }
}

/// `args` split at `sep`, a backslash escaping one.
fn split_unescaped(args: &str, sep: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut escaped = false;
    for (i, c) in args.char_indices() {
        let cut = c == sep && !escaped;
        parts.extend(cut.then(|| &args[start..i]));
        start = if cut { i + 1 } else { start };
        escaped = c == '\\' && !escaped;
    }
    parts.push(&args[start..]);
    parts
}

/// One address: `.`, `$`, `'x` off `mark`, digits, or nothing for the
/// current line. `None` for a mark named that `mark` does not answer.
fn address(
    line: &str,
    here: usize,
    last: usize,
    mark: impl Fn(char) -> Option<usize>,
) -> Option<(usize, &str)> {
    if let Some(rest) = line.strip_prefix('.') {
        return Some((here, rest));
    }
    if let Some(rest) = line.strip_prefix('$') {
        return Some((last, rest));
    }
    if let Some(rest) = line.strip_prefix('\'') {
        let name = rest.chars().next()?;
        let at = mark(name)?;
        return Some((at.min(last), &rest[name.len_utf8()..]));
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    let number: usize = line[..digits].parse().unwrap_or(here + 1);
    Some((number.saturating_sub(1).min(last), &line[digits..]))
}

#[cfg(test)]
mod tests {
    use alloc::string::String;
    use alloc::vec;

    use reovim_pattern::Pattern;

    use super::{address, split_unescaped, substituted};

    fn subst(line: &str, pat: &str, rep: &str, all: bool) -> Option<String> {
        let pattern = Pattern::parse(pat, false, false).unwrap();
        substituted(line, &pattern, rep, all).expect("a budget to spare")
    }

    #[test]
    fn substitution_replaces_the_first_match_or_every_one() {
        assert_eq!(subst("a a a", "a", "b", false), Some("b a a".into()));
        assert_eq!(subst("a a a", "a", "b", true), Some("b b b".into()));
        assert_eq!(subst("abc", "x", "y", true), None);
        assert_eq!(subst("abc", "", "-", true), Some("-a-b-c".into()));
        assert_eq!(subst("", "^", "> ", true), Some("> ".into()));
        assert_eq!(subst("ab", "$", ";", true), Some("ab;".into()));
        assert_eq!(subst("a,b", ",", "\\r", false), Some("a\nb".into()));
        assert_eq!(subst("héllo", "l", "L", true), Some("héLLo".into()));
        assert_eq!(subst("aaa", "a*", "b", true), Some("b".into()));
    }

    #[test]
    fn a_backslash_keeps_a_separator() {
        assert_eq!(split_unescaped("a/b/c", '/'), vec!["a", "b", "c"]);
        assert_eq!(split_unescaped("a\\/b/c", '/'), vec!["a\\/b", "c"]);
        assert_eq!(split_unescaped("a\\\\/b", '/'), vec!["a\\\\", "b"]);
        assert_eq!(split_unescaped("", '/'), vec![""]);
        assert_eq!(split_unescaped("a/", '/'), vec!["a", ""]);
    }

    #[test]
    fn an_address_is_a_line_from_one_clamped() {
        let none = |_: char| None;
        assert_eq!(address(".x", 4, 9, none), Some((4, "x")));
        assert_eq!(address("$", 4, 9, none), Some((9, "")));
        assert_eq!(address("3,", 4, 9, none), Some((2, ",")));
        assert_eq!(address("0", 4, 9, none), Some((0, "")));
        assert_eq!(address("50", 4, 9, none), Some((9, "")));
        assert_eq!(
            address("s/a/b/", 4, 9, none),
            Some((4, "s/a/b/")),
            "none: this line"
        );
    }

    #[test]
    fn a_quote_address_reads_the_mark_or_answers_none() {
        let set = |c: char| (c == 'a').then_some(2);
        assert_eq!(address("'ax", 4, 9, set), Some((2, "x")));
        assert_eq!(address("'bx", 4, 9, set), None, "b is not set");
    }
}
