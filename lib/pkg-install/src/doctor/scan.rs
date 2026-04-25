//! Orphan scanner: walks `<library_root>/{driver,modules}/` and
//! reports cdylibs whose package name is not in the inventory.
//!
//! `scan_paths` `dlopen`s every candidate. Orphan detection discards
//! the load result, so each pass pays the dlopen cost without using
//! the outcome. Cost is bounded by `<library_root>` size; a pass on a
//! 50-cdylib install is dozens of dlopens running through rayon. A
//! hot-path consumer would prefer a load-free walker.

use std::{
    collections::BTreeSet,
    ffi::OsStr,
    path::{Path, PathBuf},
};

use reovim_dylib_loader::{Kind, pkg_name_from_cdylib_filename, scan_paths};

use crate::inventory::InstalledPackage;

use super::report::OrphanFinding;

pub(super) fn scan_orphans(
    library_root: &Path,
    inventory: &[InstalledPackage],
) -> Vec<OrphanFinding> {
    let installed_names: BTreeSet<&str> = inventory.iter().map(|p| p.name.as_str()).collect();
    let mut findings = Vec::new();
    for kind in [Kind::Driver, Kind::Module] {
        let dir: PathBuf = library_root.join(kind.subdir());
        let report = scan_paths(&[dir]);
        for entry in report.into_entries() {
            let filename = entry
                .path
                .file_name()
                .and_then(OsStr::to_str)
                .map(str::to_string);
            let pkg_name = filename.as_deref().and_then(pkg_name_from_cdylib_filename);
            let (Some(filename), Some(pkg_name)) = (filename, pkg_name) else {
                continue;
            };
            if !installed_names.contains(pkg_name.as_str()) {
                findings.push(OrphanFinding {
                    filename,
                    path: entry.path,
                });
            }
        }
    }
    findings.sort_by(|a, b| a.path.cmp(&b.path));
    findings
}

#[cfg(test)]
#[path = "scan_tests.rs"]
mod scan_tests;
