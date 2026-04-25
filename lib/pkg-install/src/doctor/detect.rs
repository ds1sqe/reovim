//! Per-class fault detection plus the public [`audit`] composer.

use std::path::Path;

use crate::{
    artifact::DiscoveredArtifact,
    error::InstallError,
    hash::digest,
    inventory::{InstalledPackage, read_inventory},
    probe::probe,
};

use super::{
    report::{DoctorReport, DriftFinding, MissingFinding, UnloadableFinding},
    scan::scan_orphans,
};

/// Audit `library_root` against `lockfile`'s inventory.
///
/// Returns one [`DoctorReport`] capturing every fault class.
/// Findings inside each vector are sorted by package name (or path,
/// for orphans) so output is deterministic across runs.
///
/// # Errors
///
/// Returns an [`InstallError`] when the lockfile cannot be parsed or
/// when an inventoried cdylib's bytes cannot be hashed (I/O failure
/// other than file-not-found, which is a finding rather than an error).
pub fn audit(library_root: &Path, lockfile: &Path) -> Result<DoctorReport, InstallError> {
    let inventory = if lockfile.is_file() {
        read_inventory(lockfile, library_root)?
    } else {
        Vec::new()
    };
    let missing = detect_missing(&inventory);
    let drift = detect_drift(&inventory)?;
    let unloadable = detect_unloadable(&inventory);
    let orphan = scan_orphans(library_root, &inventory);
    let mut report = DoctorReport {
        unloadable,
        orphan,
        missing,
        drift,
    };
    report.unloadable.sort_by(|a, b| a.name.cmp(&b.name));
    report.missing.sort_by(|a, b| a.name.cmp(&b.name));
    report.drift.sort_by(|a, b| a.name.cmp(&b.name));
    report.orphan.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(report)
}

fn detect_missing(inventory: &[InstalledPackage]) -> Vec<MissingFinding> {
    inventory
        .iter()
        .filter(|p| !p.installed_path.is_file())
        .map(|p| MissingFinding {
            name: p.name.clone(),
            path: p.installed_path.clone(),
        })
        .collect()
}

fn detect_drift(inventory: &[InstalledPackage]) -> Result<Vec<DriftFinding>, InstallError> {
    let mut findings = Vec::new();
    for pkg in inventory {
        if !pkg.installed_path.is_file() {
            continue;
        }
        let actual = digest(&pkg.installed_path)?;
        if actual != pkg.sha256 {
            findings.push(DriftFinding {
                name: pkg.name.clone(),
                path: pkg.installed_path.clone(),
                expected: pkg.sha256.clone(),
                actual,
            });
        }
    }
    Ok(findings)
}

fn detect_unloadable(inventory: &[InstalledPackage]) -> Vec<UnloadableFinding> {
    let mut findings = Vec::new();
    for pkg in inventory {
        if !pkg.installed_path.is_file() {
            continue;
        }
        let artifact = DiscoveredArtifact {
            abs_path: pkg.installed_path.clone(),
            filename: pkg
                .installed_path
                .file_name()
                .and_then(|s| s.to_str())
                .map(str::to_string)
                .unwrap_or_default(),
        };
        if let Err(InstallError::NotLoadable { at, reason }) = probe(&artifact) {
            findings.push(UnloadableFinding {
                name: pkg.name.clone(),
                path: at,
                reason,
            });
        }
    }
    findings
}

#[cfg(test)]
#[path = "detect_tests.rs"]
mod detect_tests;
