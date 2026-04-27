//! `pkg` — reovim package manager CLI binary.
//!
//! Exit-code contract:
//! - `0` = success (help, `--version`, successful subcommand,
//!   `pkg doctor` with no findings, `pkg doctor --fix` that
//!   resolved every fault).
//! - `1` = user error OR `pkg doctor` finished with residual findings.

mod cli;
mod doctor;
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
        Cmd::Doctor(args) => match doctor::run(&args) {
            Ok(doctor::DoctorExit::Clean) => ExitCode::SUCCESS,
            Ok(doctor::DoctorExit::Findings) => ExitCode::from(1),
            Err(err) => {
                eprintln!("{err:#}");
                ExitCode::from(1)
            }
        },
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
