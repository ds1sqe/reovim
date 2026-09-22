#![no_std]
#![forbid(unsafe_code)]
//! The shell: a typed line in, text out, over a `Calls`. It names no
//! Driver and no side: the tree it runs on is whatever its host hands
//! it. The first token is a name under PATH, whose declaration says
//! what the rest of the line fills; the run answers with an output cell
//! the shell reads to its end.
//!
//! What it owns is what changes the shell and not the tree: the line
//! and its editing, where the line stands (`cd`, `pwd`), PATH, `help`,
//! `>` and `>>`, Tab. A line that does not run
//! gives a [`Miss`]: the name, what the cause is about, and the shell's
//! sentence for it, which the host prints as one line.

extern crate alloc;

mod complete;
mod decode;
mod line;
mod run;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;
use core::fmt::Write as _;

use reovim_kernel_interface::{Calls, Error, Flags, Rd};
use reovim_sdk::dynamic::Value;
use reovim_sdk::ops;
use reovim_sdk::walk::{self, ROOT};

pub use complete::{Completion, complete};
pub use line::{Act, Line};
pub use run::{Open, Ran};

/// One name the shell runs itself, which names no rd: what it is
/// called, what it takes after it, and the one line `help` says of it.
struct Builtin {
    name: &'static str,
    args: &'static str,
    help: &'static str,
}

/// The column a usage line's help begins at.
const HELP: usize = 32;

/// The names the shell runs itself, in the order `help` lists them:
/// where the line stands, and `help` itself. Everything else is a name
/// under PATH.
const BUILTINS: [Builtin; 3] = [
    Builtin {
        name: "cd",
        args: "[target]",
        help: "go there; none is /, - the one before",
    },
    Builtin { name: "pwd", args: "", help: "where the line stands now" },
    Builtin {
        name: "help",
        args: "[name]",
        help: "these lines, or the one for name",
    },
];

/// What `help` says after the lines: what a target may be, and the keys
/// the line takes, which belong to no one name.
const AFTER: &str = "\
a target is a path from /, a name under the cwd, . or .., or rd:<n>

single and double quotes hold blanks, | and >; no escape

the line: Tab completes, Up and Down walk the lines that begin as this
one does, Ctrl-A the start, Ctrl-E the end, Ctrl-U cuts to the start,
Ctrl-W the word before the cursor, Ctrl-L clears the screen; Ctrl-D
deletes under the cursor, and ends the session on an empty line, as
Ctrl-C does";

impl Builtin {
    /// What it takes, as a usage line writes it.
    fn form(&self) -> String {
        if self.args.is_empty() {
            return self.name.to_string();
        }
        format!("{} {}", self.name, self.args)
    }

    /// The line `help` lists it on: the form, then the help at [`HELP`].
    fn line(&self) -> String {
        let form = self.form();
        let pad = HELP.saturating_sub(form.chars().count());
        format!("{form}{}{}", " ".repeat(pad), self.help)
    }
}

/// The builtin of that name, which is the only one the shell runs: a
/// name PATH holds is reached by lookup and never by this table.
fn builtin(name: &str) -> Option<&'static Builtin> {
    BUILTINS.iter().find(|entry| entry.name == name)
}

/// Where the line stands in the tree, and the environment its host set.
///
/// The rd a relative target is read from, the path the prompt and `pwd`
/// show, the one before it, which `cd -` goes back to, and what the host
/// put there for a run to read. The host holds one for a session of
/// lines; a Plugin's stands where the Plugin does.
#[derive(Clone, Debug)]
pub struct Shell {
    cwd: Rd,
    path: String,
    back: Option<(Rd, String)>,
    /// What the host gave, by name: a field declared with this name and
    /// this value's type is filled from here and no token matches it.
    env: Vec<(String, Value)>,
    /// The field names the host fills by name alone, as
    /// [`Shell::option`] says them: no positional token reaches one.
    options: Vec<String>,
    /// Where a name resolves, in the order it does: this Client's own
    /// userland, then the server's, unless [`Shell::first`] put one
    /// ahead of them, as a host with names of its own does with its own
    /// directory. A directory that is not there is skipped, so a Client
    /// with no Host of its own still works.
    dirs: Vec<String>,
}

