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
}

impl fmt::Display for Fail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(text) => write!(f, "{text}"),
            Self::Host(error) => write!(f, "host: {error:?}"),
            Self::Client(fault) => write!(f, "{fault}"),
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
       reovim [--socket PATH] [--root DIR] [--log PATH] FILE
defaults: --socket <runtime dir>/reovim.sock, --root <working directory>";

/// `--socket`, `--root` and `--log` taken off the front of `args`,
/// defaults from the host filled in.
struct Options {
    socket: String,
    root: String,
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
    let socket = match socket {
        Some(socket) => socket,
        None => format!("{}/reovim.sock", reovim_host::dirs::runtime()?),
    };
    let root = match root {
        Some(root) => root,
        None => reovim_host::dirs::work()?,
    };
    Ok(Options { socket, root, log, rest })
}

fn usage() -> Fail {
    Fail::Usage(USAGE.to_string())
}

fn run(args: Vec<String>) -> Result<(), Fail> {
    let mut args = args.into_iter().peekable();
    let command = match args.peek().map(String::as_str) {
        Some("server" | "cli" | "tui") => args.next(),
        _ => None,
    };
    let Options { socket, root, log, rest } = options(args.collect())?;
    match (command.as_deref(), rest.as_slice()) {
        (Some("server"), []) => server::run(&socket, &root, log.as_deref()),
        (Some("cli"), rest) => cli::run(&socket, rest),
        (Some("tui"), [file]) => tui::remote(&socket, file),
        (None, [file]) => tui::bare(&socket, &root, log.as_deref(), file),
        _ => Err(usage()),
    }
}

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(fail) => {
            eprintln!("reovim: {fail}");
            ExitCode::FAILURE
        }
    }
}
