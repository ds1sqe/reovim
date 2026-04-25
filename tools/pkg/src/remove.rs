//! `pkg remove`: remove a named package from disk and from the
//! lockfile.

use {
    anyhow::{Context, Result},
    reovim_pkg_install::uninstall,
};

use crate::cli::RemoveArgs;

pub fn run(args: &RemoveArgs) -> Result<()> {
    let removed = uninstall(&args.name, &args.library_root, &args.lockfile)
        .with_context(|| format!("failed to remove `{}`", args.name))?;
    println!(
        "removed {} {} from {}",
        removed.name,
        removed.version,
        removed.installed_path.display(),
    );
    Ok(())
}
