#![forbid(unsafe_code)]
//! The `reovim` binary: `server` boots the Kernel with every linked
//! module behind one link; `cli` and `tui` drive a server from the
//! shell and the terminal; bare `reovim FILE` is server and tui in one
//! process.

mod cli;
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

/// Why the binary stops, printed once on stderr.
pub enum Fail {
    Usage(String),
    Host(reovim_host::Error),
    Client(Fault),
    /// SIGTERM or SIGHUP: the terminal restored, nothing written.
    Signal,
}

impl fmt::Display for Fail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(text) => write!(f, "{text}"),
            Self::Host(error) => write!(f, "host: {error:?}"),
            Self::Client(fault) => write!(f, "{fault}"),
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

impl From<reovim_kernel_interface::Error> for Fail {
    fn from(error: reovim_kernel_interface::Error) -> Self {
        Self::Client(Fault::Kernel(error))
    }
}

const USAGE: &str = "\
usage: reovim server [--socket PATH] [--root DIR] [--log PATH]
       reovim cli [--socket PATH] [COMMAND...]
       reovim tui [--socket PATH] FILE
       reovim [--socket PATH] [--log PATH] FILE
defaults: --socket <runtime dir>/reovim.sock, --root <working directory>;
the bare form edits any FILE and listens only when given --socket";

/// `--socket`, `--root` and `--log` taken off the front of `args`.
struct Options {
    socket: Option<String>,
    root: Option<String>,
    log: Option<String>,
    rest: Vec<String>,
}

fn options(args: Vec<String>) -> Result<Options, Fail> {
    let mut socket = None;
    let mut root = None;
    let mut log = None;
    let mut args = args.into_iter();
    let mut rest = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--socket" => socket = Some(args.next().ok_or_else(usage)?),
            "--root" => root = Some(args.next().ok_or_else(usage)?),
            "--log" => log = Some(args.next().ok_or_else(usage)?),
            _ => {
                rest.push(arg);
                rest.extend(args);
                break;
            }
        }
    }
    Ok(Options { socket, root, log, rest })
}

/// `--socket`, or the one in the runtime directory.
fn socket(given: Option<String>) -> Result<String, Fail> {
    match given {
        Some(socket) => Ok(socket),
        None => Ok(format!("{}/reovim.sock", reovim_host::dirs::runtime()?)),
    }
}

fn usage() -> Fail {
    Fail::Usage(USAGE.to_string())
}

fn run(args: Vec<String>) -> Result<ExitCode, Fail> {
    let mut args = args.into_iter().peekable();
    let command = match args.peek().map(String::as_str) {
        Some("server" | "cli" | "tui") => args.next(),
        _ => None,
    };
    let Options { socket: given, root, log, rest } = options(args.collect())?;
    match (command.as_deref(), rest.as_slice()) {
        (Some("server"), []) => {
            let root = match root {
                Some(root) => root,
                None => reovim_host::dirs::work()?,
            };
            server::run(&socket(given)?, &root, log.as_deref())
                .map(|()| ExitCode::SUCCESS)
        }
        (Some("cli"), rest) => {
            cli::run(&socket(given)?, rest).map(|()| ExitCode::SUCCESS)
        }
        (Some("tui"), [file]) => {
            tui::remote(&socket(given)?, file).map(ExitCode::from)
        }
        (None, [file]) if root.is_none() => {
            tui::bare(given.as_deref(), log.as_deref(), file)
                .map(ExitCode::from)
        }
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