impl Default for Shell {
    /// The root, where every session starts, on a screen of no width.
    fn default() -> Self {
        Self::at(ROOT, "/".to_string())
    }
}

impl Shell {
    /// A line standing at `rd`, which is at `path`: what a Plugin gives
    /// the lines it runs, so a relative target of theirs is read from
    /// where the Plugin is and not from the root.
    #[must_use]
    pub fn at(cwd: Rd, path: String) -> Self {
        let dirs = alloc::vec!["/self/bin".to_string(), "/bin".to_string()];
        let env = Vec::new();
        let options = Vec::new();
        let mut shell = Self { cwd, path, back: None, env, options, dirs };
        shell.screen(0, false);
        shell.input(ROOT);
        shell
    }

    /// `dir` first to resolve, ahead of every directory here already: a
    /// host with names of its own puts its directory in front of its
    /// Shell's this way, so one of its names and one of the Client's or
    /// the server's never clash.
    pub fn first(&mut self, dir: &str) {
        self.dirs.insert(0, dir.to_string());
    }

    /// The screen a run writes for: the columns a listing lays its names
    /// out across, 0 for a pipe, and whether the text is painted. The
    /// host knows both, and this crate asks no terminal.
    pub fn screen(&mut self, width: u64, color: bool) {
        self.give("width", Value::U64(width));
        self.give("color", Value::Bool(color));
    }

    /// Where a run reads its input: the cell the stage before answered,
    /// and the root where none stands before it. It is set on every
    /// stage, the only stage of a line with no `|` included, so a
    /// declared `input` is always the shell's and never a positional.
    pub fn input(&mut self, rd: Rd) {
        self.give("input", Value::Rd(rd));
    }

    /// One name of the environment a run reads, as the host of this
    /// shell sets it: a field declared with this name and this value's
    /// type is filled with it and takes no token, and a field no value
    /// here names is a positional like any other.
    ///
    /// What the host of the shell knows and no line says — the screen
    /// a run writes for, the cell a stage reads — and never a word of
    /// the line's own language: an address a `:` line spells is on
    /// that line, as [`Self::option`] has it.
    pub fn give(&mut self, name: &str, value: Value) {
        match self.env.iter_mut().find(|(held, _)| held == name) {
            Some(held) => held.1 = value,
            None => self.env.push((name.to_string(), value)),
        }
    }

    /// One field name this shell's host fills by name alone: `--NAME
    /// VALUE` on the line, never a positional token, and `[--NAME
    /// NAME]` in the form `help` prints.
    ///
    /// A declaration says a field's name and its type, and a `rest`
    /// says which field takes the rest of a line — but nothing in it
    /// says that a word is a whole language's and never a value.
    /// `substitute`'s `address, spec` and `bind`'s `keys, line` are
    /// the same shape, and `:s/a/b/` names no range while `bind gw`
    /// does name its keys. The shell knows no name's meaning; the
    /// host whose language the word belongs to does, and says so
    /// here once — Vim's `address` is Vim's own grammar, read off the
    /// `:` line before the name and put back on it as a flag.
    ///
    /// Naming one is shell-wide and says two things: no positional
    /// token ever fills a field of this name, and no `rest` field
    /// ever reads `--NAME` as text, so a record with no field for it
    /// answers an `undeclared` [`Miss`] the host may word its own
    /// way.
    pub fn option(&mut self, name: &str) {
        if !self.optioned(name) {
            self.options.push(name.to_string());
        }
    }

    /// Whether the host named `name` an option of, by [`Self::option`].
    #[must_use]
    pub fn optioned(&self, name: &str) -> bool {
        self.options.iter().any(|held| held == name)
    }

