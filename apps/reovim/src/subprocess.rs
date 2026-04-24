//! Subprocess passthrough: `reovim {server,tui,cli,module,web} …`
//! dispatches to the matching sibling bin via
//! `std::process::Command` and propagates the child's exit code.

use std::process::{Command, ExitStatus};

use clap::{Parser, Subcommand};

/// Top-level CLI for the `reovim` launcher.
#[derive(Parser, Debug)]
#[command(name = "reovim")]
#[command(version, about = "reovim launcher (subprocess-only today; in-process composition mode is tracked under #769)", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Cmd>,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Start the gRPC server runtime. Dispatched to `reovim-server`.
    Server {
        /// Arguments forwarded verbatim to `reovim-server`.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Start the terminal UI client. Dispatched to `reovim-tui`.
    Tui {
        /// Arguments forwarded verbatim to `reovim-tui`.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Run one-shot CLI commands. Dispatched to `reovim-cli`.
    Cli {
        /// Arguments forwarded verbatim to `reovim-cli`.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Manage third-party modules. Dispatched to
    /// `reovim-server module …` — module management is a server-side
    /// operation and shares the server bin's process.
    Module {
        /// Arguments forwarded verbatim to `reovim-server module …`.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Start the web UI. Scaffold today; the SSR runtime lands with
    /// the web bin's dedicated work.
    Web {
        /// Arguments forwarded verbatim to `reovim-web`.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

/// Target of one subprocess dispatch: the bin name plus any leading
/// positional arguments that go before the caller-supplied `args`.
/// `Module` alone needs a prefix (`reovim-server module …`).
#[derive(Debug, PartialEq, Eq)]
pub struct Dispatch<'a> {
    pub bin: &'static str,
    pub leading_args: &'a [&'static str],
    pub forwarded_args: &'a [String],
}

/// Pure argv-to-dispatch mapping, extracted for unit-testing without
/// actually spawning a child process.
#[must_use]
pub fn dispatch_for(cmd: &Cmd) -> Dispatch<'_> {
    match cmd {
        Cmd::Server { args } => Dispatch {
            bin: "reovim-server",
            leading_args: &[],
            forwarded_args: args,
        },
        Cmd::Tui { args } => Dispatch {
            bin: "reovim-tui",
            leading_args: &[],
            forwarded_args: args,
        },
        Cmd::Cli { args } => Dispatch {
            bin: "reovim-cli",
            leading_args: &[],
            forwarded_args: args,
        },
        Cmd::Module { args } => Dispatch {
            bin: "reovim-server",
            leading_args: &["module"],
            forwarded_args: args,
        },
        Cmd::Web { args } => Dispatch {
            bin: "reovim-web",
            leading_args: &[],
            forwarded_args: args,
        },
    }
}

/// Seam for unit tests: spawn a child and return its exit status.
pub trait CommandSpawn {
    /// Spawn the child described by `dispatch` and wait for it.
    ///
    /// # Errors
    ///
    /// Propagates any `std::io::Error` from the underlying spawn.
    fn spawn(&self, dispatch: &Dispatch<'_>) -> std::io::Result<ExitStatus>;
}

/// Default spawner — real `std::process::Command` with inherited stdio.
pub struct RealSpawner;

impl CommandSpawn for RealSpawner {
    fn spawn(&self, dispatch: &Dispatch<'_>) -> std::io::Result<ExitStatus> {
        Command::new(dispatch.bin)
            .args(dispatch.leading_args)
            .args(dispatch.forwarded_args)
            .status()
    }
}

/// Dispatches the top-level `Cli` to a sibling bin.
///
/// `None` subcommand prints a friendly pointer and exits 0. A
/// subcommand spawns the child via `RealSpawner` and propagates its
/// exit code through `std::process::exit`.
///
/// # Errors
///
/// Returns any `std::io::Error` raised by `RealSpawner::spawn`.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn run(cli: Cli) -> std::io::Result<()> {
    let Some(cmd) = cli.command else {
        eprintln!(
            "reovim: no subcommand given. Try `reovim --help` to see \
             `server`, `tui`, `cli`, `module`, or `web`."
        );
        return Ok(());
    };
    let dispatch = dispatch_for(&cmd);
    let status = RealSpawner.spawn(&dispatch)?;
    if let Some(code) = status.code() {
        std::process::exit(code);
    }
    std::process::exit(1);
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
