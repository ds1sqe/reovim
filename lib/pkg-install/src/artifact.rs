//! Locate the pre-built cdylib for a package inside
//! `<pkg-dir>/dist/`.
//!
//! The filename convention is `libreovim_pkg_<snake_name>.<ext>`
//! where `<ext>` is computed by
//! [`reovim_dylib_loader::library_filename`] (platform-specific:
//! `.so` / `.dll` / `.dylib`) and the `_` form is produced by
//! replacing hyphens in the package name with underscores (cargo's
//! cdylib convention).

use std::path::{Path, PathBuf};

use reovim_dylib_loader::library_filename;

use crate::error::InstallError;

#[derive(Debug)]
pub struct DiscoveredArtifact {
    pub abs_path: PathBuf,
    pub filename: String,
}

/// Platform-specific cdylib filename for a package, e.g.
/// `libreovim_pkg_my_theme.so`. Public so the lockfile writer can
/// derive the installed path without re-implementing the convention.
#[must_use]
pub fn cdylib_filename(pkg_name: &str) -> String {
    let stem = format!("reovim_pkg_{}", pkg_name.replace('-', "_"));
    library_filename(&stem)
}

pub fn discover(pkg_dir: &Path, pkg_name: &str) -> Result<DiscoveredArtifact, InstallError> {
    let dist = pkg_dir.join("dist");
    if !dist.is_dir() {
        return Err(InstallError::MissingDistDir {
            pkg: pkg_name.to_string(),
            at: dist,
        });
    }
    let filename = cdylib_filename(pkg_name);
    let abs_path = dist.join(&filename);
    if !abs_path.is_file() {
        return Err(InstallError::MissingArtifact {
            pkg: pkg_name.to_string(),
            probed: abs_path,
        });
    }
    Ok(DiscoveredArtifact { abs_path, filename })
}

#[cfg(test)]
#[path = "artifact_tests.rs"]
mod artifact_tests;
