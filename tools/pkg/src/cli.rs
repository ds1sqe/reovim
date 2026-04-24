//! `pkg` CLI argument schema.
//!
//! The [`Cmd`] enum is **complete in Phase 0**: every subcommand the
//! package manager will ever expose is declared here with an
//! "unimplemented" stub body in [`main`](crate::main). Later phases
//! replace each stub with real behavior, but they do not extend the
//! CLI surface. That keeps `--help` output stable across phases.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// Top-level `pkg` CLI.
#[derive(Parser, Debug)]
#[command(name = "pkg", version, about = "Reovim package manager")]
pub struct Cli {
    /// Subcommand to dispatch. `None` prints help and exits 0.
    #[command(subcommand)]
    pub cmd: Option<Cmd>,
}

/// `pkg` subcommand set.
#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Install a package into the reovim library root.
    ///
    /// Phase 2 implements the real behavior.
    Install(InstallArgs),
    /// Remove an installed package.
    ///
    /// Phase 2 implements the real behavior.
    Remove(RemoveArgs),
    /// List installed packages with versions and paths.
    ///
    /// Phase 2 implements the real behavior.
    List,
    /// Resolve the manifest and write `pkg.lock`.
    ///
    /// Phase 1 implements the real behavior.
    Lock,
    /// Resolve the manifest without writing a lockfile (dry run).
    ///
    /// Phase 1 implements the real behavior.
    Resolve,
    /// Audit the reovim library root.
    ///
    /// Phase 4 implements the real behavior.
    Doctor(DoctorArgs),
}

/// Arguments to `pkg install`.
#[derive(Parser, Debug)]
pub struct InstallArgs {
    /// Path to a cdylib file or a package manifest directory.
    pub target: Option<PathBuf>,
}

/// Arguments to `pkg remove`.
#[derive(Parser, Debug)]
pub struct RemoveArgs {
    /// Package name to remove.
    pub name: String,
}

/// Arguments to `pkg doctor`.
#[derive(Parser, Debug)]
pub struct DoctorArgs {
    /// Attempt to autoremediate safely-fixable issues.
    #[arg(long)]
    pub fix: bool,
}
