//! `pkg` CLI argument schema.
//!
//! The [`Cmd`] enum is **complete in Phase 0**: every subcommand the
//! package manager will ever expose is declared here with an
//! "unimplemented" stub body in [`main`](crate::main). Later phases
//! replace each stub with real behavior, but they do not extend the
//! CLI surface. That keeps `--help` output stable across phases.

#![allow(clippy::module_name_repetitions)]

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

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
    Install(InstallArgs),
    /// Remove an installed package.
    Remove(RemoveArgs),
    /// List installed packages with versions and paths.
    List(ListArgs),
    /// Resolve the manifest and write `pkg.lock`.
    Lock(LockArgs),
    /// Resolve the manifest without writing a lockfile (dry run).
    Resolve(ResolveArgs),
    /// Audit the reovim library root.
    ///
    /// Phase 4 implements the real behavior.
    Doctor(DoctorArgs),
    /// Open the cdylibs gated on a runtime trigger event.
    Trigger(TriggerArgs),
}

/// Arguments to `pkg install`.
#[derive(Parser, Debug)]
pub struct InstallArgs {
    /// Directory containing `pkg.toml`. Defaults to the current
    /// working directory.
    #[arg(long, default_value = ".")]
    pub manifest_dir: PathBuf,

    /// Library root to materialize cdylibs into. Defaults to
    /// `$REOVIM_LIBRARY_ROOT`.
    #[arg(long, env = "REOVIM_LIBRARY_ROOT")]
    pub library_root: PathBuf,

    /// Output path for `pkg.lock`. Defaults to
    /// `<manifest_dir>/pkg.lock`.
    #[arg(long)]
    pub lockfile: Option<PathBuf>,
}

/// Arguments to `pkg remove`.
#[derive(Parser, Debug)]
pub struct RemoveArgs {
    /// Package name to remove.
    pub name: String,

    /// Library root to remove the cdylib from. Defaults to
    /// `$REOVIM_LIBRARY_ROOT`.
    #[arg(long, env = "REOVIM_LIBRARY_ROOT")]
    pub library_root: PathBuf,

    /// Lockfile to sync. Defaults to `./pkg.lock`.
    #[arg(long, default_value = "pkg.lock")]
    pub lockfile: PathBuf,
}

/// Arguments to `pkg list`.
#[derive(Parser, Debug)]
pub struct ListArgs {
    /// Library root to derive installed paths from. Defaults to
    /// `$REOVIM_LIBRARY_ROOT`.
    #[arg(long, env = "REOVIM_LIBRARY_ROOT")]
    pub library_root: PathBuf,

    /// Lockfile to read. Defaults to `./pkg.lock`.
    #[arg(long, default_value = "pkg.lock")]
    pub lockfile: PathBuf,
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

/// Trigger event flavor — must match a [`reovim_pkg_manifest::LazyTrigger`]
/// variant.
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum TriggerKind {
    /// `LazyTrigger::OnDomain`.
    Domain,
    /// `LazyTrigger::OnEvent`.
    Event,
    /// `LazyTrigger::OnCapability`.
    Capability,
}

/// Arguments to `pkg trigger`.
#[derive(Parser, Debug)]
pub struct TriggerArgs {
    /// Trigger flavor (domain, event, capability).
    pub kind: TriggerKind,

    /// Trigger name to fire.
    pub name: String,

    /// Library root the cdylibs were installed into. Defaults to
    /// `$REOVIM_LIBRARY_ROOT`.
    #[arg(long, env = "REOVIM_LIBRARY_ROOT")]
    pub library_root: PathBuf,

    /// Lockfile to read. Defaults to `./pkg.lock`.
    #[arg(long, default_value = "pkg.lock")]
    pub lockfile: PathBuf,
}
