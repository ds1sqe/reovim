#![no_std]
#![forbid(unsafe_code)]
//! The cli: a command line in, text out, through a `Client`. Generic
//! over the tree; it knows no Module. The process boundary (stdin,
//! stdout, the link, the terminal) is the app's.
//!
//! A command that does not run gives a `Miss`: the word, the target as
//! typed, and the cli's sentence for what went wrong, which the app
//! prints as one line after `reovim: `.

extern crate alloc;

mod complete;
mod line;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;
use core::fmt::Write as _;

use reovim_client::{Client, Decl, Error, Fault, Hash, Rd, log, value};

pub use complete::{Completion, complete};
pub use line::{Act, Line};

/// The commands, one per line: the word, what it takes, what it does.
/// The source of `help` and of the usage line of a word used wrongly.
pub const USAGE: &str = "\
lookup <target>                 rd
info <target>                   name, flags, times, apis
children <target>               rd and name per line
read <target>                   bytes to stdout
write <target> [bytes]          bytes, or stdin without them, at 0
call <target> <method> [arg]    output decoded by the declaration
log [FROM]                      entries from seq FROM, default 1
load <path>                     the Module in that .rvm file
unload <name>                   the Module of that name
help [WORD]                     these lines, or the one for WORD
quit                            end an interactive session
<target> is a path from / or rd:<n>";

/// The command names, for completion.
const COMMANDS: [&str; 11] = [
    "lookup", "info", "children", "read", "write", "call", "log", "load",
    "unload", "help", "quit",
];

/// Why one command did not run: the word and the target as typed, and
/// the cause, with the way out after a semicolon where there is one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Miss {
    /// The word as typed; empty for a line that names none.
    pub word: String,
    /// The target as typed; empty for a word that takes none.
    pub target: String,
    /// What went wrong, in the cli's words.
    pub cause: String,
}

impl fmt::Display for Miss {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.word.as_str(), self.target.as_str()) {
            ("", _) => write!(f, "{}", self.cause),
            (word, "") => write!(f, "{word}: {}", self.cause),
            (word, target) => write!(f, "{word} {target}: {}", self.cause),
        }
    }
}

impl Miss {
    /// A line that names no command: a wrong word, or a link that
    /// never opened.
    #[must_use]
    pub fn line(cause: &str) -> Self {
        let (word, target) = (String::new(), String::new());
        Self { word, target, cause: cause.to_string() }
    }
}

/// What the cli was doing when it was refused; with the word and the
/// Kernel's error it keys [`CAUSES`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    /// Walking the target's path.
    Resolve,
    /// Finding the method `call` names.
    Method,
    /// The op the word stands for.
    Op,
}

/// The cli's sentence for the Kernel's error, in the light of the word
/// and the step.
///
/// An empty word matches any word; a row for the word wins over one
/// for any, a row for the step over the `Op` row. `{}` takes what only
/// the call site knows, in order: the name that did not resolve, or
/// the method asked for and the methods the rd offers.
pub const CAUSES: &[(&str, Step, Error, &str)] = &[
    ("", Step::Resolve, Error::Invalid, "no such rd ({})"),
    ("", Step::Resolve, Error::Unsupported, "no such rd ({})"),
    ("", Step::Resolve, Error::Gone, "no such rd ({})"),
    ("", Step::Resolve, Error::Denied, "denied ({})"),
    ("call", Step::Method, Error::Unsupported, "no method {}; it offers: {}"),
    ("read", Step::Op, Error::Unsupported, "a Container has no bytes; ls it"),
    ("write", Step::Op, Error::Denied, "not writable"),
    ("write", Step::Op, Error::Unsupported, "a Container takes no bytes"),
    ("", Step::Op, Error::Denied, "denied"),
    ("", Step::Op, Error::Unsupported, "unsupported"),
    ("", Step::Op, Error::Invalid, "invalid"),
    ("", Step::Op, Error::Stale, "stale; read it again"),
    ("", Step::Op, Error::Busy, "busy; try again"),
    ("", Step::Op, Error::NoSpace, "no space"),
    ("", Step::Op, Error::Closed, "closed"),
    ("", Step::Op, Error::Gone, "gone"),
    ("", Step::Op, Error::Cancelled, "cancelled"),
];

