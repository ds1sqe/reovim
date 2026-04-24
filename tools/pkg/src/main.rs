//! `pkg` — reovim package manager CLI binary.
//!
//! Phase 0.D scaffold: the full subcommand surface is present; each
//! subcommand body prints a phase-specific "not yet implemented"
//! message to stderr and exits with code 2. Later phases replace the
//! stub body with real behavior without extending the CLI surface.
//!
//! Exit-code contract:
//! - `0` = success (help, `--version`, successful subcommand).
//! - `1` = user error (bad arguments, missing manifest, …).
//! - `2` = unimplemented feature (Phase 0 stub for Phases 1/2/4).

mod cli;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, Cmd};

fn main() -> ExitCode {
    let cli = Cli::parse();
    cli.cmd.as_ref().map_or(ExitCode::SUCCESS, run)
}

fn run(cmd: &Cmd) -> ExitCode {
    let (name, phase) = match cmd {
        Cmd::Install(_) => ("install", 2u8),
        Cmd::Remove(_) => ("remove", 2),
        Cmd::List => ("list", 2),
        Cmd::Lock => ("lock", 1),
        Cmd::Resolve => ("resolve", 1),
        Cmd::Doctor(_) => ("doctor", 4),
    };
    eprintln!("pkg {name} is not yet implemented (#771 Phase {phase})");
    ExitCode::from(2)
}
