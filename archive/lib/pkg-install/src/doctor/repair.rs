//! `--fix` policy: orphan deletion + missing-entry drop. Unloadable
//! and drift findings are NOT auto-remediated; they require user
//! judgement (re-install or manual investigation).

use std::path::{Path, PathBuf};

use crate::{error::InstallError, inventory::remove_from_inventory};

use super::report::DoctorReport;

/// Outcome of a `--fix` run. The library never writes to stdout;
/// the CLI (or any other consumer) renders a per-fix log from these
/// vectors.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepairOutcome {
    /// Paths of orphan files actually deleted. Idempotent
    /// re-deletes (file already gone) are NOT included.
    pub removed_orphans: Vec<PathBuf>,
    /// Names of inventory entries actually dropped from the
    /// lockfile. Idempotent re-drops (entry already absent) are NOT
    /// included.
    pub dropped_missing: Vec<String>,
    /// Findings repair left for the user: unloadable, drift, plus
    /// any orphan or missing finding whose remediation failed.
    pub residual: DoctorReport,
}

impl RepairOutcome {
    /// True iff `residual` has no findings of any class.
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        self.residual.is_clean()
    }
}

/// Apply `--fix` policy to the audit report. The report itself is
/// consumed by reference and never mutated; the typed
/// [`RepairOutcome`] carries the residual state.
///
/// # Errors
///
/// Returns an [`InstallError`] when an orphan delete or inventory
/// rewrite fails. The unfixed remainder is preserved in
/// `outcome.residual` so callers can report partial state.
pub fn repair(
    report: &DoctorReport,
    library_root: &Path,
    lockfile: &Path,
) -> Result<RepairOutcome, InstallError> {
    let mut outcome = RepairOutcome {
        removed_orphans: Vec::new(),
        dropped_missing: Vec::new(),
        residual: DoctorReport {
            unloadable: report.unloadable.clone(),
            drift: report.drift.clone(),
            orphan: Vec::new(),
            missing: Vec::new(),
        },
    };

    for finding in &report.orphan {
        match std::fs::remove_file(&finding.path) {
            Ok(()) => outcome.removed_orphans.push(finding.path.clone()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                outcome.residual.orphan.push(finding.clone());
                return Err(InstallError::WriteFailed {
                    at: finding.path.clone(),
                    reason: e.to_string(),
                });
            }
        }
    }

    for finding in &report.missing {
        match remove_from_inventory(lockfile, library_root, &finding.name) {
            Ok(_removed) => outcome.dropped_missing.push(finding.name.clone()),
            Err(InstallError::NotInstalled { .. }) => {}
            Err(e) => {
                outcome.residual.missing.push(finding.clone());
                return Err(e);
            }
        }
    }

    Ok(outcome)
}

#[cfg(test)]
#[path = "repair_tests.rs"]
mod repair_tests;
