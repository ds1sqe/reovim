#![forbid(unsafe_code)]
//! The `reovim` binary: `server` boots the Kernel with every linked
//! module behind one link; `cli` and `tui` drive a server from the
//! shell and the terminal; `view` reads a display without holding it;
//! bare `reovim FILE` is server and tui in one process.

mod cli;
mod client;
mod dump;
mod local;
mod log;
mod remote;
mod server;
mod tui;
mod view;

/// The module table build.rs wrote from `modules`.
mod link {
    use reovim_kernel_interface::Booted;
    include!(concat!(env!("OUT_DIR"), "/link.rs"));
}

use std::fmt;
use std::io::Write as _;
use std::process::ExitCode;

use reovim_client::Fault;
use reovim_client_sh::Miss;

/// Why the binary stops, printed once on stderr.
pub enum Fail {
    Usage(String),
    Host(reovim_host::Error),
    Client(Fault),
    /// One typed line, as the shell says it.
    Cli(Miss),
    /// A file the editor has no room for, as the name was given. The
    /// Buffers hold a bounded number of bytes in all, and one file
    /// past that opens in no editor.
    Big(String),
    /// A `--sys NAME=VALUE` the Kernel refused; nothing has started.
    Sys(String, String, reovim_kernel_interface::Error),
    /// SIGTERM or SIGHUP: the terminal restored, nothing written.
    Signal,
}

impl fmt::Display for Fail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(text) => write!(f, "{text}"),
            Self::Host(error) => write!(f, "host: {error:?}"),
            Self::Client(fault) => write!(f, "{fault}"),
            Self::Cli(miss) => write!(f, "{miss}"),
            Self::Big(name) => write!(f, "{name}: more than the editor holds"),
            Self::Sys(name, value, error) => {
                write!(f, "--sys {name}={value}: {}", Fault::Kernel(*error))
            }
            Self::Signal => write!(f, "ended by a signal"),
        }
    }
}

impl From<reovim_host::Error> for Fail {
    fn from(error: reovim_host::Error) -> Self {
        Self::Host(error)
    }
}

impl From<Fault> for Fail {
    fn from(fault: Fault) -> Self {
        Self::Client(fault)
    }
}

impl From<Miss> for Fail {
    fn from(miss: Miss) -> Self {
        Self::Cli(miss)
    }
}

impl From<reovim_kernel_interface::Error> for Fail {
    fn from(error: reovim_kernel_interface::Error) -> Self {
        Self::Client(Fault::Kernel(error))
    }
}

const USAGE: &str = "\
usage: reovim server [--socket PATH] [--root DIR] [--log PATH] [--sys N=V]
       reovim cli [--socket PATH] [--color[=WHEN]] [--help] [LINE...]
       reovim tui [--socket PATH] [+N] [FILE...]
       reovim view [--socket PATH] [--color[=WHEN]] DISPLAY
       reovim [--socket PATH] [--log PATH] [--sys N=V] [+N] [FILE...]
defaults: --socket <runtime dir>/reovim.sock, --root <working directory>;
the bare form edits every FILE, or none; the first is shown and +N is
its line; it listens at the default socket, or at reovim-<pid>.sock
while another editor holds it;
--sys NAME=VALUE writes /sys/NAME before the first module loads, and
may be given more than once (--sys loglevel=debug --sys
limit/log_ring=65536);
the cli runs one typed LINE and exits, or reads lines until the end of
the input; help typed at it lists every name PATH holds;
view draws DISPLAY (a display's own directory, `.../display/0`) at
this terminal's own size until q or Ctrl-C, drawing again on a change
or a resize; it types nothing into what it shows";

/// When the output may carry color: `--color=WHEN`, and `--color`
/// alone as GNU ls takes it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum When {
    Auto,
    Always,
    Never,
}

/// `--socket`, `--root`, `--log`, `--color` and every `--sys` taken
/// off the front of `args`.
struct Options {
    socket: Option<String>,
    root: Option<String>,
    log: Option<String>,
    color: When,
    /// Each `--sys NAME=VALUE`, in the order given.
    sys: Vec<(String, String)>,
    rest: Vec<String>,
}

fn options(args: Vec<String>) -> Result<Options, Fail> {
    let mut socket = None;
    let mut root = None;
    let mut log = None;
    let mut color = When::Auto;
    let mut sys = Vec::new();
    let mut args = args.into_iter();
    let mut rest = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--socket" => socket = Some(args.next().ok_or_else(usage)?),
            "--root" => root = Some(args.next().ok_or_else(usage)?),
            "--log" => log = Some(args.next().ok_or_else(usage)?),
            "--sys" => sys.push(named(&args.next().ok_or_else(usage)?)?),
            "--color" | "--color=always" => color = When::Always,
            "--color=never" => color = When::Never,
            "--color=auto" => color = When::Auto,
            _ => {
                rest.push(arg);
                rest.extend(args);
                break;
            }
        }
    }
    Ok(Options { socket, root, log, color, sys, rest })
}

/// One `--sys` argument split at its first `=`; a name with no value
/// is a usage error, since every parameter is written something.
fn named(arg: &str) -> Result<(String, String), Fail> {
    let (name, value) = arg.split_once('=').ok_or_else(usage)?;
    if name.is_empty() {
        return Err(usage());
    }
    Ok((name.to_string(), value.to_string()))
}

/// `--socket`, or the one in the runtime directory.
fn socket(given: Option<String>) -> Result<String, Fail> {
    match given {
        Some(socket) => Ok(socket),
        None => Ok(format!("{}/reovim.sock", reovim_host::dirs::runtime()?)),
    }
}

