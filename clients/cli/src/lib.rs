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
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;
use core::fmt::Write as _;

use reovim_client::{
    Client, Decl, Error, Fault, Field, Flags, Hash, Info, Rd, Rights, Ty, log,
    value,
};

pub use complete::{Completion, complete};
pub use line::{Act, Line};

/// The commands, one per line: the word, what it takes, what it does.
/// The source of `help` and of the usage line of a word used wrongly.
pub const USAGE: &str = "\
ls [-l] [-a] [TARGET..]         children by name; -l long, -a rds
cat <target>..                  the bytes of each in turn
stat <target>                   the long line, the times, the apis
tree [-L N] [TARGET]            the tree under it, N levels deep
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
const COMMANDS: [&str; 15] = [
    "ls", "cat", "stat", "tree", "lookup", "info", "children", "read", "write",
    "call", "log", "load", "unload", "help", "quit",
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
    ("cat", Step::Op, Error::Unsupported, "a Container has no bytes; ls it"),
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
/// raw bytes.
///
/// `write` without bytes needs stdin, so the app fills the argument
/// before calling. `width` is the terminal's columns, which `ls` lays
/// its names out across; `None` when the output is a pipe, and then
/// one name per line.
///
/// # Errors
/// `Miss` for an unknown word, the wrong number of words, `quit`
/// outside an interactive session, or whatever the Client says.
pub fn input(
    client: &mut Client,
    words: &[String],
    width: Option<u16>,
) -> Result<Vec<u8>, Miss> {
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
        ("ls", _) => return ls(client, &words[1..], width),
        ("cat", 2..) => return cat(client, &words[1..]),
        ("stat", 2) => return stat(client, &words[1]),
        ("tree", _) => return tree(client, &words[1..]),
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
    let rd = resolved(client, word, target)?;
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

/// The rd of `target`, or the Miss naming the segment that stopped the
/// walk.
fn resolved(client: &mut Client, word: &str, target: &str) -> Result<Rd, Miss> {
    let stopped = |(at, f): (String, Fault)| {
        miss(word, target, Step::Resolve, &f, &[&at])
    };
    client.resolve_at(target).map_err(stopped)
}

/// Where a target that names no path starts; 10.3 puts the cwd here.
const CWD: &str = "/";

/// One listed rd: its number, the name it shows under, and what the
/// Kernel caches about it.
struct Entry {
    rd: Rd,
    name: String,
    info: Info,
}

/// How `ls` writes a listing: `-l` the long line, `-a` the rd first.
#[derive(Clone, Copy)]
struct Form {
    long: bool,
    rds: bool,
}

/// The children of each target by name, sorted; in columns across
/// `width` and one per line without it. A cell lists itself.
fn ls(
    client: &mut Client,
    args: &[String],
    width: Option<u16>,
) -> Result<Vec<u8>, Miss> {
    let Some((form, mut targets)) = options(args) else {
        return Err(misuse("ls"));
    };
    if targets.is_empty() {
        targets.push(CWD);
    }
    let many = targets.len() > 1;
    let mut out = String::new();
    for target in targets {
        let gap = if out.is_empty() { "" } else { "\n" };
        if many {
            let _ = writeln!(out, "{gap}{target}:");
        }
        out.push_str(&listing(client, target, form, width)?);
    }
    Ok(out.into_bytes())
}

/// The `-l` and `-a` of `args` and the targets after them; `None` for
/// a flag `ls` does not take.
fn options(args: &[String]) -> Option<(Form, Vec<&str>)> {
    let mut form = Form { long: false, rds: false };
    let mut targets = Vec::new();
    for arg in args {
        match flags(arg) {
            None => targets.push(arg.as_str()),
            Some("l") => form.long = true,
            Some("a") => form.rds = true,
            Some("la" | "al") => form = Form { long: true, rds: true },
            Some(_) => return None,
        }
    }
    Some((form, targets))
}

/// The letters of a flag word, or `None` for a target.
fn flags(word: &str) -> Option<&str> {
    word.strip_prefix('-').filter(|letters| !letters.is_empty())
}

/// One target's listing: a Container's children, or the target itself.
fn listing(
    client: &mut Client,
    target: &str,
    form: Form,
    width: Option<u16>,
) -> Result<String, Miss> {
    let rd = resolved(client, "ls", target)?;
    let refused = |fault: Fault| miss("ls", target, Step::Op, &fault, &[]);
    let info = client.info(rd).map_err(refused)?;
    let listed = if info.flags.contains(Flags::CONTAINER) {
        entries(client, rd).map_err(refused)?
    } else {
        vec![Entry { name: named(rd, &info), rd, info }]
    };
    if form.long {
        let mut out = String::new();
        for entry in &listed {
            let line = long_line(client, entry, form.rds).map_err(refused)?;
            out.push_str(&line);
        }
        return Ok(out);
    }
    let text = |entry: &Entry| {
        if form.rds {
            format!("{} {}", entry.rd.0, entry.name)
        } else {
            entry.name.clone()
        }
    };
    let names: Vec<String> = listed.iter().map(text).collect();
    Ok(width.map_or_else(
        || lines(&names),
        |width| columns(&names, usize::from(width)),
    ))
}

/// Every child of `rd`, named and sorted by name.
fn entries(client: &mut Client, rd: Rd) -> Result<Vec<Entry>, Fault> {
    let mut entries = Vec::new();
    for child in client.children(rd)? {
        let info = client.info(child)?;
        entries.push(Entry { rd: child, name: named(child, &info), info });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

/// The name an rd shows under; one the Kernel does not name is `rd:N`.
fn named(rd: Rd, info: &Info) -> String {
    info.name.clone().unwrap_or_else(|| format!("rd:{}", rd.0))
}

/// `TYPE RIGHTS  MTIME  APIS  NAME`, the rd number first under `-a`.
fn long_line(
    client: &mut Client,
    entry: &Entry,
    rds: bool,
) -> Result<String, Fault> {
    let flags = entry.info.flags;
    let kind = if flags.contains(Flags::CONTAINER) { 'c' } else { '-' };
    let read = if flags.contains(Flags::READ) { 'r' } else { '-' };
    let write = if flags.contains(Flags::WRITE) { 'w' } else { '-' };
    let mut apis = Vec::new();
    for hash in &entry.info.apis {
        apis.push(client.decl(*hash)?.api);
    }
    let apis = if apis.is_empty() { "-".to_string() } else { apis.join(",") };
    let number = if rds { format!("{} ", entry.rd.0) } else { String::new() };
    let at = date(entry.info.mtime);
    Ok(format!("{number}{kind} {read}{write}  {at}  {apis}  {}\n", entry.name))
}

/// One name per line, as a pipe wants them.
fn lines(names: &[String]) -> String {
    let mut out = String::new();
    for name in names {
        let _ = writeln!(out, "{name}");
    }
    out
}

/// `names` down columns as `ls` lays them out: the most columns whose
/// widths and two-space gaps fit in `width`, read down and then across.
fn columns(names: &[String], width: usize) -> String {
    let rows = |columns: usize| names.len().div_ceil(columns);
    let widths = |columns: usize| -> Vec<usize> {
        let widest =
            |column: &[String]| column.iter().map(|name| cells(name)).max();
        names.chunks(rows(columns)).map(|c| widest(c).unwrap_or(0)).collect()
    };
    let fits = |columns: &usize| {
        let widths = widths(*columns);
        widths.len() == *columns
            && widths.iter().sum::<usize>() + 2 * (columns - 1) <= width
    };
    let count = (1..=names.len()).rev().find(fits).unwrap_or(1);
    let (rows, widths) = (rows(count), widths(count));
    let mut out = String::new();
    for row in 0..rows {
        let at = |column: usize| names.get(column * rows + row);
        let cell = |column: usize| Some((at(column)?, widths[column]));
        let pad = |(name, width): (&String, usize)| {
            format!("{name}{}", " ".repeat(width - cells(name) + 2))
        };
        let line: String = (0..count).filter_map(cell).map(pad).collect();
        let _ = writeln!(out, "{}", line.trim_end());
    }
    out
}

/// The cells `text` takes on a terminal row.
fn cells(text: &str) -> usize {
    text.chars().map(reovim_client::width).sum()
}

/// `YYYY-MM-DD HH:MM:SS` in UTC from the Kernel clock's nanoseconds,
/// by the civil-from-days arithmetic, which needs no table and no host.
fn date(nanos: u64) -> String {
    let seconds = nanos / 1_000_000_000;
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    // Counting the year from March, so the leap day is the last day.
    let shifted = days + 719_468;
    let (era, doe) = (shifted / 146_097, shifted % 146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = era * 400 + yoe + u64::from(month <= 2);
    let (hour, minute) = (rest / 3600, rest % 3600 / 60);
    format!(
        "{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{:02}",
        rest % 60
    )
}

/// The bytes of each target in turn.
fn cat(client: &mut Client, targets: &[String]) -> Result<Vec<u8>, Miss> {
    let mut out = Vec::new();
    for target in targets {
        let rd = resolved(client, "cat", target)?;
        let refused = |f: Fault| miss("cat", target, Step::Op, &f, &[]);
        out.extend(client.read_all(rd).map_err(refused)?);
    }
    Ok(out)
}

/// The long line, the three times as dates, then every api with its
/// methods as the declaration shows them.
fn stat(client: &mut Client, target: &str) -> Result<Vec<u8>, Miss> {
    let rd = resolved(client, "stat", target)?;
    let refused = |fault: Fault| miss("stat", target, Step::Op, &fault, &[]);
    let info = client.info(rd).map_err(refused)?;
    let entry = Entry { name: named(rd, &info), rd, info };
    let mut out = long_line(client, &entry, false).map_err(refused)?;
    let times = [
        ("rd", entry.rd.0.to_string()),
        ("ctime", date(entry.info.ctime)),
        ("mtime", date(entry.info.mtime)),
        ("atime", date(entry.info.atime)),
    ];
    for (key, value) in times {
        let _ = writeln!(out, "{key} {value}");
    }
    for hash in &entry.info.apis {
        let decl = client.decl(*hash).map_err(refused)?;
        let _ = writeln!(out, "{} v{}", decl.api, decl.ver);
        out.push_str(&methods(&decl));
    }
    Ok(out.into_bytes())
}

/// Every method of `decl` under its api, one per line, as the
/// declaration writes it: `0 replace({at: u64}) -> () WRITE`.
fn methods(decl: &Decl) -> String {
    let mut out = String::new();
    for method in &decl.methods {
        let right =
            if method.right.contains(Rights::WRITE) { "WRITE" } else { "READ" };
        let input = match &method.input {
            Ty::Unit => String::new(),
            ty => shown(ty),
        };
        let (number, name) = (method.number, &method.name);
        let output = shown(&method.output);
        let _ = writeln!(out, "  {number} {name}({input}) -> {output} {right}");
    }
    out
}

/// A type as the declaration's text form writes it; a Record and a
/// Choice by their fields, since the canonical bytes keep no name for
/// the type itself.
fn shown(ty: &Ty) -> String {
    match ty {
        Ty::Unit => "()".to_string(),
        Ty::Bool => "bool".to_string(),
        Ty::U8 => "u8".to_string(),
        Ty::U32 => "u32".to_string(),
        Ty::U64 => "u64".to_string(),
        Ty::I64 => "i64".to_string(),
        Ty::F64 => "f64".to_string(),
        Ty::Rd => "Rd".to_string(),
        Ty::Bytes(max) => format!("bytes({max})"),
        Ty::Utf8(max) => format!("utf8({max})"),
        Ty::Sequence(max, item) | Ty::Array(max, item) => {
            format!("[{}; {max}]", shown(item))
        }
        Ty::Record(fields) | Ty::Choice(fields) => {
            let part = |f: &Field| format!("{}: {}", f.name, shown(&f.ty));
            let parts: Vec<String> = fields.iter().map(part).collect();
            let (open, sep, close) = match ty {
                Ty::Choice(_) => ("<", " | ", ">"),
                _ => ("{", ", ", "}"),
            };
            format!("{open}{}{close}", parts.join(sep))
        }
    }
}

/// The target and what is under it as `tree` draws it, ending with the
/// count of what was drawn.
fn tree(client: &mut Client, args: &[String]) -> Result<Vec<u8>, Miss> {
    let Some((depth, target)) = levels(args) else {
        return Err(misuse("tree"));
    };
    let rd = resolved(client, "tree", target)?;
    let refused = |fault: Fault| miss("tree", target, Step::Op, &fault, &[]);
    let info = client.info(rd).map_err(refused)?;
    let mut out = format!("{target}\n");
    let mut count = (0, 0);
    if info.flags.contains(Flags::CONTAINER) {
        let kids = ordered(entries(client, rd).map_err(refused)?);
        draw(client, &kids, "", depth, &mut out, &mut count);
    }
    let _ = write!(out, "\n{} containers, {} cells\n", count.0, count.1);
    Ok(out.into_bytes())
}

/// The `-L N` of `args` and the one target after it; `None` for a flag
/// `tree` does not take or a depth that is not a number above zero.
fn levels(args: &[String]) -> Option<(usize, &str)> {
    let (mut depth, mut target) = (usize::MAX, CWD);
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match flags(arg) {
            None => target = arg,
            Some("L") => {
                depth = args.next()?.parse().ok().filter(|n| *n > 0)?;
            }
            Some(_) => return None,
        }
    }
    Some((depth, target))
}

/// Containers first and then cells, each by name, as `tree` draws them.
fn ordered(mut kids: Vec<Entry>) -> Vec<Entry> {
    let cell = |entry: &Entry| !entry.info.flags.contains(Flags::CONTAINER);
    kids.sort_by(|a, b| {
        cell(a).cmp(&cell(b)).then_with(|| a.name.cmp(&b.name))
    });
    kids
}

/// `kids` under `prefix`, each Container `depth` levels deeper; what
/// cannot be listed says why on its own line and the walk goes on.
fn draw(
    client: &mut Client,
    kids: &[Entry],
    prefix: &str,
    depth: usize,
    out: &mut String,
    count: &mut (usize, usize),
) {
    for (at, kid) in kids.iter().enumerate() {
        let last = at + 1 == kids.len();
        let stem = if last { "└── " } else { "├── " };
        let below = if last { "    " } else { "│   " };
        let container = kid.info.flags.contains(Flags::CONTAINER);
        if container {
            count.0 += 1;
        } else {
            count.1 += 1;
        }
        let under = (container && depth > 1).then(|| entries(client, kid.rd));
        match under {
            Some(Err(fault)) => {
                let why = cause("tree", Step::Op, &fault, &[]);
                let _ = writeln!(out, "{prefix}{stem}{}: {why}", kid.name);
            }
            Some(Ok(kids)) => {
                let _ = writeln!(out, "{prefix}{stem}{}", kid.name);
                let deeper = format!("{prefix}{below}");
                draw(client, &ordered(kids), &deeper, depth - 1, out, count);
            }
            None => {
                let _ = writeln!(out, "{prefix}{stem}{}", kid.name);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::date;

    /// Two instants a calendar can be held against: the epoch, and
    /// 2026-09-17 12:34:56 UTC, which is 1789648496 seconds after it.
    #[test]
    fn the_date_of_a_kernel_instant_is_the_civil_date_in_utc() {
        assert_eq!(date(0), "1970-01-01 00:00:00");
        assert_eq!(date(999_999_999), "1970-01-01 00:00:00");
        assert_eq!(date(1_000_000_000), "1970-01-01 00:00:01");
        assert_eq!(date(1_789_648_496_000_000_000), "2026-09-17 12:34:56");
        // The last second of a leap year, and the leap day before it.
        assert_eq!(date(1_709_208_000_000_000_000), "2024-02-29 12:00:00");
        assert_eq!(date(1_735_689_599_000_000_000), "2024-12-31 23:59:59");
    }
}
