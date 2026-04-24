//! Convert [`Resolved`] into a [`reovim_pkg_lockfile::Lockfile`].
//!
//! Emits `sha256 = None` and `target = None` on every package: the
//! resolver knows neither the cdylib artifact digest nor the rustc
//! target triple — those come from the installer that materializes
//! artifacts.

use reovim_pkg_lockfile::{Lockfile, PackageLock};

use crate::graph::Resolved;

const LOCKFILE_VERSION: u32 = 1;

impl Resolved {
    /// Materialize the resolved set into a lockfile value.
    #[must_use]
    pub fn into_lockfile(self) -> Lockfile {
        let packages = self
            .packages
            .into_iter()
            .map(|p| PackageLock {
                name: p.name,
                version: p.version.to_string(),
                source: p.source,
                target: None,
                kind: None,
                sha256: None,
                dependencies: p.dependencies,
            })
            .collect();
        Lockfile {
            version: LOCKFILE_VERSION,
            packages,
        }
    }
}

#[cfg(test)]
#[path = "emit_tests.rs"]
mod emit_tests;