    /// The rd a relative target is read from.
    #[must_use]
    pub const fn cwd(&self) -> Rd {
        self.cwd
    }

    /// Where the line stands, as `pwd` prints it.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// `<path> > `, the prompt the line is drawn after.
    #[must_use]
    pub fn prompt(&self) -> String {
        format!("{} > ", self.path)
    }

    /// To `rd`, which is at `path`; where it stood is where `cd -` now
    /// goes.
    fn go(&mut self, rd: Rd, path: String) {
        let was = (self.cwd, core::mem::replace(&mut self.path, path));
        self.cwd = rd;
        self.back = Some(was);
    }
}

/// Why a line did not run: the name and what the cause is about, and
/// the cause, with the way out after a semicolon where there is one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Miss {
    /// The name as typed; empty for a line that names none.
    pub name: String,
    /// The token the cause is about, or everything typed after the name
    /// where the cause is about the run; empty where there is neither.
    pub target: String,
    /// What went wrong, as the shell says it.
    pub cause: String,
    /// The name is under no directory of PATH: the one Miss a host
    /// tells from the rest, every other being said as the shell says
    /// it. Vim's `:` line says E492 of this one, the line having named
    /// nothing either table has.
    pub unknown: bool,
    /// The target is a token under a dash the record declares no field
    /// for. Not a value typed wrong but a fact of the declaration, and
    /// the second Miss a host may tell from the rest: a host whose own
    /// word the flag is words it its own way, as Vim's `:` line says
    /// E481 of `--address` on a name whose grammar has no range.
    pub undeclared: bool,
}

impl fmt::Display for Miss {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.name.as_str(), self.target.as_str()) {
            ("", _) => write!(f, "{}", self.cause),
            (name, "") => write!(f, "{name}: {}", self.cause),
            (name, target) => write!(f, "{name} {target}: {}", self.cause),
        }
    }
}

impl Miss {
    /// A line that names nothing the shell can run.
    #[must_use]
    pub fn line(cause: &str) -> Self {
        let (name, target) = (String::new(), String::new());
        let cause = cause.to_string();
        Self { name, target, cause, unknown: false, undeclared: false }
    }
}

/// What the shell was doing when it was refused; with the Kernel's
/// error it keys [`CAUSES`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Step {
    /// Walking a target's path, which only `cd` and a redirect do now.
    Resolve,
    /// Everything else: the run and the reads of its cell.
    Op,
}

/// The shell's sentence for the Kernel's error, in the light of the
/// step. A row for the step wins over the `Op` row; `{}` takes the name
/// that stopped a walk.
const CAUSES: &[(Step, Error, &str)] = &[
    (Step::Resolve, Error::Invalid, "no such rd ({})"),
    (Step::Resolve, Error::Unsupported, "no such rd ({})"),
    (Step::Resolve, Error::Gone, "no such rd ({})"),
    (Step::Resolve, Error::Denied, "denied ({})"),
    (Step::Op, Error::Denied, "denied"),
    (Step::Op, Error::Unsupported, "unsupported"),
    (Step::Op, Error::Invalid, "invalid"),
    (Step::Op, Error::Stale, "stale; read it again"),
    (Step::Op, Error::Busy, "busy; try again"),
    (Step::Op, Error::NoSpace, "no space"),
    (Step::Op, Error::Closed, "closed"),
    (Step::Op, Error::Gone, "gone"),
    (Step::Op, Error::Cancelled, "cancelled"),
];

/// A token owns joined runs; quoting also keeps operators literal.
pub(crate) struct Word {
    text: String,
    quoted: bool,
}

