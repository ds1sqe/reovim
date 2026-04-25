//! `pkg` — reovim package manager CLI binary.
//!
//! Phase 0 scaffolded the subcommand surface; Phase 1 wires `lock`
//! and `resolve` to the resolver. The remaining subcommands
//! (install, remove, list, doctor) still print a phase-specific
//! "not yet implemented" message to stderr and exit with code 2.
//!
//! Exit-code contract:
//! - `0` = success (help, `--version`, successful subcommand).
//! - `1` = user error (bad manifest path, unresolvable deps, conflict).
//! - `2` = unimplemented feature (Phases 2 / 4 stubs).

mod cli;
mod install;
mod list;
mod lock;
mod remove;
mod resolve;
mod runtime;
mod trigger;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, Cmd};

fn main() -> ExitCode {
    let Some(cmd) = Cli::parse().cmd else {
        return ExitCode::SUCCESS;
    };
    match cmd {
        Cmd::Lock(args) => dispatch_result(lock::run(&args)),
        Cmd::Resolve(args) => dispatch_result(resolve::run(&args)),
        Cmd::Install(args) => dispatch_result(install::run(&args)),
        Cmd::Remove(args) => dispatch_result(remove::run(&args)),
        Cmd::List(args) => dispatch_result(list::run(&args)),
        Cmd::Doctor(_) => unimplemented_stub("doctor", 4),
        Cmd::Trigger(args) => dispatch_result(trigger::run(&args)),
    }
}

fn dispatch_result(result: anyhow::Result<()>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err:#}");
            ExitCode::from(1)
        }
    }
}

fn unimplemented_stub(name: &str, phase: u8) -> ExitCode {
    eprintln!("pkg {name} is not yet implemented (#771 Phase {phase})");
    ExitCode::from(2)
}
