//! Read `<library_root>/pkg.lock` and build a [`LazyRegistry`].

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use {reovim_pkg_lazyload::LazyRegistry, reovim_pkg_lockfile::Lockfile};

use crate::error::RuntimeLoaderError;

/// Path the runtime expects the inventory lockfile to live at.
///
/// Single source of truth for the convention. `pkg-install`'s inventory
/// writer (`lib/pkg-install/src/inventory.rs`) writes to the same
/// `<library_root>/pkg.lock` location; this helper exists so the
/// runtime side cannot drift from the install side.
#[must_use]
pub fn lockfile_path(library_root: &Path) -> PathBuf {
    library_root.join("pkg.lock")
}

/// Read the lockfile and build a shared [`LazyRegistry`].
///
/// A missing lockfile is normal — for a host that has not run
/// `pkg install` yet — and yields an empty registry. Every package
/// then falls through `LazyRegistry::trigger_for` as
/// `LazyTrigger::Eager`, preserving Wave-2 "dlopen everything" behavior.
///
/// # Errors
///
/// - [`RuntimeLoaderError::LockfileReadFailed`] — the file exists but
///   the OS refused the read.
/// - [`RuntimeLoaderError::LockfileMalformed`] — the file parsed but
///   `Lockfile::from_toml_str` rejected it.
/// - [`RuntimeLoaderError::RegistryBuildFailed`] — the file parsed
///   but at least one package's `trigger` field was malformed.
pub fn load_registry(library_root: &Path) -> Result<Arc<LazyRegistry>, RuntimeLoaderError> {
    let path = lockfile_path(library_root);
    let raw = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            trace_missing_lockfile(&path);
            return Ok(Arc::new(LazyRegistry::empty()));
        }
        Err(err) => {
            return Err(RuntimeLoaderError::LockfileReadFailed {
                path,
                reason: err.to_string(),
            });
        }
    };
    let lockfile =
        Lockfile::from_toml_str(&raw).map_err(|source| RuntimeLoaderError::LockfileMalformed {
            path: path.clone(),
            source,
        })?;
    let registry = LazyRegistry::from_lockfile(&lockfile)
        .map_err(|source| RuntimeLoaderError::RegistryBuildFailed { path, source })?;
    Ok(Arc::new(registry))
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn trace_missing_lockfile(path: &Path) {
    tracing::debug!(
        lockfile = %path.display(),
        "no pkg.lock at library root; treating every package as eager"
    );
}

#[cfg(test)]
#[path = "registry_load_tests.rs"]
mod registry_load_tests;