/// A line read a word at a time, or looked at without being read at
/// all. Quoting is the whole of the tokenizing here — `|` and `>` are
/// [`run::stages`]' — so a reader that stops partway leaves the rest
/// of the line exactly as it was typed, which is what a `rest` field
/// takes.
pub(crate) struct Cursor<'a> {
    line: &'a str,
    chars: core::iter::Peekable<core::str::CharIndices<'a>>,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(line: &'a str) -> Self {
        Self { line, chars: line.char_indices().peekable() }
    }

    /// Where the next word begins: the first byte not yet read that is
    /// no blank, or the end of the line. Reading none of it.
    pub(crate) fn mark(&mut self) -> usize {
        let blank = |&(_, next): &(usize, char)| next.is_whitespace();
        while self.chars.next_if(blank).is_some() {}
        self.chars.peek().map_or(self.line.len(), |&(at, _)| at)
    }

    /// The run of non-blank bytes at [`Self::mark`], as they stand: no
    /// quote is opened, so none can go unclosed, and a token under a
    /// dash — which is spelled with neither blank nor quote in it — is
    /// known for one before anything is tokenized.
    pub(crate) fn bare(&mut self) -> &'a str {
        let from = self.mark();
        let end = self.line[from..]
            .find(char::is_whitespace)
            .map_or(self.line.len(), |off| from + off);
        &self.line[from..end]
    }

    /// The bare run just read, taken.
    pub(crate) fn past(&mut self, run: &str) {
        let end = self.mark() + run.len();
        while self.chars.peek().is_some_and(|&(at, _)| at < end) {
            self.chars.next();
        }
    }

    /// The next word, quoted runs joined; `None` where only blanks are
    /// left.
    ///
    /// # Errors
    /// `Miss` for a quote this word opened and the line never closed.
    pub(crate) fn next(&mut self) -> Result<Option<Word>, Miss> {
        if self.mark() == self.line.len() {
            return Ok(None);
        }
        word(&mut self.chars).map(Some)
    }
}

/// The first blank-separated word of `line` and everything past it,
/// read before a quote or a `|` is looked at: a name is never quoted,
/// and what a name's declaration says is what decides how the rest of
/// the line is read at all.
fn first_word(line: &str) -> Option<(&str, &str)> {
    let from = line.find(|c: char| !c.is_whitespace())?;
    let named = &line[from..];
    let end = named.find(char::is_whitespace).unwrap_or(named.len());
    Some((&named[..end], &named[end..]))
}

fn words(line: &str) -> Result<Vec<Word>, Miss> {
    let mut tokens = Vec::new();
    let mut cursor = Cursor::new(line);
    while let Some(word) = cursor.next()? {
        tokens.push(word);
    }
    Ok(tokens)
}

fn word(
    chars: &mut core::iter::Peekable<core::str::CharIndices<'_>>,
) -> Result<Word, Miss> {
    let mut token = Word { text: String::new(), quoted: false };
    while let Some(&(_, next)) = chars.peek() {
        if next.is_whitespace() {
            break;
        }
        chars.next();
        if matches!(next, '\'' | '"') {
            token.quoted = true;
            quoted(chars, next, &mut token.text)?;
        } else {
            token.text.push(next);
        }
    }
    Ok(token)
}

fn quoted(
    chars: &mut impl Iterator<Item = (usize, char)>,
    quote: char,
    text: &mut String,
) -> Result<(), Miss> {
    for (_, next) in chars {
        if next == quote {
            return Ok(());
        }
        text.push(next);
    }
    Err(Miss::line("unclosed quote"))
}

