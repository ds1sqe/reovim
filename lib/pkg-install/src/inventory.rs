//! Read and write the installed-package inventory as a `pkg.lock`.
//!
//! The inventory IS the lockfile — every resolved package that the
//! installer has placed on disk has a [`PackageLock`] entry whose
//! `sha256`, `target`, and `kind` fields are populated. The
//! `installed_path` on [`InstalledPackage`] is NOT serialized; it is
//! always derived from `(library_root, kind_subdir, cdylib_filename)`
//! at read time.

use std::path::{Path, PathBuf};

use {
    reovim_pkg_lockfile::{Lockfile, PackageLock, Source},
    reovim_pkg_manifest::PackageKind,
};

use crate::{artifact::cdylib_filename, error::InstallError, place::kind_subdir};

/// A package that has been materialized into the library root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledPackage {
    /// Package name.
    pub name: String,
    /// Exact resolved version.
    pub version: String,
    /// Original local-path source of the package.
    pub source: Source,
    /// Classification.
    pub kind: PackageKind,
    /// rustc target triple the installer ran on.
    pub target: String,
    /// SHA-256 of the installed cdylib bytes.
    pub sha256: String,
    /// Absolute path to the cdylib in the library root. Derived
    /// from `(library_root, kind, name)`; not serialized.
    pub installed_path: PathBuf,
}

const LOCKFILE_VERSION: u32 = 1;

/// Return every installed package in the lockfile, sorted by name.
///
/// # Errors
///
/// Returns [`InstallError::Lockfile`] if the lockfile is malformed
/// or [`InstallError::ArtifactReadFailed`] if the file itself can't
/// be read.
pub fn list(lockfile: &Path, library_root: &Path) -> Result<Vec<InstalledPackage>, InstallError> {
    if !lockfile.is_file() {
        return Ok(Vec::new());
    }
    read_inventory(lockfile, library_root)
}

pub fn read_inventory(
    lockfile: &Path,
    library_root: &Path,
) -> Result<Vec<InstalledPackage>, InstallError> {
    let src = std::fs::read_to_string(lockfile).map_err(|e| read_failed(lockfile, &e))?;
    let lock = Lockfile::from_toml_str(&src)?;
    let mut out: Vec<InstalledPackage> = Vec::new();
    for pkg in lock.packages {
        let kind = parse_kind(&pkg)?;
        let installed_path = library_root
            .join(kind_subdir(kind))
            .join(cdylib_filename(&pkg.name));
        out.push(InstalledPackage {
            name: pkg.name,
            version: pkg.version,
            source: pkg.source,
            kind,
            target: pkg.target.unwrap_or_default(),
            sha256: pkg.sha256.unwrap_or_default(),
            installed_path,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub fn write_inventory(lockfile: &Path, items: &[InstalledPackage]) -> Result<(), InstallError> {
    let packages: Vec<PackageLock> = items
        .iter()
        .map(|p| PackageLock {
            name: p.name.clone(),
            version: p.version.clone(),
            source: p.source.clone(),
            target: Some(p.target.clone()),
            kind: Some(p.kind.as_str().to_string()),
            sha256: Some(p.sha256.clone()),
            dependencies: Vec::new(),
        })
        .collect();
    let lock = Lockfile {
        version: LOCKFILE_VERSION,
        packages,
    };
    let body = lock.to_toml_string()?;
    std::fs::write(lockfile, body).map_err(|e| write_failed(lockfile, &e))
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn write_failed(at: &Path, e: &std::io::Error) -> InstallError {
    InstallError::WriteFailed {
        at: at.to_path_buf(),
        reason: e.to_string(),
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn read_failed(at: &Path, e: &std::io::Error) -> InstallError {
    InstallError::ArtifactReadFailed {
        at: at.to_path_buf(),
        reason: e.to_string(),
    }
}

pub fn remove_from_inventory(
    lockfile: &Path,
    library_root: &Path,
    name: &str,
) -> Result<InstalledPackage, InstallError> {
    let mut items = read_inventory(lockfile, library_root)?;
    let idx =
        items
            .iter()
            .position(|p| p.name == name)
            .ok_or_else(|| InstallError::NotInstalled {
                name: name.to_string(),
            })?;
    let removed = items.remove(idx);
    write_inventory(lockfile, &items)?;
    Ok(removed)
}

fn parse_kind(pkg: &PackageLock) -> Result<PackageKind, InstallError> {
    match pkg.kind.as_deref() {
        Some("driver") => Ok(PackageKind::Driver),
        Some("module") => Ok(PackageKind::Module),
        _ => Err(InstallError::MissingPackageKind {
            pkg: pkg.name.clone(),
        }),
    }
}

#[cfg(test)]
#[path = "inventory_tests.rs"]
mod inventory_tests;