/// One command already split into words; what goes to stdout, text or
/// raw bytes. `write` without bytes needs stdin, so the app fills the
/// argument before calling.
///
/// # Errors
/// `Miss` for an unknown word, the wrong number of words, `quit`
/// outside an interactive session, or whatever the Client says.
pub fn input(client: &mut Client, words: &[String]) -> Result<Vec<u8>, Miss> {
    if let Some(text) = help(words) {
        return text.map(String::into_bytes);
    }
    let Some(word) = words.first().map(String::as_str) else {
        return Err(Miss::line("no word; help lists them"));
    };
    match (word, words.len()) {
        ("quit", _) => {
            return Err(Miss::line("quit ends an interactive session"));
        }
        ("log", 1 | 2) => return log_from(client, words.get(1)),
        ("load", 2) => {
            let path = &words[1];
            let loaded = client.load(path).map(|()| Vec::new());
            return loaded.map_err(|f| miss(word, path, Step::Op, &f, &[]));
        }
        ("unload", 2) => {
            let name = &words[1];
            let gone = client.unload(name).map(|()| Vec::new());
            return gone.map_err(|f| miss(word, name, Step::Op, &f, &[]));
        }
        ("lookup" | "info" | "children" | "read", 2)
        | ("write", 3)
        | ("call", 3 | 4) => {}
        _ => return Err(misuse(word)),
    }
    let target = &words[1];
    let rd = client
        .resolve_at(target)
        .map_err(|(at, f)| miss(word, target, Step::Resolve, &f, &[&at]))?;
    let refused = |fault: Fault| miss(word, target, Step::Op, &fault, &[]);
    match word {
        "lookup" => Ok(format!("{}\n", rd.0).into_bytes()),
        "info" => info(client, rd).map_err(refused),
        "children" => children(client, rd).map_err(refused),
        "read" => client.read_all(rd).map_err(refused),
        "write" => {
            let bytes = words[2].as_bytes().to_vec();
            let len = client.write_all(rd, bytes).map_err(refused)?;
            Ok(format!("{len}\n").into_bytes())
        }
        _ => call(client, rd, target, &words[2], words.get(3)),
    }
}

/// `help`, `help WORD`, and the app's `--help` and `-h`: the table, or
/// the one line of that word, to stdout. None when the words ask for
/// something else.
///
/// # Errors
/// `Miss` for a word with no entry, or for more words than one.
#[must_use]
pub fn help(words: &[String]) -> Option<Result<String, Miss>> {
    let word = words.first().map(String::as_str)?;
    if !matches!(word, "help" | "--help" | "-h") {
        return None;
    }
    Some(match (words.get(1).map(String::as_str), words.len()) {
        (None, _) => Ok(format!("{USAGE}\n")),
        (Some(asked), 2) => entry(asked)
            .map_or_else(|| Err(unknown(asked)), |l| Ok(format!("{l}\n"))),
        _ => Err(misuse("help")),
    })
}

/// A fault with no command behind it: nothing listening at `socket`
/// before anything was sent, or the app's own stdin and stdout.
#[must_use]
pub fn plain(fault: &Fault, socket: Option<&str>) -> Miss {
    match (fault, socket) {
        (Fault::Carrier, Some(socket)) => {
            Miss::line(&format!("no server at {socket}"))
        }
        _ => Miss::line(&cause("", Step::Op, fault, &[])),
    }
}

/// The word used wrongly: its own usage line, or no such word.
fn misuse(word: &str) -> Miss {
    form(word).map_or_else(
        || unknown(word),
        |form| Miss::line(&format!("usage: {form}; help {word} says more")),
    )
}

fn unknown(word: &str) -> Miss {
    Miss::line(&format!("unknown word {word}; help lists them"))
}

/// The `USAGE` line of `word`.
fn entry(word: &str) -> Option<&'static str> {
    if !COMMANDS.contains(&word) {
        return None;
    }
    USAGE.lines().find(|line| line.split_whitespace().next() == Some(word))
}

/// What the word takes: the left column of its entry.
fn form(word: &str) -> Option<&'static str> {
    entry(word).map(|line| line.split("  ").next().unwrap_or(line))
}

/// The Miss for `fault` under `word` on `target`, `details` filling the
/// row's `{}`.
fn miss(
    word: &str,
    target: &str,
    step: Step,
    fault: &Fault,
    details: &[&str],
) -> Miss {
    let cause = cause(word, step, fault, details);
    Miss { word: word.to_string(), target: target.to_string(), cause }
}

