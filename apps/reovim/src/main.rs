#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! `reovim` subprocess launcher.
//!
//! Thin clap dispatcher: `reovim {server,tui,cli,module,web} …`
//! becomes a `Command::new("reovim-<kind>")` call that inherits stdio
//! and exits with the child's status. No `reovim-*` workspace deps;
//! composition happens through the OS. A future embedded-mode will
//! grow this crate into a dual-mode composition root and relax the
//! zero-deps constraint.

use std::process::{Command, ExitStatus};

use clap::{Parser, Subcommand};

/// Top-level CLI for the `reovim` launcher.
#[derive(Parser, Debug)]
#[command(name = "reovim")]
#[command(version, about = "reovim launcher (subprocess-only; embedded composition mode is not yet implemented)", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Subcommand, Debug)]
enum Cmd {
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
struct Dispatch<'a> {
    bin: &'static str,
    leading_args: &'a [&'static str],
    forwarded_args: &'a [String],
}

/// Pure argv-to-dispatch mapping, extracted for unit-testing without
/// actually spawning a child process.
fn dispatch_for(cmd: &Cmd) -> Dispatch<'_> {
    match cmd {
        Cmd::Server { args } => Dispatch { bin: "reovim-server", leading_args: &[], forwarded_args: args },
        Cmd::Tui { args } => Dispatch { bin: "reovim-tui", leading_args: &[], forwarded_args: args },
        Cmd::Cli { args } => Dispatch { bin: "reovim-cli", leading_args: &[], forwarded_args: args },
        Cmd::Module { args } => Dispatch { bin: "reovim-server", leading_args: &["module"], forwarded_args: args },
        Cmd::Web { args } => Dispatch { bin: "reovim-web", leading_args: &[], forwarded_args: args },
    }
}

/// Seam for unit tests: spawn a child and return its exit status.
trait CommandSpawn {
    fn spawn(&self, dispatch: &Dispatch<'_>) -> std::io::Result<ExitStatus>;
}

/// Default spawner — real `std::process::Command` with inherited stdio.
struct RealSpawner;

impl CommandSpawn for RealSpawner {
    fn spawn(&self, dispatch: &Dispatch<'_>) -> std::io::Result<ExitStatus> {
        Command::new(dispatch.bin)
            .args(dispatch.leading_args)
            .args(dispatch.forwarded_args)
            .status()
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn main() -> std::io::Result<()> {
    let cli = Cli::parse();
    let Some(cmd) = cli.command else {
        // No subcommand — print a friendly pointer and exit 0.
        eprintln!(
            "reovim: no subcommand given. Try `reovim --help` to see \
             `server`, `tui`, `cli`, `module`, or `web`. Embedded \
             composition mode is not yet implemented."
        );
        return Ok(());
    };
    let dispatch = dispatch_for(&cmd);
    let status = RealSpawner.spawn(&dispatch)?;
    if let Some(code) = status.code() {
        std::process::exit(code);
    }
    // Unix: terminated by signal — propagate a non-zero code.
    std::process::exit(1);
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