/// One typed line: what it printed, and the run it left open.
///
/// The declaration is read before the line is tokenized. The name is
/// the first blank-separated word; if the record its run takes ends
/// in a `rest` field, that field is the rest of the line and the line
/// is never cut at all: the leading flags and the earlier positionals
/// are read off the front as words, and everything from the next
/// word's first byte to the end is one value — quotes, blanks, `|`
/// and `>` all part of it, no stage, no redirect and no dequoting.
/// That is how `:s/don't/X/` reaches Vim's own parser whole and how
/// `bind <C-r> search --backward` binds a line with a flag in it.
///
/// A record with no `rest` is read as it always was: `|`, `>` and
/// `>>` are the shell's own, so a line that redirects prints nothing
/// and the bytes are in the target instead, and a line that pipes
/// prints what its last stage found. `stat a | grep bytes` pipes and
/// `capture /path > out` pours, their declarations saying no field
/// takes the rest of a line.
///
/// # Errors
/// `Miss` for a name PATH does not hold, a token no field takes, a
/// stage that names nothing, an unclosed quote, or whatever the tree says.
pub fn run(
    ctx: &dyn Calls,
    shell: &mut Shell,
    text: &str,
) -> Result<Ran, Miss> {
    if let Some((name, tail)) = first_word(text)
        && let Some(fields) = run::fields(ctx, shell, name)
        && decode::rest_at(&fields).is_some()
    {
        return run::under_path(ctx, shell, name, &run::Fill::Tail(tail), None);
    }
    let words = words(text)?;
    let typed: Vec<&str> = words.iter().map(|word| &*word.text).collect();
    let bare: Vec<bool> = words.iter().map(|word| !word.quoted).collect();
    let (stages, sink) = run::stages(&typed, &bare)?;
    // A builtin answers bytes and not a cell, so neither a pipe nor a
    // redirect has anything of its to take.
    let itself = |stage: &&run::Stage<'_>| builtin(stage.0).is_some();
    let Some(&(name, args)) = stages.iter().find(itself) else {
        return piped(ctx, shell, &stages, sink);
    };
    if stages.len() > 1 {
        return Err(no_stage("a pipe takes a run's output", name, args));
    }
    if sink.is_some() {
        return Err(no_stage("a redirect takes a run's output", name, args));
    }
    let out = builtin_ran(ctx, shell, name, args)?;
    Ok(Ran { out, open: None })
}