/// The cli's words for `fault`: the table for the Kernel's errors, the
/// link and the input as they are.
fn cause(word: &str, step: Step, fault: &Fault, details: &[&str]) -> String {
    match fault {
        Fault::Kernel(error) => fill(row(word, step, *error), details),
        Fault::Carrier => "link gone".to_string(),
        Fault::Wire => "not a message from the server".to_string(),
        Fault::Usage(text) => text.clone(),
    }
}

/// The row for this word at this step, else the one for any word, else
/// the word's own op.
fn row(word: &str, step: Step, error: Error) -> &'static str {
    let at = |w: &str, s: Step| {
        let row = |r: &&(&str, Step, Error, &str)| {
            r.0 == w && r.1 == s && r.2 == error
        };
        CAUSES.iter().find(row).map(|r| r.3)
    };
    at(word, step)
        .or_else(|| at("", step))
        .or_else(|| at("", Step::Op))
        .unwrap_or("refused")
}

/// The `{}` of a row, in order, from `details`.
fn fill(cause: &str, details: &[&str]) -> String {
    let mut out = String::new();
    let mut details = details.iter();
    for (at, part) in cause.split("{}").enumerate() {
        if at > 0 {
            out.push_str(details.next().copied().unwrap_or_default());
        }
        out.push_str(part);
    }
    out
}

fn info(client: &mut Client, rd: Rd) -> Result<Vec<u8>, Fault> {
    let info = client.info(rd)?;
    let flags = ["READ", "WRITE", "CONTAINER"]
        .iter()
        .zip([1, 2, 4])
        .filter(|(_, bit)| info.flags.bits() & bit != 0)
        .map(|(name, _)| *name)
        .collect::<Vec<_>>()
        .join("|");
    let apis: Vec<String> =
        info.apis.iter().map(|h| format!("{:016x}", h.0)).collect();
    let line = format!(
        "name={} flags={flags} ctime={} mtime={} atime={} apis={}\n",
        info.name.unwrap_or_default(),
        info.ctime,
        info.mtime,
        info.atime,
        apis.join(",")
    );
    Ok(line.into_bytes())
}

fn children(client: &mut Client, rd: Rd) -> Result<Vec<u8>, Fault> {
    let mut out = String::new();
    for child in client.children(rd)? {
        let name = client.name(child)?;
        let _ = writeln!(out, "{} {name}", child.0);
    }
    Ok(out.into_bytes())
}

/// The method by name among what the rd offers, so a name that is not
/// there says what is.
fn call(
    client: &mut Client,
    rd: Rd,
    target: &str,
    name: &str,
    arg: Option<&String>,
) -> Result<Vec<u8>, Miss> {
    let refused = |fault: Fault| miss("call", target, Step::Op, &fault, &[]);
    let decls = client
        .decls(rd)
        .map_err(|f| miss("call", target, Step::Method, &f, &[]))?;
    let asked = |(hash, decl): &(Hash, Decl)| {
        let named = decl.methods.iter().find(|m| m.name == name);
        named.map(|m| (*hash, m.clone()))
    };
    let Some((api, method)) = decls.iter().find_map(asked) else {
        return Err(offered(target, name, &decls));
    };
    let input = value::encode_arg(&method.input, arg.map(String::as_str))
        .map_err(refused)?;
    let output =
        client.call_method(rd, api, &method, input).map_err(refused)?;
    value::show(&method.output, &output).map_err(refused)
}

/// No such method: the ones the rd does offer.
fn offered(target: &str, name: &str, decls: &[(Hash, Decl)]) -> Miss {
    let names: Vec<&str> = decls
        .iter()
        .flat_map(|(_, decl)| decl.methods.iter())
        .map(|m| m.name.as_str())
        .collect();
    let offers =
        if names.is_empty() { "none".to_string() } else { names.join(", ") };
    let none = Fault::Kernel(Error::Unsupported);
    miss("call", target, Step::Method, &none, &[name, &offers])
}

fn log_from(
    client: &mut Client,
    from: Option<&String>,
) -> Result<Vec<u8>, Miss> {
    let from = from
        .map_or(Ok(1), |text| text.parse::<u64>().map_err(|_| misuse("log")))?;
    let mut out = String::new();
    let tail = log::tail(client, from)
        .map_err(|f| miss("log", "", Step::Op, &f, &[]))?;
    for entry in tail {
        out.push_str(&entry.line());
    }
    Ok(out.into_bytes())
}
