//! The command line: `:` and what a person types after it, and the
//! `/` and `?` prompts that share it.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use reovim_client_cli::{Act, Word, Words};

use crate::pattern::{Pattern, expand};
use crate::{Ask, Edit, Effect, Found, Key, Mode, Motion, Quit, Vim, Wild};

/// Lines from 0, both ends included.
type Lines = (usize, usize);

/// Vim's words, in the order a `:` line resolves a name by.
///
/// `:w` is `write` because it stands before `wq`, `:s` is
/// `substitute` before `set`, and `:e` is `edit` before `explore`.
/// The cli's own words follow these, so a name none of these begins
/// is a cli line.
pub const WORDS: &[Word] = &[
    Word {
        name: "write",
        args: "[PATH]",
        help: "the buffer, to PATH if one is given",
    },
    Word { name: "wq", args: "", help: "write the buffer and quit" },
    Word { name: "quit", args: "", help: "quit; ! leaves the draft" },
    Word { name: "xit", args: "", help: "write if it changed, then quit" },
    Word { name: "cquit", args: "", help: "quit with 1, so the caller stops" },
    Word {
        name: "substitute",
        args: "/PAT/REP/[g]",
        help: "PAT becomes REP over the range",
    },
    Word { name: "set", args: "OPTION", help: "an option on, no OPTION off" },
    Word { name: "nohlsearch", args: "", help: "stop marking the last search" },
    Word { name: "edit", args: "PATH", help: "that file, in this region" },
    Word {
        name: "explore",
        args: "[DIR]",
        help: "a listing of DIR, or of the file's own",
    },
];

/// The table a `:` line is read with: Vim's words while Vim is open,
/// then the cli's, which is what makes `:ls` a listing and `:w` a
/// write although the cli has a `write` of its own.
const fn table() -> Words<'static> {
    Words { first: WORDS, then: reovim_client_cli::WORDS }
}