/// A line's stages in order: each answers a cell, and every one but the
/// last is opened, read and closed by the stage after it, which takes
/// it out of the tree; the shell opens the last stage's alone. What a
/// stage did not take is ended here, so a line that stops early and a
/// last stage that declares no `input` both leave nothing behind.
fn piped(
    ctx: &dyn Calls,
    shell: &Shell,
    stages: &[run::Stage<'_>],
    sink: Option<run::Sink<'_>>,
) -> Result<Ran, Miss> {
    let Some((last, before)) = stages.split_last() else {
        return Err(Miss::line(run::NO_NAME));
    };
    // A stage after a `|` is filled through a declared `input: Rd`, so
    // one that declares none would leave the cell before it unread and
    // print as if nothing stood there: refused from the declarations,
    // before anything is called.
    for (name, _) in stages.iter().skip(1) {
        if !run::joins(ctx, shell, name) {
            return Err(no_stage("takes no input", name, &[]));
        }
    }
    let mut input = ROOT;
    for (name, args) in before {
        // No stage but the last takes a redirect, so the pair this
        // answers with holds nothing but the cell.
        let stage = staged(shell, input, false);
        let fill = run::Fill::Tokens(args);
        let cell = run::called(ctx, &stage, name, &fill, None);
        run::dropped(ctx, input);
        input = cell?.0;
    }
    let stage = staged(shell, input, sink.is_none());
    let fill = run::Fill::Tokens(last.1);
    let ran = run::under_path(ctx, &stage, last.0, &fill, sink);
    run::dropped(ctx, input);
    ran
}

/// The Shell one stage runs with: the cell the stage before answered as
/// its input, and the host's screen only where the bytes reach one. A
/// stage that feeds another, and every stage of a line that redirects,
/// writes for no screen at all, so a file holds text and no escape.
fn staged(shell: &Shell, input: Rd, screen: bool) -> Shell {
    let mut stage = shell.clone();
    stage.input(input);
    if !screen {
        stage.screen(0, false);
    }
    stage
}

/// A name that cannot stand where the line put it: a builtin, which
/// answers bytes and no cell for a `|` or a `>` to take, and a stage
/// after a `|` whose run declares no `input: Rd` to be filled through.
fn no_stage(cause: &str, name: &str, args: &[&str]) -> Miss {
    let (name, target) = (name.to_string(), args.join(" "));
    Miss {
        name,
        target,
        cause: cause.to_string(),
        unknown: false,
        undeclared: false,
    }
}

/// What the line takes after `name`, as a usage line writes it: a
/// builtin's own form, else the one the declaration of a name under
/// PATH gives. `None` for a name the shell cannot run.
#[must_use]
pub fn form(ctx: &dyn Calls, shell: &Shell, name: &str) -> Option<String> {
    let under = || run::form(ctx, shell, name).ok();
    builtin(name).map_or_else(under, |entry| Some(entry.form()))
}

/// The three the shell runs itself; what a builtin prints.
fn builtin_ran(
    ctx: &dyn Calls,
    shell: &mut Shell,
    name: &str,
    args: &[&str],
) -> Result<Vec<u8>, Miss> {
    match (name, args.len()) {
        ("cd", 0 | 1) => cd(ctx, shell, args.first().copied()),
        ("pwd", 0) => Ok(format!("{}\n", shell.path).into_bytes()),
        ("help", 0 | 1) => {
            help(ctx, shell, args.first().copied()).map(String::into_bytes)
        }
        _ => Err(misused(name, args)),
    }
}

/// A builtin given more or fewer tokens than it takes: its own form.
fn misused(name: &str, args: &[&str]) -> Miss {
    let form = builtin(name).map(Builtin::form).unwrap_or_default();
    let cause = format!("usage: {form}");
    let (name, target) = (name.to_string(), args.join(" "));
    Miss { name, target, cause, unknown: false, undeclared: false }
}

/// `help`, and `help NAME`: the lines the shell can run, read from the
/// builtins and from the declaration of every name under PATH, or the
/// one line of that name. The listing is sorted, a listing reading
/// better that way; Tab is not, a completion having to offer first
/// what the line would run.
fn help(
    ctx: &dyn Calls,
    shell: &Shell,
    asked: Option<&str>,
) -> Result<String, Miss> {
    let Some(name) = asked else {
        let mut out = String::new();
        for entry in &BUILTINS {
            let _ = writeln!(out, "{}", entry.line());
        }
        let mut names = run::names(ctx, shell);
        names.sort();
        for name in names {
            let _ = writeln!(out, "{}", run::form(ctx, shell, &name)?);
        }
        out.push_str(AFTER);
        return Ok(format!("{out}\n"));
    };
    match builtin(name) {
        Some(entry) => Ok(format!("{}\n", entry.line())),
        None => Ok(format!("{}\n", run::form(ctx, shell, name)?)),
    }
}

/// `cd`: the root with no target, the one before it under `-`, else the
/// Container the target names, whose path the prompt then shows.
fn cd(
    ctx: &dyn Calls,
    shell: &mut Shell,
    target: Option<&str>,
) -> Result<Vec<u8>, Miss> {
    let target = target.unwrap_or("/");
    if target == "-" {
        let Some((rd, path)) = shell.back.clone() else {
            let (name, target) = ("cd".to_string(), target.to_string());
            let cause = "nowhere to go back to".to_string();
            return Err(Miss {
                name,
                target,
                cause,
                unknown: false,
                undeclared: false,
            });
        };
        shell.go(rd, path);
        return Ok(Vec::new());
    }
    let rd = resolved(ctx, shell, "cd", target)?;
    let refused = |error: Error| miss("cd", target, Step::Op, error, "");
    let info = ops::info(ctx, rd).map_err(refused)?;
    if !info.flags.contains(Flags::CONTAINER) {
        return Err(refused(Error::Unsupported));
    }
    let path = ops::where_is(ctx, rd).map_err(refused)?;
    shell.go(rd, path);
    Ok(Vec::new())
}

/// The rd of `target` from where the shell stands, or the Miss naming
/// the segment that stopped the walk.
///
/// The walk is spelled here and not [`reovim_sdk::cell::resolve`]
/// because that one answers an `Error` with no name in it, and the
/// Miss says which segment was not there.
fn resolved(
    ctx: &dyn Calls,
    shell: &Shell,
    name: &str,
    target: &str,
) -> Result<Rd, Miss> {
    let stepped = |step: walk::Step<'_>| match step {
        walk::Step::Child { rd, name } => {
            ops::lookup(ctx, rd, name).map_err(|e| (name.to_string(), e))
        }
        walk::Step::Parent { rd } => {
            ops::parent(ctx, rd).map_err(|e| ("..".to_string(), e))
        }
    };
    walk::walk(shell.cwd, target, stepped)
        .map_err(|(at, error)| miss(name, target, Step::Resolve, error, &at))
}

