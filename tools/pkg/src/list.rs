//! `pkg list`: print the installed-package inventory.

use {
    anyhow::{Context, Result},
    reovim_pkg_install::list,
};

use crate::cli::ListArgs;

pub fn run(args: &ListArgs) -> Result<()> {
    let installed = list(&args.lockfile, &args.library_root)
        .with_context(|| format!("failed to read `{}`", args.lockfile.display()))?;
    for pkg in installed {
        let sha_prefix = pkg.sha256.chars().take(8).collect::<String>();
        println!(
            "{} {} {} {} {}",
            pkg.name,
            pkg.version,
            pkg.kind.as_str(),
            sha_prefix,
            pkg.installed_path.display(),
        );
    }
    Ok(())
}
