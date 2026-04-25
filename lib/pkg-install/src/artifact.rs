//! Locate the pre-built cdylib for a package inside
//! `<pkg-dir>/dist/`.
//!
//! The filename convention lives on
//! [`reovim_dylib_loader::cdylib_filename`] — platform-specific
//! `.so` / `.dll` / `.dylib` with the `reovim_pkg_<snake_name>` cargo
//! stem.

use std::path::{Path, PathBuf};

use reovim_dylib_loader::cdylib_filename;

use crate::error::InstallError;

#[derive(Debug)]
pub struct DiscoveredArtifact {
    pub abs_path: PathBuf,
    pub filename: String,
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
