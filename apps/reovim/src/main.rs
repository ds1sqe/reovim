#![forbid(unsafe_code)]
//! The `reovim` binary: `server` boots the Kernel with every linked
//! module behind one link; `cli` and `tui` drive a server from the
//! shell and the terminal; bare `reovim FILE` is server and tui in one
//! process.

mod cli;
mod dump;
mod local;
mod log;
mod remote;
mod server;
mod tui;

/// The module table build.rs wrote from `modules`.
mod link {
    use reovim_kernel_interface::Booted;
    include!(concat!(env!("OUT_DIR"), "/link.rs"));
}

use std::fmt;
use std::process::ExitCode;

use reovim_client::Fault;
use reovim_client_cli::Miss;

/// Why the binary stops, printed once on stderr.
pub enum Fail {
    Usage(String),
    Host(reovim_host::Error),
    Client(Fault),
    /// One command of the cli, in the cli's own words.
    Cli(Miss),
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
       reovim cli [--socket PATH] [--color[=WHEN]] [COMMAND...]
       reovim tui [--socket PATH] [+N] [FILE]
       reovim [--socket PATH] [--log PATH] [--sys N=V] [+N] [FILE]
defaults: --socket <runtime dir>/reovim.sock, --root <working directory>;
the bare form edits any FILE, or none, and listens at the default
socket, or at reovim-<pid>.sock while another editor holds it;
--sys NAME=VALUE writes /sys/NAME before the first module loads, and
may be given more than once (--sys loglevel=debug --sys
limit/log_ring=65536)";

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
        Some("server" | "cli" | "tui") => args.next(),
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
        (Some("cli"), rest) if plain => {
            let out = cli::out(colored(color));
            cli::run(&socket(given)?, rest, out).map(|()| ExitCode::SUCCESS)
        }
        (Some("tui"), rest) if plain => {
            let (line, file) = file_at(rest)?;
            tui::remote(&socket(given)?, line, file).map(ExitCode::from)
        }
        (None, rest) if root.is_none() => {
            let (line, file) = file_at(rest)?;
            let socket = free_socket(given)?;
            tui::bare(&socket, log.as_deref(), &sys, line, file)
                .map(ExitCode::from)
        }
        _ => Err(usage()),
    }
}

/// `[+N] [FILE]`: the line to start on and the file, each if given.
fn file_at(rest: &[String]) -> Result<(Option<usize>, Option<&str>), Fail> {
    let (line, rest) = match rest {
        [line, rest @ ..] if line.starts_with('+') => {
            (Some(line[1..].parse().map_err(|_| usage())?), rest)
        }
        _ => (None, rest),
    };
    match rest {
        [] => Ok((line, None)),
        [file] => Ok((line, Some(file.as_str()))),
        _ => Err(usage()),
    }
}

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(code) => code,
        Err(fail) => {
            eprintln!("reovim: {fail}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{When, file_at, options};

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
    fn a_file_may_carry_its_line_and_may_be_none() {
        let (file, both) = (args(&["f"]), args(&["+7", "f"]));
        let (none, line) = (args(&[]), args(&["+7"]));
        assert_eq!(file_at(&file).ok(), Some((None, Some("f"))));
        assert_eq!(file_at(&both).ok(), Some((Some(7), Some("f"))));
        assert_eq!(file_at(&none).ok(), Some((None, None)));
        assert_eq!(file_at(&line).ok(), Some((Some(7), None)));
        assert!(file_at(&args(&["+x", "f"])).is_err());
        assert!(file_at(&args(&["a", "b"])).is_err());
    }
}