impl Vim {
    /// `:`, `/` or `?`: the prompt opens over an empty line of the
    /// two, the mode to return to kept.
    pub(crate) fn prompt(&mut self, prompt: char, count: usize) -> Effect {
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

    /// One key on the command line, which is the cli's line editor:
    /// its acts, and before them the keys Vim reads itself, the two
    /// ways out of the prompt and the wildmenu's own.
    pub(crate) fn command(&mut self, key: Key) -> Effect {
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
        match self.prompt.line().key(key) {
            Act::Nothing | Act::Clear => Effect::Nothing,
            Act::Redraw => Effect::Moved,
            Act::Complete => self.complete(),
            Act::Submit(line) => self.run(line),
            // Ctrl-C, and Ctrl-D on an empty line: Esc's way out.
            Act::Quit => self.leave_prompt(),
        }
    }

    /// Tab on the `:` line. Nothing for `/` and `?`, which complete
    /// nothing.
    ///
    /// On a line with no space yet: the words the typed one begins,
    /// the first of them on the line, and nothing for a name no table
    /// has. Past the first word: that word expanded to its full name,
    /// so `:w Ca` Tab is `:write Cargo.toml`, and then the argument,
    /// which `:set` answers here and the tree answers through the
    /// Client.
    fn complete(&mut self) -> Effect {
        if self.prompt.at != ':' {
            return Effect::Nothing;
        }
        let text = String::from(self.prompt.text());
        let Some((typed, rest)) = text.split_once(' ') else {
            return self.words(&text);
        };
        let Some(found) = table().resolve(typed) else {
            return Effect::Nothing;
        };
        let line = alloc::format!("{} {rest}", found.name);
        self.prompt.line().put(0, &line);
        let from = line.rfind(' ').map_or(0, |at| at + 1);
        let word = String::from(&line[from..]);
        match found.name {
            "set" => self.offer(from, options(&word)),
            "write" | "edit" | "explore" => {
                Effect::Complete(Ask::Path { from, word })
            }
            // Every other word of Vim's takes no name from the tree.
            name if WORDS.iter().any(|word| word.name == name) => Effect::Moved,
            _ => Effect::Complete(Ask::Cli { line }),
        }
    }

    /// Tab on a line with no space on it: the words the typed one
    /// begins, which the wildmenu walks where there is more than one.
    fn words(&mut self, typed: &str) -> Effect {
        let found = table().starting(typed);
        if found.is_empty() {
            return Effect::Nothing;
        }
        let candidates = found.into_iter().map(Found::Word).collect();
        self.wild = Some(Wild { candidates, at: 0, from: 0 });
        self.take()
    }

    /// What Tab found over the tree, or among the options of `:set`:
    /// one name is simply put on the line, as the cli's `apply` puts
    /// it, since nothing is left to choose between and the next Tab
    /// goes on under a Container it named; several are the wildmenu.
    /// Nothing found leaves the line as the expansion left it.
    pub(crate) fn offer(&mut self, from: usize, found: Vec<String>) -> Effect {
        if found.is_empty() {
            return Effect::Moved;
        }
        let alone = found.len() == 1;
        let candidates = found.into_iter().map(Found::Text).collect();
        self.wild = Some(Wild { candidates, at: 0, from });
        let effect = self.take();
        if alone {
            self.wild = None;
        }
        effect
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
        let text = wild.candidates[wild.at].text();
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
            return self.searched(line, forward, count.max(1));
        }
        self.repeat.record.clear();
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

    /// One `:` line: the range off the front, then the word it names
    /// and what follows that.
    ///
    /// A word of Vim's own is one of these effects, by its full name
    /// or by any beginning of it the table resolves; a word of the
    /// cli's is a line for clients/cli, which the Plugin runs over the
    /// tree, the range being none of its business; a name no table has
    /// is E492, as it always was.
    fn ex(&mut self, line: &str) -> Effect {
        let line = line.trim();
        let (range, rest) = self.range(line);
        let command = rest.trim();
        if command.is_empty() {
            return self.addressed(range, rest.len() < line.len());
        }
        let Typed { word, bang, arg } = typed(command);
        let Some(found) = table().resolve(word) else {
            self.message =
                alloc::format!("E492: Not an editor command: {command}");
            return Effect::Moved;
        };
        match found.name {
            "write" if arg.is_empty() => Effect::Save(bang),
            "write" => Effect::SaveAs(arg.into(), bang),
            "wq" | "xit" => Effect::Quit(Quit::Write),
            "quit" if bang => Effect::Quit(Quit::Force),
            "quit" => Effect::Quit(Quit::Ask),
            "cquit" => Effect::Quit(Quit::Abort),
            // `:s` writes its pattern straight after the word.
            "substitute" => {
                self.substitute(range, arg.strip_prefix('/').unwrap_or(arg))
            }
            "set" => self.set(arg),
            "nohlsearch" => {
                self.search.stale = true;
                Effect::Moved
            }
            // `:e` without a name would reload the file; nothing here.
            "edit" if arg.is_empty() => Effect::Moved,
            "edit" => Effect::Open(arg.into(), bang),
            "explore" => {
                Effect::Explore((!arg.is_empty()).then(|| arg.into()), bang)
            }
            name => Effect::Cli(line_of(name, arg)),
        }
    }

    /// A `:` line with no word on it: the cursor to the last line the
    /// range names, at its first non-blank, where `:12` and `:$` leave
    /// it; nothing at all for a line that named no range either.
    fn addressed(&mut self, range: Lines, named: bool) -> Effect {
        if !named {
            return Effect::Moved;
        }
        let at = self.line_at(range.1).unwrap_or(self.cursor);
        self.go_clamped(self.first_non_blank(at))
    }

    /// `[range]` off the front: `%`, `'<,'>`, `N`, `.`, `$`, `A,B`;
    /// the current line alone when there is none.
    fn range<'a>(&self, line: &'a str) -> (Lines, &'a str) {
        let here = self.position().0;
        let last = self.line_count() - 1;
        if let Some(rest) = line.strip_prefix('%') {
            return ((0, last), rest);
        }
        if let Some(rest) = line.strip_prefix("'<,'>") {
            return (self.visual_lines().unwrap_or((here, here)), rest);
        }
        let (from, rest) = address(line, here, last);
        let Some(after_comma) = rest.strip_prefix(',') else {
            return ((from, from), rest);
        };
        let (to, rest) = address(after_comma, here, last);
        ((from.min(to), from.max(to)), rest)
    }

    /// The lines of the last selection.
    fn visual_lines(&self) -> Option<Lines> {
        let last = self.line_count() - 1;
        let v = self.last_visual?;
        let (a, c) =
            (self.position_of(v.anchor).0, self.position_of(v.cursor).0);
        Some((a.min(c).min(last), a.max(c).min(last)))
    }

