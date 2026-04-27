//! Load a single `pkg.toml` from a directory on disk.
//!
//! The resolver identifies packages by the canonicalized directory
//! they live in; two paths that differ textually but resolve to the
//! same inode are the same package. [`load`] returns both the canonical
//! directory and the parsed manifest so callers can key cycle and
//! duplicate detection on the canonical path.

use std::path::{Path, PathBuf};

use reovim_pkg_manifest::Manifest;

use crate::graph::ResolveError;

#[derive(Debug)]
pub struct LoadedManifest {
    pub canonical_path: PathBuf,
    pub manifest_path: PathBuf,
    pub manifest: Manifest,
}

pub fn load(dir: &Path) -> Result<LoadedManifest, ResolveError> {
    let canonical_path = dir
        .canonicalize()
        .map_err(|_| ResolveError::ManifestNotFound {
            at: dir.to_path_buf(),
        })?;
    let manifest_path = canonical_path.join("pkg.toml");
    if !manifest_path.is_file() {
        return Err(ResolveError::ManifestNotFound { at: manifest_path });
    }
    let source =
        std::fs::read_to_string(&manifest_path).map_err(|_| ResolveError::ManifestNotFound {
            at: manifest_path.clone(),
        })?;
    let manifest =
        Manifest::from_toml_str(&source).map_err(|source| ResolveError::ManifestParse {
            at: manifest_path.clone(),
            source,
        })?;
    Ok(LoadedManifest {
        canonical_path,
        manifest_path,
        manifest,
    })
}

#[cfg(test)]
#[path = "loader_tests.rs"]
mod loader_tests;
