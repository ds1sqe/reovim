//! Audit report types: one finding vector per fault class plus the
//! [`DoctorReport`] envelope.

use std::path::PathBuf;

use reovim_dylib_loader::pkg_name_from_cdylib_filename;

/// A cdylib that exists on disk and is in the inventory but fails to
/// load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnloadableFinding {
    /// Package name as recorded in the inventory.
    pub name: String,
    /// On-disk path to the offending cdylib.
    pub path: PathBuf,
    /// Loader's error text.
    pub reason: String,
}

/// A cdylib that lives under `<library_root>/{driver,modules}/` but is
/// not declared in the inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrphanFinding {
    /// File name as it appears on disk.
    pub filename: String,
    /// Full path to the unowned file.
    pub path: PathBuf,
}

impl OrphanFinding {
    /// Decode the package name from `filename` using the reovim cdylib
    /// convention. Returns `None` for filenames that do not match the
    /// `lib?reovim_pkg_<snake>.<ext>` shape — those are foreign and not
    /// orphans-of-ours.
    #[must_use]
    pub fn package_name(&self) -> Option<String> {
        pkg_name_from_cdylib_filename(&self.filename)
    }
}

/// An inventory entry whose `installed_path` is no longer on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingFinding {
    /// Package name as recorded in the inventory.
    pub name: String,
    /// Path the inventory says the cdylib should live at.
    pub path: PathBuf,
}

/// An inventory entry whose on-disk SHA-256 does not match the
/// recorded digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriftFinding {
    /// Package name as recorded in the inventory.
    pub name: String,
    /// Path to the cdylib whose bytes disagree with the lockfile.
    pub path: PathBuf,
    /// Digest the inventory expected.
    pub expected: String,
    /// Digest computed from the bytes on disk.
    pub actual: String,
}

/// Aggregate audit findings, one vector per fault class.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DoctorReport {
    /// Cdylibs the loader rejected.
    pub unloadable: Vec<UnloadableFinding>,
    /// Files in the library root with no inventory entry.
    pub orphan: Vec<OrphanFinding>,
    /// Inventory entries with no on-disk file.
    pub missing: Vec<MissingFinding>,
    /// Inventory entries whose bytes drifted from the recorded digest.
    pub drift: Vec<DriftFinding>,
}

impl DoctorReport {
    /// True iff every finding vector is empty.
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        self.unloadable.is_empty()
            && self.orphan.is_empty()
            && self.missing.is_empty()
            && self.drift.is_empty()
    }

    /// Sum of every finding vector's length.
    #[must_use]
    pub const fn total_findings(&self) -> usize {
        self.unloadable.len() + self.orphan.len() + self.missing.len() + self.drift.len()
    }
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod report_tests;
