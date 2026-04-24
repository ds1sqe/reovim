//! `pkg` CLI argument schema.
//!
//! The [`Cmd`] enum is **complete in Phase 0**: every subcommand the
//! package manager will ever expose is declared here with an
//! "unimplemented" stub body in [`main`](crate::main). Later phases
//! replace each stub with real behavior, but they do not extend the
//! CLI surface. That keeps `--help` output stable across phases.

#![allow(clippy::module_name_repetitions)]

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
    Lock(LockArgs),
    /// Resolve the manifest without writing a lockfile (dry run).
    Resolve(ResolveArgs),
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

/// Arguments to `pkg lock`.
#[derive(Parser, Debug)]
pub struct LockArgs {
    /// Directory containing `pkg.toml`. Defaults to the current
    /// working directory.
    #[arg(long, default_value = ".")]
    pub manifest_dir: PathBuf,

    /// Output path for `pkg.lock`. Defaults to
    /// `<manifest_dir>/pkg.lock`.
    #[arg(long)]
    pub lockfile: Option<PathBuf>,
}

/// Arguments to `pkg resolve`.
#[derive(Parser, Debug)]
pub struct ResolveArgs {
    /// Directory containing `pkg.toml`. Defaults to the current
    /// working directory.
    #[arg(long, default_value = ".")]
    pub manifest_dir: PathBuf,
}
