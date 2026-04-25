//! Install, uninstall, and list the cdylibs produced by a reovim
//! package graph.
//!
//! Given a [`reovim_pkg_resolver::Resolved`] set and a
//! `$REOVIM_LIBRARY_ROOT`, [`install`] discovers each package's
//! cdylib in `<pkg-dir>/dist/`, runs a `dlopen` smoke test, hashes
//! the bytes for tamper detection, and copies the artifact into
//! the library-root subdir that matches its [`PackageKind`]. The
//! lockfile at the caller-supplied path is kept in sync as an
//! installed-package inventory; [`uninstall`] and [`list`] read and
//! update that same inventory.
//!
//! Phase 2 scope: the installer is a composition of four single-
//! purpose helpers (discovery, probe, hash, placement) plus the
//! inventory sync. Vtable-level validation of the loaded cdylib is
//! out of scope — the probe only confirms that the shared object
//! opens. The doctor command (Phase 4) owns deeper integrity checks.

#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(missing_docs)]

mod artifact;
mod doctor;
mod error;
mod hash;
mod inventory;
mod place;
mod probe;

use std::path::{Path, PathBuf};

pub use crate::{
    doctor::{
        DoctorReport, DriftFinding, MissingFinding, OrphanFinding, RepairOutcome,
        UnloadableFinding, audit, repair,
    },
    error::InstallError,
    inventory::{InstalledPackage, list},
};

use {reovim_pkg_lockfile::Source, reovim_pkg_manifest::Manifest, reovim_pkg_resolver::Resolved};

/// Install every package in `resolved` into `library_root`, syncing
/// the inventory at `lockfile`.
///
/// Returns the full installed-package list after the operation
/// completes (in sorted order).
///
/// # Errors
///
/// Returns [`InstallError`] on any discovery, probe, hash, copy, or
/// inventory-write failure. See the enum variants for the full set.
pub fn install(
    resolved: &Resolved,
    library_root: &Path,
    lockfile: &Path,
) -> Result<Vec<InstalledPackage>, InstallError> {
    let existing: Vec<InstalledPackage> = if lockfile.is_file() {
        crate::inventory::read_inventory(lockfile, library_root)?
    } else {
        Vec::new()
    };

    let mut installed: Vec<InstalledPackage> = Vec::new();
    let target_triple = current_target();

    for pkg in &resolved.packages {
        let pkg_dir = local_source(&pkg.source, &pkg.name)?;

        let manifest = load_manifest(&pkg_dir)?;
        let kind = manifest
            .package
            .kind
            .ok_or_else(|| InstallError::MissingPackageKind {
                pkg: pkg.name.clone(),
            })?;

        let artifact = crate::artifact::discover(&pkg_dir, &pkg.name)?;
        crate::probe::probe(&artifact)?;
        let sha = crate::hash::digest(&artifact.abs_path)?;

        if let Some(prev) = existing
            .iter()
            .find(|p| p.name == pkg.name && p.version == pkg.version.to_string())
            && prev.sha256 != sha
        {
            return Err(InstallError::TamperedArtifact {
                pkg: pkg.name.clone(),
                expected: prev.sha256.clone(),
                actual: sha,
            });
        }

        let placement = crate::place::place(&artifact, kind, library_root)?;

        let trigger = resolved
            .lazy
            .iter()
            .find_map(|(name, t)| (name == &pkg.name).then(|| t.clone()));

        installed.push(InstalledPackage {
            name: pkg.name.clone(),
            version: pkg.version.to_string(),
            source: pkg.source.clone(),
            kind,
            target: target_triple.clone(),
            sha256: sha,
            trigger,
            installed_path: placement.dst,
        });
    }

    installed.sort_by(|a, b| a.name.cmp(&b.name));
    crate::inventory::write_inventory(lockfile, &installed)?;
    Ok(installed)
}

/// Remove the named package from disk and from the lockfile
/// inventory. Returns the removed record for caller inspection.
///
/// # Errors
///
/// Returns [`InstallError::NotInstalled`] if the package is not in
/// the inventory. Deleting a cdylib that is already gone is not an
/// error — only inventory-write failures propagate.
pub fn uninstall(
    name: &str,
    library_root: &Path,
    lockfile: &Path,
) -> Result<InstalledPackage, InstallError> {
    let removed = crate::inventory::remove_from_inventory(lockfile, library_root, name)?;
    best_effort_remove(&removed.installed_path)?;
    Ok(removed)
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn best_effort_remove(path: &Path) -> Result<(), InstallError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(InstallError::WriteFailed {
            at: path.to_path_buf(),
            reason: e.to_string(),
        }),
    }
}

fn load_manifest(dir: &Path) -> Result<Manifest, InstallError> {
    let path = dir.join("pkg.toml");
    let src = std::fs::read_to_string(&path).map_err(|e| read_failed(&path, &e))?;
    Manifest::from_toml_str(&src).map_err(|source| InstallError::Manifest { at: path, source })
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn read_failed(at: &Path, e: &std::io::Error) -> InstallError {
    InstallError::ArtifactReadFailed {
        at: at.to_path_buf(),
        reason: e.to_string(),
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn local_source(source: &Source, pkg_name: &str) -> Result<PathBuf, InstallError> {
    match source {
        Source::LocalPath(p) => Ok(p.clone()),
        Source::Registry { .. } => Err(InstallError::UnsupportedSource {
            pkg: pkg_name.to_string(),
        }),
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn current_target() -> String {
    format!("{}-{}-{}", std::env::consts::ARCH, target_vendor(), target_os_and_env())
}

#[cfg_attr(coverage_nightly, coverage(off))]
const fn target_vendor() -> &'static str {
    if cfg!(target_os = "macos") || cfg!(target_os = "ios") {
        "apple"
    } else if cfg!(target_env = "msvc") {
        "pc"
    } else {
        "unknown"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn target_os_and_env() -> String {
    let os = std::env::consts::OS;
    let env = std::env::consts::FAMILY;
    if env == "unix" {
        format!("{os}-gnu")
    } else if env == "windows" {
        format!("{os}-msvc")
    } else {
        os.to_string()
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod lib_tests;
