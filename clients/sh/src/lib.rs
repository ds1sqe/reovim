#![no_std]
#![forbid(unsafe_code)]
//! The shell: a typed line in, text out, over a `Tree`. It names no
//! Driver. The first token is a name under PATH, whose declaration says
//! what the rest of the line fills; the run answers with an output cell
//! the shell reads to its end.
//!
//! What it owns is what changes the shell and not the tree: the line
//! and its editing, where the line stands (`cd`, `pwd`), PATH, `help`,
//! `load` and `unload`, `>` and `>>`, Tab. A line that does not run
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

use reovim_client::{Error, Fault, Flags, ROOT, Rd, Tree, walk};

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
/// where the line stands, what the server loads, and `help` itself.
/// Everything else is a name under PATH.
const BUILTINS: [Builtin; 5] = [
    Builtin {
        name: "cd",
        args: "[target]",
        help: "go there; none is /, - the one before",
    },
    Builtin { name: "pwd", args: "", help: "where the line stands now" },
    Builtin {
        name: "load",
        args: "<path>",
        help: "the Module in that .rvm file",
    },
    Builtin { name: "unload", args: "<name>", help: "the Module of that name" },
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

/// Where the line stands in the tree and what the host's screen is.
///
/// The rd a relative target is read from, the path the prompt and `pwd`
/// show, the one before it, which `cd -` goes back to, and the two
/// fields of a run's environment beside the cwd. The host holds one for
/// a session of lines; a Plugin's stands where the Plugin does.
#[derive(Clone, Debug)]
pub struct Shell {
    cwd: Rd,
    path: String,
    back: Option<(Rd, String)>,
    width: u64,
    color: bool,
}

impl Default for Shell {
    /// The root, where every session starts, on a screen of no width.
    fn default() -> Self {
        Self {
            cwd: ROOT,
            path: "/".to_string(),
            back: None,
            width: 0,
            color: false,
        }
    }
}

impl Shell {
    /// A line standing at `rd`, which is at `path`: what a Plugin gives
    /// the lines it runs, so a relative target of theirs is read from
    /// where the Plugin is and not from the root.
    #[must_use]
    pub const fn at(cwd: Rd, path: String) -> Self {
        Self { cwd, path, back: None, width: 0, color: false }
    }

    /// The screen a run writes for: the columns a listing lays its names
    /// out across, 0 for a pipe, and whether the text is painted. The
    /// host knows both, and this crate asks no terminal.
    pub const fn screen(&mut self, width: u64, color: bool) {
        self.width = width;
        self.color = color;
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
        Self { name, target, cause: cause.to_string(), unknown: false }
    }
}

/// What the shell was doing when it was refused; with the Kernel's
/// error it keys [`CAUSES`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Step {
    /// Walking a target's path, which only `cd` and a redirect do now.
    Resolve,
    /// Everything else: the run, the reads of its cell, `load`.
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

/// One typed line: what it printed, and the run it left open.
///
/// `>` and `>>` are the shell's own, so a line that redirects prints
/// nothing and the bytes are in the target instead.
///
/// # Errors
/// `Miss` for a name PATH does not hold, a token no field takes, or
/// whatever the tree says.
pub fn run(
    tree: &mut Tree,
    shell: &mut Shell,
    text: &str,
) -> Result<Ran, Miss> {
    let typed: Vec<&str> = text.split_whitespace().collect();
    let (tokens, sink) = run::split(&typed)?;
    let Some((name, args)) = tokens.split_first() else {
        return Err(Miss::line("no name; help lists them"));
    };
    if builtin(name).is_none() {
        return run::under_path(tree, shell, name, args, sink);
    }
    if sink.is_some() {
        let cause = "a redirect takes a run's output".to_string();
        let target = args.join(" ");
        let name = (*name).to_string();
        return Err(Miss { name, target, cause, unknown: false });
    }
    let out = ran(tree, shell, name, args)?;
    Ok(Ran { out, open: None })
}

/// The five the shell runs itself; what a builtin prints.
fn ran(
    tree: &mut Tree,
    shell: &mut Shell,
    name: &str,
    args: &[&str],
) -> Result<Vec<u8>, Miss> {
    match (name, args.len()) {
        ("cd", 0 | 1) => cd(tree, shell, args.first().copied()),
        ("pwd", 0) => Ok(format!("{}\n", shell.path).into_bytes()),
        ("load", 1) => served(tree.load(args[0]), name, args[0]),
        ("unload", 1) => served(tree.unload(args[0]), name, args[0]),
        ("help", 0 | 1) => {
            help(tree, args.first().copied()).map(String::into_bytes)
        }
        _ => Err(misused(name, args)),
    }
}

/// `load` and `unload` print nothing; what the server refuses is the
/// shell's own line, since neither is an op.
fn served(
    done: Result<(), Fault>,
    name: &str,
    target: &str,
) -> Result<Vec<u8>, Miss> {
    done.map(|()| Vec::new())
        .map_err(|fault| miss(name, target, Step::Op, &fault, ""))
}

/// A builtin given more or fewer tokens than it takes: its own form.
fn misused(name: &str, args: &[&str]) -> Miss {
    let form = builtin(name).map(Builtin::form).unwrap_or_default();
    let cause = format!("usage: {form}");
    let (name, target) = (name.to_string(), args.join(" "));
    Miss { name, target, cause, unknown: false }
}

/// `help`, and `help NAME`: the lines the shell can run, read from the
/// builtins and from the declaration of every name under PATH, or the
/// one line of that name.
fn help(tree: &mut Tree, asked: Option<&str>) -> Result<String, Miss> {
    let Some(name) = asked else {
        let mut out = String::new();
        for entry in &BUILTINS {
            let _ = writeln!(out, "{}", entry.line());
        }
        for name in run::names(tree) {
            let _ = writeln!(out, "{}", run::form(tree, &name)?);
        }
        out.push_str(AFTER);
        return Ok(format!("{out}\n"));
    };
    match builtin(name) {
        Some(entry) => Ok(format!("{}\n", entry.line())),
        None => Ok(format!("{}\n", run::form(tree, name)?)),
    }
}

/// `cd`: the root with no target, the one before it under `-`, else the
/// Container the target names, whose path the prompt then shows.
fn cd(
    tree: &mut Tree,
    shell: &mut Shell,
    target: Option<&str>,
) -> Result<Vec<u8>, Miss> {
    let target = target.unwrap_or("/");
    if target == "-" {
        let Some((rd, path)) = shell.back.clone() else {
            let (name, target) = ("cd".to_string(), target.to_string());
            let cause = "nowhere to go back to".to_string();
            return Err(Miss { name, target, cause, unknown: false });
        };
        shell.go(rd, path);
        return Ok(Vec::new());
    }
    let rd = resolved(tree, shell, "cd", target)?;
    let refused = |fault: Fault| miss("cd", target, Step::Op, &fault, "");
    let info = tree.info(rd).map_err(refused)?;
    if !info.flags.contains(Flags::CONTAINER) {
        return Err(refused(Fault::Kernel(Error::Unsupported)));
    }
    let path = where_is(tree, rd).map_err(refused)?;
    shell.go(rd, path);
    Ok(Vec::new())
}

/// As deep a path as a prompt will show; below that the rd says where
/// it is instead.
const DEEP: usize = 64;

/// The path of `rd`, read from the tree itself: its name under each
/// parent up to the root, so the prompt says where the cwd is however
/// it was named. An rd the Kernel does not name, or one deeper than
/// [`DEEP`], has no path to show and is `rd:<n>`.
fn where_is(tree: &mut Tree, rd: Rd) -> Result<String, Fault> {
    let mut names: Vec<String> = Vec::new();
    let mut at = rd;
    while at != ROOT {
        let name = tree.name(at)?;
        if name.is_empty() || names.len() == DEEP {
            return Ok(format!("rd:{}", rd.0));
        }
        names.push(name);
        at = tree.parent(at)?;
    }
    names.reverse();
    Ok(format!("/{}", names.join("/")))
}

/// The rd of `target` from where the shell stands, or the Miss naming
/// the segment that stopped the walk.
fn resolved(
    tree: &mut Tree,
    shell: &Shell,
    name: &str,
    target: &str,
) -> Result<Rd, Miss> {
    let stopped = |(at, fault): (String, Fault)| {
        miss(name, target, Step::Resolve, &fault, &at)
    };
    walk(tree, shell.cwd, target).map_err(stopped)
}

/// The Miss for `fault` under `name` on `target`, `at` filling the
/// row's `{}`.
fn miss(name: &str, target: &str, step: Step, fault: &Fault, at: &str) -> Miss {
    let cause = cause(step, fault, at);
    let (name, target) = (name.to_string(), target.to_string());
    Miss { name, target, cause, unknown: false }
}

/// The shell's sentence for `fault`: the table for the Kernel's
/// errors, the link and the input as they are.
fn cause(step: Step, fault: &Fault, at: &str) -> String {
    match fault {
        Fault::Kernel(error) => row(step, *error).replace("{}", at),
        Fault::Carrier => "link gone".to_string(),
        Fault::Wire => "not a message from the server".to_string(),
        Fault::Usage(text) => text.clone(),
    }
}

/// The row for this error at this step, else the one for its op.
fn row(step: Step, error: Error) -> &'static str {
    let at = |step: Step| {
        let row = |r: &&(Step, Error, &str)| r.0 == step && r.1 == error;
        CAUSES.iter().find(row).map(|r| r.2)
    };
    at(step).or_else(|| at(Step::Op)).unwrap_or("refused")
}
