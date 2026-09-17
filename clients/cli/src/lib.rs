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
pub mod dmesg;
mod line;
mod paint;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;
use core::fmt::Write as _;

use reovim_client::{
    Client, Decl, Error, Event, Fault, Field, Flags, Handle, Hash, Info, Kind,
    ROOT, Rd, Rights, Ty, log, value,
};

pub use complete::{Completion, complete};
pub use line::{Act, Line};
pub use paint::Paint;

/// The commands, one per line: the word, what it takes, what it does.
/// The source of `help` and of the usage line of a word used wrongly.
pub const USAGE: &str = "\
ls [-l] [-a] [-w] [TARGET..]    children by name; -l long, -a rds
cat [-f] [-n N] <target>..      the bytes of each in turn; -f follows
stat <target>                   the long line, the times, the apis
tree [-L N] [TARGET]            the tree under it, N levels deep
dmesg [-tTHxkwW] [-l LEVELS]    the log as dmesg reads it; -w follows
watch <target>                  seq kind name, per event, as they come
cd [TARGET]                     go there; none is /, - the one before
pwd                             where the line stands now
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
<target> is a path from /, a name under the cwd, . or .., or rd:<n>

the line: Tab completes, Up and Down walk the lines that begin as this
one does, Ctrl-A the start, Ctrl-E the end, Ctrl-U cuts to the start,
Ctrl-W the word before the cursor, Ctrl-L clears the screen; Ctrl-D
deletes under the cursor, and ends the session on an empty line, as
Ctrl-C does";

/// The command names, for completion.
const COMMANDS: [&str; 19] = [
    "ls", "cat", "stat", "tree", "dmesg", "watch", "cd", "pwd", "lookup",
    "info", "children", "read", "write", "call", "log", "load", "unload",
    "help", "quit",
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
    ("cd", Step::Op, Error::Unsupported, "not a Container; cat it"),
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

/// Where the output goes.
///
/// The terminal's columns, which `ls` lays its names out across, are
/// `None` for a pipe and then it writes one name per line; the palette
/// is on when the app allows color. The app knows both, and this crate
/// asks neither the terminal nor the environment.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Out {
    pub width: Option<u16>,
    pub paint: Paint,
}

impl Out {
    /// A pipe: one name per line, no color.
    #[must_use]
    pub const fn plain() -> Self {
        Self { width: None, paint: Paint::off() }
    }

    /// A terminal `width` columns wide, colored or not.
    #[must_use]
    pub const fn screen(width: u16, color: bool) -> Self {
        let paint = if color { Paint::on() } else { Paint::off() };
        Self { width: Some(width), paint }
    }
}

