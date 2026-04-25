//! Recover a reovim package name from a cdylib filename.

use std::path::Path;

use reovim_dylib_loader::pkg_name_from_cdylib_filename;

/// Return the reovim package name encoded in `path`'s filename, when
/// it follows the `cdylib_filename` convention.
///
/// Returns `None` for paths whose filename does not match the
/// convention (foreign cdylib, intermediate build artifact, missing
/// filename, non-UTF-8 path) — the runtime side of the diagnostic
/// pipeline can then fall back to the opaque error variant instead
/// of fabricating a package name.
#[must_use]
pub fn package_name_for_path(path: &Path) -> Option<String> {
    pkg_name_from_cdylib_filename(path.file_name()?.to_str()?)
}

#[cfg(test)]
#[path = "package_lookup_tests.rs"]
mod package_lookup_tests;
