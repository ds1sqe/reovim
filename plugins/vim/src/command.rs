//! The command line: `:` and what a person types after it, and the
//! `/` and `?` prompts that share it.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use reovim_client_sh::Act;
use reovim_pattern::{Pattern, Spent, expand};

use crate::{Command, Effect, Key, Mode, Motion, Vim, Wild, minimal_edit};

/// Lines from 0, both ends included.
type Lines = (usize, usize);

impl Vim {
    /// `:`, `/` or `?`: the prompt opens over an empty line of the
    /// two, the mode to return to kept, and the want with it: nvim's
    /// `nv_colon` and `nv_search` leave `curswant` alone.
    ///
    /// A `/` or `?` line opens a preview with it, which holds the
    /// cursor and the first line shown as they stand now: every way
    /// out of the line puts them back, and the match the line finds
    /// as it is typed moves them meanwhile. A `:` line has none.
    pub(crate) fn prompt(&mut self, prompt: char, count: usize) -> Effect {
        self.keep_want = true;
        self.prompt.at = prompt;
        self.prompt.line().put(0, "");
        // `:` from a selection is nvim's OP_COLON operator, not a
        // prompt over it: `do_pending_operator` puts the cursor at
        // the selection's start and sets the want there, and only
        // then does `op_colon` put `'<,'>` on the line. The marks
        // `<` and `>` are still the selection's, so it is left first.
        if prompt == ':'
            && let Mode::Visual(kind) = self.mode
        {
            let start = self.selection_start(kind);
            self.leave_visual();
            self.cursor = start;
            self.clamp();
            self.keep_want = false;
            self.prompt.line().put(0, "'<,'>");
        }
        self.preview = (prompt != ':').then_some(crate::search::Preview {
            cursor: self.cursor,
            top: self.top,
            at: None,
        });
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
            // The text or the cursor of a `/` or `?` line moved, so
            // the preview follows what stands on it now.
            Act::Redraw if self.prompt.at != ':' => self.previewed(),
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
    pub fn listed(
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
    pub fn offer(&mut self, from: usize, found: Vec<String>) -> Effect {
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
    pub fn expand(&mut self, line: &str) {
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

    /// Esc, Ctrl-C, or Backspace on an empty line: the prompt goes,
    /// the mode it was opened from comes back, and a `/` or `?`
    /// line's cursor and window go back where it was opened.
    fn leave_prompt(&mut self) -> Effect {
        self.leave_command();
        self.unpreviewed();
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
    ///
    /// A `/` or `?` line's preview goes first, cursor and window with
    /// it, so the motion below starts where the line was opened. The
    /// list the preview began stands, so the motion asks for no other.
    fn run(&mut self, line: String) -> Effect {
        self.mode = self.prompt.before;
        self.unpreviewed();
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

    /// `search [--backward] [PATTERN]`, the name on PATH whose line
    /// the `/` and `?` keys are one door of: a pattern is searched
    /// for from the cursor exactly as a typed line's is — the same
    /// motion, the same view, and the pattern the last one after it,
    /// so `n` and `N` and `hlsearch` follow it. No pattern opens the
    /// prompt those keys open, which is what makes a key bound to
    /// this name the `/` key.
    pub fn searching(&mut self, pattern: &str, backward: bool) -> Effect {
        let forward = !backward;
        let at = if forward { '/' } else { '?' };
        if pattern.is_empty() {
            return self.prompt(at, 0);
        }
        // A search is a motion: the want is set where it lands, as
        // `run` sets it for the line a person typed.
        self.keep_want = false;
        self.searched(String::from(pattern), forward, 1)
    }

    /// One line of `init`, which no key typed: the `:` line it is,
    /// run apart from whatever ran before it, so its edits begin a
    /// revision of their own rather than joining a replay's.
    pub fn init(&mut self, line: &str) -> Effect {
        self.macros.fed = false;
        self.macros.joined = false;
        self.ex(line)
    }

    /// One `:` line: the address text off the front, the name and the
    /// bang past it, and the rest — Vim's own Ex grammar, read and not
    /// resolved. A name with none of it, `%`, `5,10`, `'a,'b`, `.`,
    /// `$`, `'<,'>`, `/pat/` and their offsets included, is put back
    /// as `--address`, quoted where it holds a blank; the bang as
    /// `--bang`; what the rest of Vim's `bin` sees is the same door a
    /// typed one runs through. A first token that reads as no name —
    /// no run of letters at its front — is nothing of Vim's own
    /// grammar and goes to the shell exactly as it stood, address and
    /// all: `:ls /module | cat` becomes itself.
    ///
    /// An address with nothing past it, `:12`, `:'a`, stays the
    /// engine's own cursor move; a bare `:` with nothing at all moves
    /// nothing. E20 for a mark not set is the engine's to say either
    /// way, at the one resolver [`Vim::lines_of`] both this and every
    /// bin call.
    pub(crate) fn ex(&mut self, line: &str) -> Effect {
        let line = line.trim();
        let (address, rest) = address_span(line);
        let rest = rest.trim();
        if rest.is_empty() {
            return self.addressed(address);
        }
        let Typed { name, bang, arg } = typed(rest);
        if name.is_empty() {
            return Effect::Command(Command { line: String::from(rest) });
        }
        let mut built = String::from(name);
        if bang {
            built.push_str(" --bang");
        }
        if !address.is_empty() {
            built.push_str(" --address ");
            built.push_str(&quoted(address));
        }
        if !arg.is_empty() {
            built.push(' ');
            built.push_str(arg);
        }
        Effect::Command(Command { line: built })
    }

    /// A `:` line with no word on it: the cursor to the last line the
    /// address names, at the wanted column, where `:12` and `:$` leave
    /// it; nothing at all for a line that named no address either.
    /// nvim's `do_one_cmd` ends a bare address with `beginline(BL_SOL
    /// | BL_FIX)`, which under `nostartofline` (its default) is
    /// `coladvance(curswant)` and leaves `curswant` alone. A bare
    /// address is not a jump: it pushes nothing of its own, so `'` is
    /// not this door's to set.
    fn addressed(&mut self, address: &str) -> Effect {
        if address.is_empty() {
            return Effect::Moved;
        }
        let range = match self.lines_of(address) {
            Ok(range) => range,
            Err(effect) => return effect,
        };
        let at = self.line_at(range.1).unwrap_or(self.cursor);
        self.go_clamped(self.at_screen_column(at, self.want))
    }

    /// The one resolver a bin's own `address: utf8` text and the
    /// engine's own bare address both read through: `%`, `'a,'b`
    /// (`'<,'>` among them, `<` and `>` being marks like any other),
    /// `N`, `.`, `$`, `/pat/`, `?pat?`, each item's own `+`/`-`
    /// offsets, and up to one `,` or `;` joining a second item —
    /// `;` moving the base the second item and its offsets read from
    /// to the first item's own line, `,` leaving it the cursor's.
    /// `E20: Mark not set` for a mark not set, said here and not
    /// merely answered, since a bin's own call has nowhere else to
    /// say it; `E486` the same way for a pattern not found.
    ///
    /// # Errors
    /// The `Effect` the failure already said, `Nothing` in every case
    /// today.
    pub fn lines_of(&mut self, address: &str) -> Result<Lines, Effect> {
        if address == "%" {
            return Ok((0, self.line_count().saturating_sub(1)));
        }
        let here = self.position().0;
        let last = self.line_count().saturating_sub(1);
        let split = item_span(address);
        let (first, rest) = address.split_at(split);
        let from = self.resolved_item(first, here, last)?;
        if let Some(rest) = rest.strip_prefix(';') {
            let to = self.resolved_item(rest, from, last)?;
            return Ok((from.min(to), from.max(to)));
        }
        let Some(rest) = rest.strip_prefix(',') else {
            return Ok((from, from));
        };
        let to = self.resolved_item(rest, here, last)?;
        Ok((from.min(to), from.max(to)))
    }

    /// The text of the lines `lines` names, each ended by its own
    /// newline as a file's lines are: what `:{range}w FILE` writes.
    /// A last line the text ends without one still gets one here,
    /// which is what nvim writes under `fixeol`.
    #[must_use]
    pub fn text_of(&self, (first, last): Lines) -> String {
        let Some(from) = self.line_at(first) else {
            return String::new();
        };
        let to =
            self.line_at(last).map_or(self.text.len(), |at| self.line_end(at));
        let mut text = String::from(&self.text[from..to]);
        text.push('\n');
        text
    }

    /// One address item: its base, `here` the line relative addresses
    /// and an empty base read from, then its own `+`/`-` offsets,
    /// clamped to the text.
    fn resolved_item(
        &mut self,
        text: &str,
        here: usize,
        last: usize,
    ) -> Result<usize, Effect> {
        let (base, rest) = self.resolved_base(text, here, last)?;
        let offset = offset_of(rest);
        Ok(base.saturating_add_signed(offset).min(last))
    }

    /// One address base and what follows it past the base alone: `.`,
    /// `$`, `'x`, `/pat/`, `?pat?`, digits, or `here` for none of
    /// them, which is every address item with an empty base — the
    /// second of `5;+2` among them.
    fn resolved_base<'a>(
        &mut self,
        text: &'a str,
        here: usize,
        last: usize,
    ) -> Result<(usize, &'a str), Effect> {
        if let Some(rest) = text.strip_prefix('.') {
            return Ok((here, rest));
        }
        if let Some(rest) = text.strip_prefix('$') {
            return Ok((last, rest));
        }
        if let Some(rest) = text.strip_prefix('\'') {
            return self.resolved_mark(rest, last);
        }
        if text.starts_with(['/', '?']) {
            return self.resolved_pattern(text, here, last);
        }
        let digits = text.bytes().take_while(u8::is_ascii_digit).count();
        let number: usize = text[..digits].parse().unwrap_or(here + 1);
        Ok((number.saturating_sub(1).min(last), &text[digits..]))
    }

    /// `'x`: the mark's own line, `E20` for one not set or one in
    /// another buffer, which a range cannot cross.
    fn resolved_mark<'a>(
        &mut self,
        rest: &'a str,
        last: usize,
    ) -> Result<(usize, &'a str), Effect> {
        let name = rest.chars().next().unwrap_or('\'');
        let found = self
            .found_mark(name)
            .filter(|(elsewhere, _)| elsewhere.is_none())
            .map(|(_, p)| p.line);
        let Some(line) = found else {
            self.error("E20: Mark not set");
            return Err(Effect::Nothing);
        };
        Ok((line.min(last), &rest[name.len_utf8()..]))
    }

    /// `/pat/` or `?pat?`: the next (`/`) or previous (`?`) line past
    /// `here` the pattern matches, wrapping, and never `here` itself —
    /// nvim's own "next line" and "previous line". An empty pattern is
    /// the last one searched, which this also becomes for the next
    /// empty one.
    fn resolved_pattern<'a>(
        &mut self,
        text: &'a str,
        here: usize,
        last: usize,
    ) -> Result<(usize, &'a str), Effect> {
        let forward = text.starts_with('/');
        let delimiter = if forward { '/' } else { '?' };
        let span = pattern_span(text);
        let body = &text[1..span];
        let pattern = body.strip_suffix(delimiter).unwrap_or(body);
        let source = if pattern.is_empty() {
            self.search.pattern.clone()
        } else {
            String::from(pattern)
        };
        self.search.pattern.clone_from(&source);
        let options = self.search.options;
        let Some(compiled) =
            Pattern::parse(&source, options.ignorecase, options.smartcase)
        else {
            return Err(self.not_found());
        };
        let line = self.scanned(&compiled, here, last, forward)?;
        Ok((line, &text[span..]))
    }

    /// One line the pattern matches, walking from `here` forward or
    /// back and wrapping at either end; `E486` where none does.
    fn scanned(
        &mut self,
        pattern: &Pattern,
        here: usize,
        last: usize,
        forward: bool,
    ) -> Result<usize, Effect> {
        // nvim's own rule: "next line" and "previous line" never
        // include a match on `here` itself, wrapped around to or not.
        let order: Vec<usize> = if forward {
            (here + 1..=last).chain(0..here).collect()
        } else {
            (0..here).rev().chain((here + 1..=last).rev()).collect()
        };
        self.first_matching(pattern, &order)
    }

    /// The first of `order` `pattern` matches; `E486` for none of them.
    fn first_matching(
        &mut self,
        pattern: &Pattern,
        order: &[usize],
    ) -> Result<usize, Effect> {
        let Some((&line, rest)) = order.split_first() else {
            return Err(self.not_found());
        };
        if self.matches_line(pattern, line)? {
            return Ok(line);
        }
        self.first_matching(pattern, rest)
    }

    /// Whether `pattern` matches somewhere on `line`'s own text.
    fn matches_line(
        &mut self,
        pattern: &Pattern,
        line: usize,
    ) -> Result<bool, Effect> {
        let start = self.line_at(line).unwrap_or(0);
        let end = self.line_end(start);
        match pattern.find(&self.text[start..end], 0) {
            Ok(found) => Ok(found.is_some()),
            Err(Spent) => Err(self.too_costly()),
        }
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
    ///
    /// `NAME=VALUE` is read first, as nvim reads it: `leader` takes a
    /// VALUE the notation spells at least one key with, kept as
    /// typed, and a switch takes no value at all. `leader` with no
    /// value, and `noleader`, are E474 the same way.
    fn set_one(&mut self, arg: &str) -> bool {
        if let Some((name, value)) = arg.split_once('=') {
            return self.set_value(arg, name, value);
        }
        let (name, on) =
            arg.strip_prefix("no").map_or((arg, true), |n| (n, false));
        if matches!(name, "is" | "incsearch") {
            return true;
        }
        if name == "leader" {
            self.refused(arg);
            return false;
        }
        let Some(option) = self.option(name) else {
            self.error(&alloc::format!("E518: Unknown option: {arg}"));
            return false;
        };
        *option = on;
        true
    }

    /// `NAME=VALUE`: the leader as it was typed, E474 for a name that
    /// takes no value, E518 for one that is no option at all.
    fn set_value(&mut self, arg: &str, name: &str, value: &str) -> bool {
        if name == "leader" && self.keymap.read(value).is_some() {
            self.keymap.leader = String::from(value);
            return true;
        }
        if name == "leader" {
            self.refused(arg);
            return false;
        }
        let bare = name.strip_prefix("no").unwrap_or(name);
        if matches!(bare, "is" | "incsearch") || self.option(bare).is_some() {
            self.refused(arg);
            return false;
        }
        self.error(&alloc::format!("E518: Unknown option: {arg}"));
        false
    }

    /// What `/self/vim/options/NAME` reads, one of the five: a switch
    /// as `0` or `1`, the leader as `set leader` took it, and a
    /// newline. `None` for a name that is none of them.
    ///
    /// The long names alone: a cell is the option's one reading, not
    /// the `:set` spelling, which takes the short forms too.
    #[must_use]
    pub fn shown(&self, name: &str) -> Option<String> {
        let options = &self.search.options;
        let on = match name {
            "ignorecase" => options.ignorecase,
            "smartcase" => options.smartcase,
            "hlsearch" => options.hlsearch,
            "vi" => options.vi,
            "leader" => {
                return Some(alloc::format!("{}\n", self.keymap.leader));
            }
            _ => return None,
        };
        Some(alloc::format!("{}\n", u8::from(on)))
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
/// the argument being everything left. [`ex`]'s own reading and
/// nothing the engine hands out: a caller outside reads a line by
/// running it.
struct Typed<'a> {
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
fn typed(line: &str) -> Typed<'_> {
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

/// The address text at the front of `line` as Vim's own grammar reads
/// it — syntax alone, nothing resolved — and what follows it. `%`
/// stands apart, a whole range and not an item with offsets of its
/// own; else one item, then up to one `,`- or `;`-joined second one,
/// each `item_span` on its own.
fn address_span(line: &str) -> (&str, &str) {
    if line.starts_with('%') {
        return line.split_at(1);
    }
    let mut at = item_span(line);
    if let Some(rest) = line[at..].strip_prefix([',', ';']) {
        at += 1;
        at += item_span(rest);
    }
    line.split_at(at)
}

/// Whether `line` opens with an address of its own, as [`address_span`]
/// reads it — what a bound line already carrying `'<,'>` or any other
/// address is told from one that does not.
///
/// `mod command` is private, so this reaches no caller outside the
/// crate however it is spelled; `pub(crate)` and `pub(super)` are
/// both `redundant_pub_crate` here, which the workspace denies.
pub fn addressed_by(line: &str) -> bool {
    !address_span(line).0.is_empty()
}

/// One address item's own span in bytes: a base — `.`, `$`, `'x`,
/// `/pat/`, `?pat?`, digits, or nothing, which is every base an
/// address item may have — then its own run of `+`/`-` offsets.
/// Nothing of this reads a mark or runs a search: it is syntax, read
/// once more at [`Vim::lines_of`] to resolve it.
fn item_span(text: &str) -> usize {
    let base = if text.starts_with(['.', '$']) {
        1
    } else if let Some(rest) = text.strip_prefix('\'') {
        1 + rest.chars().next().map_or(0, char::len_utf8)
    } else if text.starts_with(['/', '?']) {
        pattern_span(text)
    } else {
        text.bytes().take_while(u8::is_ascii_digit).count()
    };
    base + offset_span(&text[base..])
}

/// `/pat/` or `?pat?`'s own span: the delimiter, the pattern up to
/// the next unescaped delimiter, and that delimiter where there is
/// one — nvim's own rule, the closing mark optional only where
/// nothing follows the pattern in this address item.
fn pattern_span(text: &str) -> usize {
    let delimiter = text.as_bytes()[0] as char;
    let mut escaped = false;
    for (i, c) in text.char_indices().skip(1) {
        if c == delimiter && !escaped {
            return i + delimiter.len_utf8();
        }
        escaped = c == '\\' && !escaped;
    }
    text.len()
}

/// A run of `+`/`-` offsets' own span in bytes, each an optional
/// digit run, so `+`, `+2`, `++`, `+2-1` are each one span.
fn offset_span(text: &str) -> usize {
    let mut at = 0;
    while let Some(rest) = text[at..].strip_prefix(['+', '-']) {
        at += 1;
        at += rest.bytes().take_while(u8::is_ascii_digit).count();
    }
    at
}

/// A run of `+`/`-` offsets read as the one number they sum to: no
/// digits after a sign is `1` for it, as nvim reads `'x++` and `'x+2`
/// the same.
fn offset_of(text: &str) -> isize {
    let mut at = 0;
    let mut total: isize = 0;
    while let Some(rest) = text[at..].strip_prefix(['+', '-']) {
        let sign: isize = if text[at..].starts_with('+') { 1 } else { -1 };
        at += 1;
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        let n: isize = rest[..digits].parse().unwrap_or(1);
        total += sign * n;
        at += digits;
    }
    total
}

/// `text` quoted by the shell's own rule where it holds a blank, so a
/// `--address` flag whose text does is one token still; `text` itself
/// where it holds none, an address rarely doing.
fn quoted(text: &str) -> String {
    // A mark's own `'` is as much the shell's to misread as a blank:
    // `'<,'>` holds two, and either quote char, blank or not, must be
    // quoted so `words` reads the address as one token and not the
    // marks as an opened and closed run of its own.
    if !text.contains(char::is_whitespace) && !text.contains(['\'', '"']) {
        return String::from(text);
    }
    if !text.contains('\'') {
        return format!("'{text}'");
    }
    if !text.contains('"') {
        return format!("\"{text}\"");
    }
    // Both quote chars, and the shell's reading holds no backslash to
    // escape either: one token is written as quoted runs side by side,
    // which `words` joins back, every `'` its own `"'"` between them.
    let mut out = String::new();
    let run = |out: &mut String, part: &str| {
        if !part.is_empty() {
            out.push('\'');
            out.push_str(part);
            out.push('\'');
        }
    };
    let mut runs = text.split('\'');
    if let Some(first) = runs.next() {
        run(&mut out, first);
    }
    for part in runs {
        out.push_str("\"'\"");
        run(&mut out, part);
    }
    out
}

#[cfg(test)]
mod tests {
    use alloc::string::String;
    use alloc::vec;

    use reovim_pattern::Pattern;

    use super::{
        address_span, offset_of, quoted, split_unescaped, substituted,
    };

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

    /// The shell reads a token by its quotes and no backslash of any:
    /// an address that holds one quote char is wrapped in the other,
    /// and one that holds both is written as runs side by side, which
    /// `words` joins back into the one token it was.
    #[test]
    fn an_address_is_one_token_whatever_quotes_it_holds() {
        assert_eq!(quoted("5,10"), "5,10", "nothing to quote");
        assert_eq!(quoted("'<,'>"), "\"'<,'>\"");
        assert_eq!(quoted("/a b/"), "'/a b/'");
        assert_eq!(quoted("/\"x\"/"), "'/\"x\"/'");
        assert_eq!(quoted("/it's \"x\"/"), "'/it'\"'\"'s \"x\"/'");
    }

    #[test]
    fn the_address_span_is_syntax_alone_nothing_resolved() {
        assert_eq!(address_span(".x"), (".", "x"));
        assert_eq!(address_span("$"), ("$", ""));
        assert_eq!(address_span("3,"), ("3,", ""));
        assert_eq!(address_span("50s/a/b/"), ("50", "s/a/b/"));
        assert_eq!(address_span("s/a/b/"), ("", "s/a/b/"), "no digit: no item");
        assert_eq!(address_span("%s"), ("%", "s"), "% is a whole range alone");
        assert_eq!(address_span("'a,'bs/x/y/"), ("'a,'b", "s/x/y/"));
        assert_eq!(address_span("'<,'>d"), ("'<,'>", "d"));
        assert_eq!(address_span("5;+2d"), ("5;+2", "d"));
        assert_eq!(address_span("/x/,$d"), ("/x/,$", "d"));
        assert_eq!(address_span("/x\\/y/d"), ("/x\\/y/", "d"), "escaped /");
        assert_eq!(address_span("12"), ("12", ""));
    }

    #[test]
    fn offsets_sum_a_bare_sign_counting_one() {
        assert_eq!(offset_of(""), 0);
        assert_eq!(offset_of("+2"), 2);
        assert_eq!(offset_of("-2"), -2);
        assert_eq!(offset_of("++"), 2);
        assert_eq!(offset_of("+2-1"), 1);
    }
}
