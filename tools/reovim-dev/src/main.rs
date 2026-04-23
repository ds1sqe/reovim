//! `cargo reovim-dev` — contributor dev-loop for cdylib drivers and modules.
//!
//! Cargo subcommand that stages `target/debug/*.<ext>` into
//! `target/reovim-dev/{driver,modules}/` (by crate-name prefix) and
//! launches a reovim bin with `REOVIM_LIBRARY_ROOT` preset so the
//! subsys loaders discover the staged cdylibs without needing an
//! install step.
//!
//! Invocation shape: `cargo reovim-dev <sub>`, where cargo passes
//! `reovim-dev` as the first argument. We strip it when present.

#![forbid(missing_docs)]

use reovim_dev::{launch, stage};

use {
    clap::{Parser, Subcommand},
    std::{path::PathBuf, process::ExitCode},
};

#[derive(Parser, Debug)]
#[command(bin_name = "cargo reovim-dev", disable_help_subcommand = true)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Symlink cdylibs from `target/debug/` into `target/reovim-dev/`.
    Stage {
        /// Workspace root (directory containing `target/`). Defaults
        /// to the current working directory.
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
    /// Scan `target/reovim-dev/{driver,modules}/` and print a JSON
    /// report of each cdylib's open status. Debug helper.
    Scan {
        /// Workspace root (see `stage --workspace`).
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
    /// Execute a reovim bin with `REOVIM_LIBRARY_ROOT` preset to the
    /// staging root.
    Run {
        /// Workspace root (see `stage --workspace`).
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// Binary to execute. Defaults to `reovim`.
        #[arg(long)]
        bin: Option<String>,
        /// Arguments forwarded verbatim to the binary.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

fn main() -> ExitCode {
    tracing_subscriber::fmt::try_init().ok();
    let raw: Vec<String> = std::env::args().collect();
    let cli = parse_cli(raw);
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("cargo-reovim-dev: {e}");
            ExitCode::from(1)
        }
    }
}

/// Strip the cargo-injected `reovim-dev` subcommand name when this bin
/// is invoked as `cargo reovim-dev <sub>`; leave the args alone when
/// invoked directly (tests / debugging).
fn parse_cli(mut raw: Vec<String>) -> Cli {
    if raw.get(1).map(String::as_str) == Some("reovim-dev") {
        raw.remove(1);
    }
    Cli::parse_from(raw)
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Cmd::Stage { workspace } => {
            let ws = resolve_workspace(workspace)?;
            let report = stage::stage_all(&ws).map_err(|e| e.to_string())?;
            println!(
                "staged {} cdylib(s) into {}",
                report.staged_count(),
                report.staging_root().display()
            );
            Ok(())
        }
        Cmd::Scan { workspace } => {
            let ws = resolve_workspace(workspace)?;
            let scan = stage::scan_staging(&ws);
            println!(
                "{}",
                serde_json::to_string_pretty(&scan).map_err(|e| e.to_string())?
            );
            Ok(())
        }
        Cmd::Run { workspace, bin, args } => {
            let ws = resolve_workspace(workspace)?;
            launch::exec(&ws, bin.as_deref().unwrap_or("reovim"), &args)
        }
    }
}

fn resolve_workspace(explicit: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(p) = explicit {
        return Ok(p);
    }
    std::env::current_dir().map_err(|e| format!("cwd unavailable: {e}"))
}
