//! `pkg lock`: resolve the manifest at `--manifest-dir` and write
//! a deterministic `pkg.lock`.

use std::{fs, path::PathBuf};

use {
    anyhow::{Context, Result},
    reovim_pkg_resolver::resolve,
    semver::Version,
};

use crate::cli::LockArgs;

pub fn run(args: &LockArgs) -> Result<()> {
    let runtime = runtime_version()?;
    let resolved = resolve(&args.manifest_dir, &runtime).with_context(|| {
        format!("failed to resolve manifest in `{}`", args.manifest_dir.display())
    })?;
    let lockfile = resolved.into_lockfile();
    let out = lockfile_path(args);
    let body = lockfile
        .to_toml_string()
        .context("failed to serialize pkg.lock")?;
    fs::write(&out, body).with_context(|| format!("failed to write `{}`", out.display()))?;
    println!("wrote {}", out.display());
    Ok(())
}

fn lockfile_path(args: &LockArgs) -> PathBuf {
    args.lockfile
        .clone()
        .unwrap_or_else(|| args.manifest_dir.join("pkg.lock"))
}

fn runtime_version() -> Result<Version> {
    crate::runtime::reovim_runtime_version()
}
