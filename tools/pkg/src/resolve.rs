//! `pkg resolve`: dry-run resolution. Reads the manifest at
//! `--manifest-dir` and prints one line per resolved package to
//! stdout.

use {
    anyhow::{Context, Result},
    reovim_pkg_lockfile::Source,
    reovim_pkg_resolver::resolve,
};

use crate::cli::ResolveArgs;

pub fn run(args: &ResolveArgs) -> Result<()> {
    let runtime = crate::runtime::reovim_runtime_version()?;
    let resolved = resolve(&args.manifest_dir, &runtime).with_context(|| {
        format!("failed to resolve manifest in `{}`", args.manifest_dir.display())
    })?;
    for pkg in resolved.packages {
        let src = match pkg.source {
            Source::LocalPath(p) => format!("local-path:{}", p.display()),
            Source::Registry { url } => format!("registry:{url}"),
        };
        println!("{} {} {}", pkg.name, pkg.version, src);
    }
    Ok(())
}