/// Where the bare editor listens: `--socket` as given, else the
/// default socket while nothing answers there, else one named by this
/// process, so a second editor neither takes the first one's socket nor
/// goes without.
fn free_socket(given: Option<String>) -> Result<String, Fail> {
    if given.is_some() {
        return socket(given);
    }
    let dir = reovim_host::dirs::runtime()?;
    let default = format!("{dir}/reovim.sock");
    // A link that opens says someone listens there; the probe ends
    // before it is served. Anything else is a free name, or a socket
    // file a killed editor left for `listen` to replace.
    let taken =
        reovim_host::link::open(&default).map(reovim_host::link::close).is_ok();
    if taken {
        return Ok(format!("{dir}/reovim-{}.sock", std::process::id()));
    }
    Ok(default)
}

fn usage() -> Fail {
    Fail::Usage(USAGE.to_string())
}

/// Whether the output carries color: forced either way by `--color`,
/// else on when stdout is a terminal and `NO_COLOR` is unset or empty,
/// as no-color.org asks. The library is told; it never asks itself.
fn colored(when: When) -> bool {
    let no_color = std::env::var("NO_COLOR").is_ok_and(|set| !set.is_empty());
    match when {
        When::Always => true,
        When::Never => false,
        When::Auto => reovim_host::term::is_tty_out() && !no_color,
    }
}

fn run(args: Vec<String>) -> Result<ExitCode, Fail> {
    let mut args = args.into_iter().peekable();
    let command = match args.peek().map(String::as_str) {
        Some("server" | "cli" | "tui" | "view") => args.next(),
        _ => None,
    };
    let Options { socket: given, root, log, color, sys, rest } =
        options(args.collect())?;
    // Only the two forms that hold a Kernel take a parameter for one.
    let plain = sys.is_empty();
    match (command.as_deref(), rest.as_slice()) {
        (Some("server"), []) => {
            let root = match root {
                Some(root) => root,
                None => reovim_host::dirs::work()?,
            };
            server::run(&socket(given)?, &root, log.as_deref(), &sys)
                .map(|()| ExitCode::SUCCESS)
        }
        // The app's own usage, which opens no link; the shell's `help`
        // is the one that reads the tree.
        (Some("cli"), [asked]) if asked == "--help" => {
            println!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
        (Some("cli"), rest) if plain => {
            cli::run(&socket(given)?, rest, colored(color))
                .map(|()| ExitCode::SUCCESS)
        }
        (Some("tui"), rest) if plain => {
            let (line, files) = files_at(rest)?;
            tui::remote(&socket(given)?, line, files).map(ExitCode::from)
        }
        (Some("view"), [target]) if plain => {
            view::run(&socket(given)?, target, colored(color))
                .map(|()| ExitCode::SUCCESS)
        }
        (None, rest) if root.is_none() => {
            let (line, files) = files_at(rest)?;
            let socket = free_socket(given)?;
            tui::bare(&socket, log.as_deref(), &sys, line, files)
                .map(ExitCode::from)
        }
        _ => Err(usage()),
    }
}

/// `[+N] [FILE...]`: the line to start on and every file, in order.
fn files_at(rest: &[String]) -> Result<(Option<usize>, &[String]), Fail> {
    match rest {
        [line, rest @ ..] if line.starts_with('+') => {
            Ok((Some(line[1..].parse().map_err(|_| usage())?), rest))
        }
        _ => Ok((None, rest)),
    }
}

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(code) => code,
        Err(fail) => {
            // `eprintln!` panics on a failed write; a terminal that
            // hung up (`Fail::Signal`) takes stderr down with it, so
            // the write's own result is dropped instead.
            let _ = writeln!(std::io::stderr(), "reovim: {fail}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{When, files_at, options};

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn options_stop_at_the_first_positional() {
        let given = [
            "--socket",
            "s",
            "--log",
            "l",
            "--color=never",
            "+3",
            "f",
            "--root",
            "r",
        ];
        let found = options(args(&given)).ok().unwrap();
        assert_eq!(found.socket.as_deref(), Some("s"));
        assert!(found.color == When::Never);
        assert_eq!(found.log.as_deref(), Some("l"));
        assert_eq!(found.root, None);
        assert_eq!(found.rest, args(&["+3", "f", "--root", "r"]));
        assert!(
            options(args(&["--socket"])).is_err(),
            "a flag needs its value"
        );
        let none = options(args(&[])).ok().unwrap();
        assert_eq!(none.rest, Vec::<String>::new());
        assert!(none.color == When::Auto, "no --color asks the terminal");
    }

    #[test]
    fn files_may_carry_a_line_and_may_be_none() {
        let (one, both) = (args(&["f"]), args(&["+7", "f"]));
        let (none, line) = (args(&[]), args(&["+7"]));
        let two = args(&["a", "b"]);
        let three = args(&["+2", "a", "b"]);
        assert_eq!(files_at(&one).ok(), Some((None, &args(&["f"])[..])));
        assert_eq!(files_at(&both).ok(), Some((Some(7), &args(&["f"])[..])));
        assert_eq!(files_at(&none).ok(), Some((None, &[][..])));
        assert_eq!(files_at(&line).ok(), Some((Some(7), &[][..])));
        assert_eq!(files_at(&two).ok(), Some((None, &args(&["a", "b"])[..])));
        assert_eq!(
            files_at(&three).ok(),
            Some((Some(2), &args(&["a", "b"])[..]))
        );
        assert!(files_at(&args(&["+x", "a"])).is_err());
    }
}