/// The Miss for `error` under `name` on `target`, `at` filling the
/// row's `{}`.
fn miss(name: &str, target: &str, step: Step, error: Error, at: &str) -> Miss {
    let cause = row(step, error).replace("{}", at);
    let (name, target) = (name.to_string(), target.to_string());
    Miss { name, target, cause, unknown: false, undeclared: false }
}

/// The row for this error at this step, else the one for its op.
fn row(step: Step, error: Error) -> &'static str {
    let at = |step: Step| {
        let row = |r: &&(Step, Error, &str)| r.0 == step && r.1 == error;
        CAUSES.iter().find(row).map(|r| r.2)
    };
    at(step).or_else(|| at(Step::Op)).unwrap_or("refused")
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;
    use alloc::vec::Vec;

    use reovim_kernel_interface::Error;

    use super::{CAUSES, Miss, Step, row};

    /// The nine the Kernel may answer with.
    const ERRORS: [Error; 9] = [
        Error::Denied,
        Error::Unsupported,
        Error::Invalid,
        Error::Stale,
        Error::Busy,
        Error::NoSpace,
        Error::Closed,
        Error::Gone,
        Error::Cancelled,
    ];

    #[test]
    fn every_row_of_the_cause_table_is_its_own_and_no_error_is_nameless() {
        let mut seen: Vec<(Step, Error)> = Vec::new();
        for (step, error, text) in CAUSES {
            let key = (*step, *error);
            assert!(!seen.contains(&key), "{step:?} {error:?} twice");
            seen.push(key);
            assert!(!text.is_empty() && !text.ends_with(' '), "{text}");
            // `{}` is filled by the call site: the name that stopped a
            // walk, which only a resolve has.
            let holes = text.matches("{}").count();
            assert_eq!(holes, usize::from(*step == Step::Resolve), "{text}");
        }
        // The op step answers for all nine, so no error is ever
        // nameless at either step.
        for error in ERRORS {
            assert!(seen.contains(&(Step::Op, error)), "{error:?} has no row");
            assert_ne!(row(Step::Resolve, error), "refused", "{error:?}");
        }
    }

    #[test]
    fn a_line_that_names_nothing_says_its_cause_as_it_is() {
        // A Miss the shell made itself carries a whole sentence, and
        // there is no name or target to put around it.
        assert_eq!(Miss::line("said as it is").to_string(), "said as it is");
    }

    #[test]
    fn words_join_what_a_quote_touches() {
        let read = |line| {
            super::words(line)
                .unwrap()
                .into_iter()
                .map(|word| (word.text, word.quoted))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            read("a\" \"b ''"),
            alloc::vec![
                ("a b".to_string(), true),
                (alloc::string::String::new(), true)
            ]
        );
        assert_eq!(read("'\"'\"'\""), alloc::vec![("\"'".to_string(), true)]);
        assert_eq!(
            read(r"a\ b"),
            alloc::vec![("a\\".to_string(), false), ("b".to_string(), false)]
        );
        assert_eq!(read(" \t\n"), Vec::new());
        assert_eq!(read("'a\\b'"), alloc::vec![("a\\b".to_string(), true)]);
        let Err(miss) = super::words("a \"b") else {
            panic!("an open quote was accepted");
        };
        assert_eq!(miss.to_string(), "unclosed quote");
    }
}
