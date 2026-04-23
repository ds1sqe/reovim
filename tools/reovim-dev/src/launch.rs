//! `cargo reovim-dev run` — execute a reovim bin with
//! `REOVIM_LIBRARY_ROOT` preset to the staging root so the subsys
//! loaders discover staged cdylibs without an install step.

use std::{
    io,
    path::Path,
    process::{Command, ExitStatus},
};

/// Execute `<bin>` with `REOVIM_LIBRARY_ROOT` set to
/// `<workspace>/target/reovim-dev/`, forwarding the remaining argv.
/// Inherits stdio so interactive bins (TUI) work correctly.
///
/// # Errors
///
/// Returns the underlying `io::Error` if the bin cannot be spawned,
/// or a formatted message if the child exited with a non-zero status.
pub fn exec(workspace: &Path, bin: &str, args: &[String]) -> Result<(), String> {
    let staging_root = workspace.join("target").join("reovim-dev");
    let status: ExitStatus = Command::new(bin)
        .args(args)
        .env("REOVIM_LIBRARY_ROOT", &staging_root)
        .status()
        .map_err(|e: io::Error| format!("failed to spawn {bin}: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("{bin} exited with {status}"))
    }
}
