//! Eager-only wrapper around [`reovim_dylib_loader::scan_paths`].
//!
//! [`scan_eager`] performs the same filesystem scan as `scan_paths`
//! and then drops every [`ScanEntry`] whose filename maps to a package
//! the registry classifies as lazy. Filenames that do not match the
//! reovim cdylib convention (`libreovim_pkg_<snake>.<ext>` or
//! `reovim_pkg_<snake>.dll`) are passed through unchanged — they are
//! not ours to gate.
//!
//! The signature returns `Vec<ScanEntry>` rather than `ScanReport`:
//! `ScanReport` exposes no public constructor and Phase 3 may not
//! modify `reovim-dylib-loader`, so callers consume the entry vector
//! directly.

use std::path::Path;

use reovim_dylib_loader::{Kind, ScanEntry, pkg_name_from_cdylib_filename, scan_paths};

use crate::registry::LazyRegistry;

/// Scan the directory `library_root/<kind.subdir()>/` for cdylibs and
/// retain only those that should be loaded eagerly per `registry`.
#[must_use]
pub fn scan_eager(library_root: &Path, kind: Kind, registry: &LazyRegistry) -> Vec<ScanEntry> {
    let dir = library_root.join(kind.subdir());
    scan_paths(&[dir])
        .into_entries()
        .into_iter()
        .filter(|entry| !is_lazy_entry(entry, registry))
        .collect()
}

fn is_lazy_entry(entry: &ScanEntry, registry: &LazyRegistry) -> bool {
    entry
        .path
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .and_then(pkg_name_from_cdylib_filename)
        .is_some_and(|name| registry.is_lazy(&name))
}

#[cfg(test)]
#[path = "filter_tests.rs"]
mod filter_tests;
