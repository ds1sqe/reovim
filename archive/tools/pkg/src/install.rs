//! `pkg install`: resolve the manifest at `--manifest-dir` and
//! materialize every resolved cdylib into `--library-root`, syncing
//! the lockfile at `--lockfile` (default `<manifest_dir>/pkg.lock`).

use std::path::PathBuf;

use {
    anyhow::{Context, Result},
    reovim_pkg_install::install,
    reovim_pkg_resolver::resolve,
};

use crate::cli::InstallArgs;

pub fn run(args: &InstallArgs) -> Result<()> {
    let runtime = crate::runtime::reovim_runtime_version()?;
    let resolved = resolve(&args.manifest_dir, &runtime).with_context(|| {
        format!("failed to resolve manifest in `{}`", args.manifest_dir.display())
    })?;
    let lockfile = lockfile_path(args);
    let installed = install(&resolved, &args.library_root, &lockfile).with_context(|| {
        format!("failed to install packages into `{}`", args.library_root.display())
    })?;
    for pkg in &installed {
        println!(
            "installed {} {} ({}) at {}",
            pkg.name,
            pkg.version,
            pkg.kind.as_str(),
            pkg.installed_path.display(),
        );
    }
    Ok(())
}

fn lockfile_path(args: &InstallArgs) -> PathBuf {
    args.lockfile
        .clone()
        .unwrap_or_else(|| args.manifest_dir.join("pkg.lock"))
}