/// One command already split into words; what goes to stdout, text or
/// raw bytes. `shell` is where the line stands: what a relative target
/// is read from, and what `cd` moves.
///
/// `write` without bytes needs stdin, so the app fills the argument
/// before calling. A word that follows the tree prints here what it
/// has now and nothing more: the loop after it is the app's, from
/// [`following`].
///
/// # Errors
/// `Miss` for an unknown word, the wrong number of words, `quit`
/// outside an interactive session, or whatever the Client says.
pub fn input(
    client: &mut Client,
    shell: &mut Shell,
    words: &[String],
    out: Out,
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
        ("cd", 1 | 2) => return cd(client, shell, words.get(1)),
        ("pwd", 1) => return Ok(format!("{}\n", shell.path).into_bytes()),
        ("ls", _) => return ls(client, shell, &words[1..], out),
        ("cat", 2..) => return cat(client, shell, &words[1..]),
        ("stat", 2) => return stat(client, shell, &words[1], out),
        ("tree", _) => return tree(client, shell, &words[1..], out),
        ("dmesg", _) => return dmesg_now(client, &words[1..], out),
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
        ("lookup" | "info" | "children" | "read" | "watch", 2)
        | ("write", 3)
        | ("call", 3 | 4) => {}
        _ => return Err(misuse(word)),
    }
    let target = &words[1];
    let rd = resolved(client, shell, word, target)?;
    let refused = |fault: Fault| miss(word, target, Step::Op, &fault, &[]);
    match word {
        "lookup" => Ok(format!("{}\n", rd.0).into_bytes()),
        // `watch` prints nothing until an event; the loop is the app's.
        "watch" => Ok(Vec::new()),
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

/// Where the line stands in the tree: the rd a relative target is read
/// from, the path the prompt and `pwd` show, and the one before it,
/// which `cd -` goes back to.
///
/// The app holds one for a session of lines and passes it to every
/// word; a single command has its own, which starts and stays at the
/// root.
#[derive(Clone, Debug)]
pub struct Shell {
    cwd: Rd,
    path: String,
    back: Option<(Rd, String)>,
}

impl Default for Shell {
    /// The root, where every session starts.
    fn default() -> Self {
        Self { cwd: ROOT, path: "/".to_string(), back: None }
    }
}

impl Shell {
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

/// `cd`: the root with no target, the one before it under `-`, else
/// the Container the target names, whose path the prompt then shows.
fn cd(
    client: &mut Client,
    shell: &mut Shell,
    target: Option<&String>,
) -> Result<Vec<u8>, Miss> {
    let target = target.map_or("/", String::as_str);
    if target == "-" {
        let Some((rd, path)) = shell.back.clone() else {
            let (word, target) = ("cd".to_string(), target.to_string());
            let cause = "nowhere to go back to".to_string();
            return Err(Miss { word, target, cause });
        };
        shell.go(rd, path);
        return Ok(Vec::new());
    }
    let rd = resolved(client, shell, "cd", target)?;
    let refused = |fault: Fault| miss("cd", target, Step::Op, &fault, &[]);
    let info = client.info(rd).map_err(refused)?;
    if !info.flags.contains(Flags::CONTAINER) {
        return Err(refused(Fault::Kernel(Error::Unsupported)));
    }
    let path = where_is(client, rd).map_err(refused)?;
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
fn where_is(client: &mut Client, rd: Rd) -> Result<String, Fault> {
    let mut names: Vec<String> = Vec::new();
    let mut at = rd;
    while at != ROOT {
        let name = client.name(at)?;
        if name.is_empty() || names.len() == DEEP {
            return Ok(format!("rd:{}", rd.0));
        }
        names.push(name);
        at = client.parent(at)?;
    }
    names.reverse();
    Ok(format!("/{}", names.join("/")))
}

/// The rd of `target` from where the shell stands, the one walk every
/// word takes.
///
/// `rd:<n>` is that rd wherever the line stands. Anything else is
/// walked segment by segment, from the root when it begins with `/`
/// and from the cwd when it does not, `.` staying where it is and `..`
/// going to the parent; so a `.` or a `..` inside an absolute path is
/// a move and never a name to look up.
fn locate(
    client: &mut Client,
    shell: &Shell,
    target: &str,
) -> Result<Rd, (String, Fault)> {
    if target.starts_with("rd:") {
        return client.resolve_at(target);
    }
    let mut rd = if target.starts_with('/') { ROOT } else { shell.cwd };
    let named = |name: &&str| !name.is_empty() && *name != ".";
    for name in target.split('/').filter(named) {
        let step =
            if name == ".." { up(client, rd) } else { client.lookup(rd, name) };
        rd = step.map_err(|fault| (name.to_string(), fault))?;
    }
    Ok(rd)
}

/// The parent of `rd`; the root is its own parent, as `cd /..` in a
/// shell is `/`.
fn up(client: &mut Client, rd: Rd) -> Result<Rd, Fault> {
    if rd == ROOT {
        return Ok(ROOT);
    }
    client.parent(rd)
}

/// The rd of `target`, or the Miss naming the segment that stopped the
/// walk.
fn resolved(
    client: &mut Client,
    shell: &Shell,
    word: &str,
    target: &str,
) -> Result<Rd, Miss> {
    let stopped = |(at, f): (String, Fault)| {
        miss(word, target, Step::Resolve, &f, &[&at])
    };
    locate(client, shell, target).map_err(stopped)
}

/// One listed rd: its number, the name it shows under, and what the
/// Kernel caches about it.
struct Entry {
    rd: Rd,
    name: String,
    info: Info,
}

/// How `ls` writes a listing: `-l` the long line, `-a` the rd first,
/// `-w` the listing again after every event under the target.
#[derive(Clone, Copy, Default)]
struct Form {
    long: bool,
    rds: bool,
    follow: bool,
}

/// The children of each target by name, sorted; in columns across
/// `width` and one per line without it. A cell lists itself.
fn ls(
    client: &mut Client,
    shell: &Shell,
    args: &[String],
    out: Out,
) -> Result<Vec<u8>, Miss> {
    let Some((form, mut targets)) = options(args) else {
        return Err(misuse("ls"));
    };
    if targets.is_empty() {
        targets.push(&shell.path);
    }
    let many = targets.len() > 1;
    let mut text = String::new();
    for target in targets {
        let gap = if text.is_empty() { "" } else { "\n" };
        if many {
            let _ = writeln!(text, "{gap}{target}:");
        }
        text.push_str(&listing(client, shell, target, form, out)?);
    }
    Ok(text.into_bytes())
}

/// The `-l` and `-a` of `args` and the targets after them; `None` for
/// a flag `ls` does not take.
fn options(args: &[String]) -> Option<(Form, Vec<&str>)> {
    let mut form = Form::default();
    let mut targets = Vec::new();
    for arg in args {
        let Some(letters) = flags(arg) else {
            targets.push(arg.as_str());
            continue;
        };
        for letter in letters.chars() {
            match letter {
                'l' => form.long = true,
                'a' => form.rds = true,
                'w' => form.follow = true,
                _ => return None,
            }
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
    shell: &Shell,
    target: &str,
    form: Form,
    out: Out,
) -> Result<String, Miss> {
    let rd = resolved(client, shell, "ls", target)?;
    let refused = |fault: Fault| miss("ls", target, Step::Op, &fault, &[]);
    let info = client.info(rd).map_err(refused)?;
    let listed = if info.flags.contains(Flags::CONTAINER) {
        entries(client, rd).map_err(refused)?
    } else {
        vec![Entry { name: named(rd, &info), rd, info }]
    };
    if form.long {
        let mut text = String::new();
        for entry in &listed {
            let paint = out.paint;
            let line = long_line(client, entry, form.rds, paint);
            text.push_str(&line.map_err(refused)?);
        }
        return Ok(text);
    }
    let text = |entry: &Entry| {
        let name = painted(entry, out.paint);
        if form.rds { format!("{} {name}", entry.rd.0) } else { name }
    };
    let names: Vec<String> = listed.iter().map(text).collect();
    Ok(laid_out(&names, out.width))
}

/// `names` as a listing shows them: down the columns that fit in
/// `width`, one per line without it. What Tab shows of several
/// candidates, laid out the way `ls` lays out a listing.
#[must_use]
pub fn laid_out(names: &[String], width: Option<u16>) -> String {
    width.map_or_else(
        || lines(names),
        |width| columns(names, usize::from(width)),
    )
}

/// A name as a listing shows it: a Container bold blue, a cell that
/// offers an api green, anything else as it is.
fn painted(entry: &Entry, paint: Paint) -> String {
    let name = &entry.name;
    if entry.info.flags.contains(Flags::CONTAINER) {
        paint.blue(name)
    } else if entry.info.apis.is_empty() {
        name.clone()
    } else {
        paint.green(name)
    }
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
    paint: Paint,
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
    let name = painted(entry, paint);
    Ok(format!("{number}{kind} {read}{write}  {at}  {apis}  {name}\n"))
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

/// The cells `text` takes on a terminal row; an SGR sequence from
/// `paint` takes none, so a colored name lays out as its bare one.
fn cells(text: &str) -> usize {
    let mut cells = 0;
    let mut rest = text;
    while let Some(at) = rest.find('\x1b') {
        cells += rest[..at].chars().map(reovim_client::width).sum::<usize>();
        let end = rest[at..].find('m').map_or(rest.len(), |m| at + m + 1);
        rest = &rest[end..];
    }
    cells + rest.chars().map(reovim_client::width).sum::<usize>()
}

/// One instant of the Kernel clock as a calendar reads it, in UTC.
/// `weekday` counts Sunday 0, as `dmesg -T` names the day.
struct Civil {
    year: u64,
    month: u64,
    day: u64,
    hour: u64,
    minute: u64,
    second: u64,
    weekday: u64,
}

/// The civil date of the Kernel clock's nanoseconds, by the
/// civil-from-days arithmetic, which needs no table and no host.
fn civil(nanos: u64) -> Civil {
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
    // 1970-01-01 was a Thursday, the fourth day of the week.
    Civil {
        year,
        month,
        day,
        hour: rest / 3600,
        minute: rest % 3600 / 60,
        second: rest % 60,
        weekday: (days + 4) % 7,
    }
}

/// `YYYY-MM-DD HH:MM:SS` in UTC, as `ls -l` and `stat` write a time.
fn date(nanos: u64) -> String {
    let c = civil(nanos);
    let (year, month, day) = (c.year, c.month, c.day);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}",
        c.hour, c.minute, c.second
    )
}

/// How `cat` reads: `-f` (or `-w`) goes on printing the bytes after
/// every Modify, `-n N` until N of them.
#[derive(Clone, Copy, Default)]
struct Reading {
    follow: bool,
    stop: Option<usize>,
}

/// The flags of `cat` and the targets after them; `None` for a flag it
/// does not take, a count that is not a number, or no target at all.
fn reading(args: &[String]) -> Option<(Reading, Vec<&str>)> {
    let mut form = Reading::default();
    let mut targets = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let Some(letters) = flags(arg) else {
            targets.push(arg.as_str());
            continue;
        };
        for letter in letters.chars() {
            match letter {
                'f' | 'w' => form.follow = true,
                'n' => form.stop = Some(args.next()?.parse().ok()?),
                _ => return None,
            }
        }
    }
    (!targets.is_empty()).then_some((form, targets))
}

/// The bytes of each target in turn.
fn cat(
    client: &mut Client,
    shell: &Shell,
    args: &[String],
) -> Result<Vec<u8>, Miss> {
    let Some((_, targets)) = reading(args) else {
        return Err(misuse("cat"));
    };
    let mut out = Vec::new();
    for target in targets {
        let rd = resolved(client, shell, "cat", target)?;
        let refused = |f: Fault| miss("cat", target, Step::Op, &f, &[]);
        out.extend(client.read_all(rd).map_err(refused)?);
    }
    Ok(out)
}

/// The long line, the three times as dates, then every api with its
/// methods as the declaration shows them.
fn stat(
    client: &mut Client,
    shell: &Shell,
    target: &str,
    out: Out,
) -> Result<Vec<u8>, Miss> {
    let rd = resolved(client, shell, "stat", target)?;
    let refused = |fault: Fault| miss("stat", target, Step::Op, &fault, &[]);
    let info = client.info(rd).map_err(refused)?;
    let entry = Entry { name: named(rd, &info), rd, info };
    let line = long_line(client, &entry, false, out.paint);
    let mut out = line.map_err(refused)?;
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
fn tree(
    client: &mut Client,
    shell: &Shell,
    args: &[String],
    out: Out,
) -> Result<Vec<u8>, Miss> {
    let Some((depth, target)) = levels(args, &shell.path) else {
        return Err(misuse("tree"));
    };
    let rd = resolved(client, shell, "tree", target)?;
    let refused = |fault: Fault| miss("tree", target, Step::Op, &fault, &[]);
    let info = client.info(rd).map_err(refused)?;
    let mut text = format!("{target}\n");
    let mut count = (0, 0);
    if info.flags.contains(Flags::CONTAINER) {
        let kids = ordered(entries(client, rd).map_err(refused)?);
        let deeper = Deeper { prefix: String::new(), depth, paint: out.paint };
        draw(client, &kids, &deeper, &mut text, &mut count);
    }
    let _ = write!(text, "\n{} containers, {} cells\n", count.0, count.1);
    Ok(text.into_bytes())
}

/// The `-L N` of `args` and the one target after it, `here` without
/// one; `None` for a flag `tree` does not take or a depth that is not
/// a number above zero.
fn levels<'a>(args: &'a [String], here: &'a str) -> Option<(usize, &'a str)> {
    let (mut depth, mut target) = (usize::MAX, here);
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

/// Where `draw` is: what each line starts with, how many levels are
/// left, and the palette the names are written in.
struct Deeper {
    prefix: String,
    depth: usize,
    paint: Paint,
}

/// `kids` under `at.prefix`, each Container a level deeper; what
/// cannot be listed says why on its own line and the walk goes on.
fn draw(
    client: &mut Client,
    kids: &[Entry],
    at: &Deeper,
    out: &mut String,
    count: &mut (usize, usize),
) {
    let prefix = &at.prefix;
    for (i, kid) in kids.iter().enumerate() {
        let last = i + 1 == kids.len();
        let stem = if last { "└── " } else { "├── " };
        let below = if last { "    " } else { "│   " };
        let container = kid.info.flags.contains(Flags::CONTAINER);
        if container {
            count.0 += 1;
        } else {
            count.1 += 1;
        }
        let name = painted(kid, at.paint);
        let under =
            (container && at.depth > 1).then(|| entries(client, kid.rd));
        match under {
            Some(Err(fault)) => {
                let why = cause("tree", Step::Op, &fault, &[]);
                let _ = writeln!(out, "{prefix}{stem}{name}: {why}");
            }
            Some(Ok(kids)) => {
                let _ = writeln!(out, "{prefix}{stem}{name}");
                let deeper = Deeper {
                    prefix: format!("{prefix}{below}"),
                    depth: at.depth - 1,
                    paint: at.paint,
                };
                draw(client, &ordered(kids), &deeper, out, count);
            }
            None => {
                let _ = writeln!(out, "{prefix}{stem}{name}");
            }
        }
    }
}

/// Where `dmesg` reads from; the cli knows no Module, and this is a
/// path in the tree like any other.
const LOG: &str = "/sys/log";

/// What `dmesg` prints of everything the log holds now.
fn dmesg_now(
    client: &mut Client,
    args: &[String],
    out: Out,
) -> Result<Vec<u8>, Miss> {
    let (form, _) = dmesg::form(args, out.paint)?;
    let refused = |f: Fault| miss("dmesg", LOG, Step::Op, &f, &[]);
    let entries = log::tail(client, 1).map_err(refused)?;
    let boot = dmesg::boot(&entries);
    let mut last = None;
    Ok(dmesg::show(&entries, boot, &mut last, &form).into_bytes())
}

/// A word that goes on printing after the line is typed: what the app
/// watches, what it prints at once, and what it makes of each event
/// after that. One loop in the app serves all of them.
pub struct Follow {
    /// The handles watched; the app closes them when the loop ends.
    pub handles: Vec<Handle>,
    /// What to print before the first event.
    pub first: Vec<u8>,
    /// How many prints end the loop; `None` until Ctrl-C.
    pub limit: Option<usize>,
    what: Watching,
}

/// What each following word makes of an event.
enum Watching {
    /// `dmesg -w`: every new entry, in the form the line asked for.
    Log(Tailing),
    /// `cat -f`: the bytes of the cell again on every Modify.
    Cell { rds: Vec<Rd> },
    /// `watch`: `seq kind name` for each event on the target.
    Events { rd: Rd, name: String, paint: Paint },
    /// `ls -w`: the listing again after every event under the target,
    /// from where the line stood when it was typed.
    List { rds: Vec<Rd>, args: Vec<String>, shell: Shell, out: Out },
}

/// `dmesg -w` between events: the watched log, the sequence to read
/// from, and what the forms need to write the next line.
struct Tailing {
    h: Handle,
    rd: Rd,
    from: u64,
    boot: u64,
    last: Option<u64>,
    form: dmesg::Form,
}

impl Tailing {
    /// Whatever the log gained, in the form the line asked for.
    fn event(
        &mut self,
        client: &mut Client,
        event: Event,
    ) -> Result<Option<Vec<u8>>, Miss> {
        if event.rd != self.rd || event.kind != Kind::Modify {
            return Ok(None);
        }
        let refused = |f: Fault| miss("dmesg", LOG, Step::Op, &f, &[]);
        let got = log::read(client, self.h, self.from).map_err(refused)?;
        let Some(newest) = got.last() else { return Ok(None) };
        self.from = newest.seq + 1;
        let text = dmesg::show(&got, self.boot, &mut self.last, &self.form);
        Ok(some(text.into_bytes()))
    }
}

/// The plan for a line that follows the tree, or `None` for an
/// ordinary command, which [`input`] runs whole.
///
/// The handles are open and watched when this returns; the app polls
/// `since`, gives each event to [`Follow::event`] and closes the
/// handles when its loop ends.
///
/// # Errors
/// As [`input`]: the flags of the word, and whatever the Client says
/// while it opens and watches the target.
pub fn following(
    client: &mut Client,
    shell: &Shell,
    words: &[String],
    out: Out,
) -> Result<Option<Follow>, Miss> {
    let Some(word) = words.first().map(String::as_str) else {
        return Ok(None);
    };
    let args = &words[1..];
    match (word, words.len()) {
        ("dmesg", _) => dmesg_follow(client, shell, args, out),
        ("cat", 2..) => cat_follow(client, shell, args, out),
        ("ls", _) => ls_follow(client, shell, args, out),
        ("watch", 2) => watch_follow(client, shell, &words[1], out).map(Some),
        _ => Ok(None),
    }
}

/// What the ring already holds is not a following word's to print: the
/// cursor starts at what has happened by now, and the word prints the
/// state it reads after that.
fn drain(client: &mut Client, word: &str) -> Result<(), Miss> {
    let refused = |f: Fault| miss(word, "", Step::Op, &f, &[]);
    client.since().map(|_| ()).map_err(refused)
}

/// A handle on `rd` open and watching, its faults named as the word's.
fn watching(
    client: &mut Client,
    word: &str,
    target: &str,
    rd: Rd,
) -> Result<Handle, Miss> {
    let refused = |f: Fault| miss(word, target, Step::Op, &f, &[]);
    let h = client.open(rd, Rights::READ).map_err(refused)?;
    client.watch(h, true).map_err(refused)?;
    Ok(h)
}

fn dmesg_follow(
    client: &mut Client,
    shell: &Shell,
    args: &[String],
    out: Out,
) -> Result<Option<Follow>, Miss> {
    let (form, tail) = dmesg::form(args, out.paint)?;
    if tail == dmesg::Tail::No {
        return Ok(None);
    }
    let rd = resolved(client, shell, "dmesg", LOG)?;
    let h = watching(client, "dmesg", LOG, rd)?;
    drain(client, "dmesg")?;
    let refused = |f: Fault| miss("dmesg", LOG, Step::Op, &f, &[]);
    let entries = log::read(client, h, 1).map_err(refused)?;
    let boot = dmesg::boot(&entries);
    let from = entries.last().map_or(1, |entry| entry.seq + 1);
    let mut last = None;
    let first = match tail {
        dmesg::Tail::All => dmesg::show(&entries, boot, &mut last, &form),
        _ => String::new(),
    };
    let what = Watching::Log(Tailing { h, rd, from, boot, last, form });
    Ok(Some(Follow {
        handles: vec![h],
        first: first.into_bytes(),
        limit: None,
        what,
    }))
}

fn cat_follow(
    client: &mut Client,
    shell: &Shell,
    args: &[String],
    out: Out,
) -> Result<Option<Follow>, Miss> {
    let Some((form, targets)) = reading(args) else {
        return Err(misuse("cat"));
    };
    if !form.follow {
        return Ok(None);
    }
    let mut handles = Vec::new();
    let mut rds = Vec::new();
    for target in &targets {
        let rd = resolved(client, shell, "cat", target)?;
        handles.push(watching(client, "cat", target, rd)?);
        rds.push(rd);
    }
    drain(client, "cat")?;
    let first = cat(client, shell, args)?;
    let what = Watching::Cell { rds };
    let _ = out;
    Ok(Some(Follow { handles, first, limit: form.stop, what }))
}

fn ls_follow(
    client: &mut Client,
    shell: &Shell,
    args: &[String],
    out: Out,
) -> Result<Option<Follow>, Miss> {
    let Some((form, mut targets)) = options(args) else {
        return Err(misuse("ls"));
    };
    if !form.follow {
        return Ok(None);
    }
    if targets.is_empty() {
        targets.push(&shell.path);
    }
    let mut handles = Vec::new();
    let mut rds = Vec::new();
    for target in &targets {
        let rd = resolved(client, shell, "ls", target)?;
        handles.push(watching(client, "ls", target, rd)?);
        rds.push(rd);
    }
    drain(client, "ls")?;
    let first = ls(client, shell, args, out)?;
    let (args, shell) = (args.to_vec(), shell.clone());
    let what = Watching::List { rds, args, shell, out };
    Ok(Some(Follow { handles, first, limit: None, what }))
}

fn watch_follow(
    client: &mut Client,
    shell: &Shell,
    target: &str,
    out: Out,
) -> Result<Follow, Miss> {
    let rd = resolved(client, shell, "watch", target)?;
    let h = watching(client, "watch", target, rd)?;
    drain(client, "watch")?;
    let refused = |f: Fault| miss("watch", target, Step::Op, &f, &[]);
    let info = client.info(rd).map_err(refused)?;
    let name = named(rd, &info);
    let what = Watching::Events { rd, name, paint: out.paint };
    Ok(Follow { handles: vec![h], first: Vec::new(), limit: None, what })
}

impl Follow {
    /// What this event prints, if anything; `None` for an event on
    /// something else, or one this word has nothing to say about, and
    /// then it counts for nothing.
    ///
    /// # Errors
    /// Whatever the Client says while it reads what the event names.
    pub fn event(
        &mut self,
        client: &mut Client,
        event: Event,
    ) -> Result<Option<Vec<u8>>, Miss> {
        match &mut self.what {
            Watching::Log(tail) => tail.event(client, event),
            Watching::Cell { rds } => again(client, event, rds),
            Watching::Events { rd, name, paint } => {
                let shown = event.rd == *rd;
                let line = || happened(client, event, name, *paint);
                Ok(shown.then(line).map(String::into_bytes))
            }
            Watching::List { rds, args, shell, out } => {
                let shown = rds.contains(&event.rd);
                shown.then(|| ls(client, shell, args, *out)).transpose()
            }
        }
    }
}

/// `cat -f`: the bytes of the cell that changed, all of them again.
fn again(
    client: &mut Client,
    event: Event,
    rds: &[Rd],
) -> Result<Option<Vec<u8>>, Miss> {
    if event.kind != Kind::Modify || !rds.contains(&event.rd) {
        return Ok(None);
    }
    let target = format!("rd:{}", event.rd.0);
    let refused = |f: Fault| miss("cat", &target, Step::Op, &f, &[]);
    Ok(some(client.read_all(event.rd).map_err(refused)?))
}

/// Bytes worth printing; nothing for nothing.
fn some(bytes: Vec<u8>) -> Option<Vec<u8>> {
    (!bytes.is_empty()).then_some(bytes)
}

/// One event as `watch` prints it: `seq kind name`, the name of the
/// child where there is one, else of the target itself.
fn happened(
    client: &mut Client,
    event: Event,
    name: &str,
    paint: Paint,
) -> String {
    let named_child = |child: Rd| {
        let info = client.info(child).ok();
        info.map_or_else(|| format!("rd:{}", child.0), |i| named(child, &i))
    };
    let name = event.child.map_or_else(|| name.to_string(), named_child);
    format!("{} {} {name}\n", event.seq, happening(event.kind, paint))
}

/// The kind of an event, colored as what it did: a Create green, a
/// removal red, an Attrib or a Move yellow, a Modify as it is.
fn happening(kind: Kind, paint: Paint) -> String {
    match kind {
        Kind::Modify => "Modify".to_string(),
        Kind::Attrib => paint.yellow("Attrib"),
        Kind::Create => paint.green("Create"),
        Kind::Delete => paint.red("Delete"),
        Kind::Move => paint.yellow("Move"),
        Kind::Gone => paint.red("Gone"),
        Kind::Overflow => paint.bold("Overflow"),
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
