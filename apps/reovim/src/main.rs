#![forbid(unsafe_code)]
//! The `reovim` binary: `server` boots the Kernel with every linked
//! module behind one link; `cli` drives a server from the shell.

mod cli;
mod server;

/// The module table build.rs wrote from `modules`.
mod link {
    use reovim_kernel_interface::Booted;
    include!(concat!(env!("OUT_DIR"), "/link.rs"));
}

use std::fmt;
use std::process::ExitCode;

/// Why the binary stops, printed once on stderr.
pub enum Fail {
    Usage(String),
    Host(reovim_host::Error),
    Kernel(reovim_kernel_interface::Error),
    /// The other end sent something the wire does not describe.
    Wire,
}

impl fmt::Display for Fail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(text) => write!(f, "{text}"),
            Self::Host(error) => write!(f, "host: {error:?}"),
            Self::Kernel(error) => write!(f, "{error:?}"),
            Self::Wire => write!(f, "not a message"),
        }
    }
}

impl From<reovim_host::Error> for Fail {
    fn from(error: reovim_host::Error) -> Self {
        Self::Host(error)
    }
}

impl From<reovim_kernel_interface::Error> for Fail {
    fn from(error: reovim_kernel_interface::Error) -> Self {
        Self::Kernel(error)
    }
}

const USAGE: &str = "\
usage: reovim server [--socket PATH] [--root DIR]
       reovim cli [--socket PATH] [COMMAND...]
defaults: --socket <runtime dir>/reovim.sock, --root <working directory>";

/// `--socket` and `--root` taken off the front of `args`, defaults from
/// the host filled in.
struct Options {
    socket: String,
    root: String,
    rest: Vec<String>,
}

fn options(args: Vec<String>) -> Result<Options, Fail> {
    let mut socket = None;
    let mut root = None;
    let mut args = args.into_iter();
    let mut rest = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--socket" => socket = Some(args.next().ok_or_else(usage)?),
            "--root" => root = Some(args.next().ok_or_else(usage)?),
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
    Ok(Options { socket, root, rest })
}

fn usage() -> Fail {
    Fail::Usage(USAGE.to_string())
}

fn run(args: Vec<String>) -> Result<(), Fail> {
    let mut args = args.into_iter();
    let command = args.next().ok_or_else(usage)?;
    let options = options(args.collect())?;
    match command.as_str() {
        "server" if options.rest.is_empty() => {
            server::run(&options.socket, &options.root)
        }
        "cli" => cli::run(&options.socket, &options.rest),
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