    /// `:set` with its arguments: the search options, `no` off, long
    /// or short names, `incsearch` taken and left as nvim has it;
    /// E518 for the rest.
    fn set(&mut self, args: &str) -> Effect {
        args.split_whitespace().all(|arg| self.set_one(arg));
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
            self.message = alloc::format!("E518: Unknown option: {arg}");
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

    /// `:s/pat/rep/[g][i]` over `lines`: each line's replacements one
    /// edit, bottom-up so no offset moves under another, all one
    /// revision; the cursor on the last line changed.
    fn substitute(&mut self, lines: Lines, args: &str) -> Effect {
        let parts: Vec<&str> = split_unescaped(args, '/');
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
        let changes: Vec<(usize, usize, String)> = starts
            .map(|start| (start, self.line_end(start)))
            .filter_map(|(start, end)| {
                let line = &self.text[start..end];
                substituted(line, &pattern, replacement, global)
                    .map(|text| (start, end, text))
            })
            .collect();
        let mut edits = Vec::new();
        // The cursor goes to the last line substituted, after any
        // newline a `\r` brought in; each line changed above moves it.
        let mut last = None;
        for (start, end, text) in changes {
            self.text.replace_range(start..end, &text);
            last = Some(last.map_or_else(
                || start + text.rfind('\n').map_or(0, |i| i + 1),
                |below| below + text.len() - (end - start),
            ));
            let join = !edits.is_empty();
            edits.push(Edit { at: start, len: end - start, text, join });
        }
        let Some(line) = last else { return self.not_found() };
        self.cursor = self.first_non_blank(line);
        self.clamp();
        self.joining = false;
        Effect::Edits(edits)
    }
}

/// `line` with its matches replaced, or `None` when there are none.
/// An empty match at the line's end is not one after the first try.
fn substituted(
    line: &str,
    pattern: &Pattern,
    replacement: &str,
    global: bool,
) -> Option<String> {
    let mut out = String::new();
    let mut col = 0;
    let mut first = true;
    let mut found = pattern.find(line, col);
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
            .flatten();
    }
    if first {
        return None;
    }
    out.push_str(&line[col.min(line.len())..]);
    Some(out)
}

/// What Tab found, as the status row says it after the line: the one
/// word's argument form, where it takes any, and its help; several as
/// nvim's wildmenu lists them, the one the line holds in brackets.
///
/// This is where the `Tab on the : line` row of [`crate::DIVERGENCES`]
/// is: under `vi` one word is listed as several are, with no argument
/// form and no help, which is nvim's way.
fn found(wild: &Wild, vi: bool) -> Vec<String> {
    if let [Found::Word(one)] = &wild.candidates[..]
        && !vi
    {
        let args = (!one.args.is_empty()).then(|| one.args.to_string());
        return args.into_iter().chain([one.help.to_string()]).collect();
    }
    wild.candidates
        .iter()
        .enumerate()
        .map(|(at, found)| {
            let text = found.text();
            if at == wild.at {
                alloc::format!("[{text}]")
            } else {
                text.to_string()
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
fn options(word: &str) -> Vec<String> {
    let off = OPTIONS.iter().map(|name| alloc::format!("no{name}"));
    let all = OPTIONS.iter().map(ToString::to_string).chain(off);
    all.filter(|name| name.starts_with(word)).collect()
}

/// A `:` line past its range, as the table reads it.
struct Typed<'a> {
    /// The run of letters at the front; empty for a line that names
    /// no word at all.
    word: &'a str,
    /// A `!` straight after the word.
    bang: bool,
    /// What follows, trimmed; `:s` keeps the `/` its pattern starts
    /// with, which is the one word that needs no space.
    arg: &'a str,
}

/// `command` read: the word, the bang on it, and the argument.
fn typed(command: &str) -> Typed<'_> {
    let letters = command.bytes().take_while(u8::is_ascii_alphabetic).count();
    let (word, rest) = command.split_at(letters);
    let (bang, rest) =
        rest.strip_prefix('!').map_or((false, rest), |r| (true, r));
    Typed { word, bang, arg: rest.trim() }
}

/// A cli word and its argument as a command line: the word by its
/// full name, however short the beginning typed was, and what follows
/// split as the cli splits a line.
fn line_of(name: &str, arg: &str) -> Vec<String> {
    let mut words = alloc::vec![name.to_string()];
    words.extend(arg.split_whitespace().map(ToString::to_string));
    words
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

/// One address: `.`, `$`, digits, or nothing for the current line.
fn address(line: &str, here: usize, last: usize) -> (usize, &str) {
    if let Some(rest) = line.strip_prefix('.') {
        return (here, rest);
    }
    if let Some(rest) = line.strip_prefix('$') {
        return (last, rest);
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    let number: usize = line[..digits].parse().unwrap_or(here + 1);
    (number.saturating_sub(1).min(last), &line[digits..])
}

#[cfg(test)]
mod tests {
    use alloc::string::String;
    use alloc::vec;

    use super::{address, split_unescaped, substituted};
    use crate::pattern::Pattern;

    fn subst(line: &str, pat: &str, rep: &str, all: bool) -> Option<String> {
        let pattern = Pattern::parse(pat, false, false).unwrap();
        substituted(line, &pattern, rep, all)
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
        assert_eq!(address(".x", 4, 9), (4, "x"));
        assert_eq!(address("$", 4, 9), (9, ""));
        assert_eq!(address("3,", 4, 9), (2, ","));
        assert_eq!(address("0", 4, 9), (0, ""));
        assert_eq!(address("50", 4, 9), (9, ""));
        assert_eq!(address("s/a/b/", 4, 9), (4, "s/a/b/"), "none: this line");
    }
}
