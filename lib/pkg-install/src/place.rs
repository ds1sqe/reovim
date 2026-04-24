//! Copy a discovered cdylib into
//! `<library_root>/<kind-subdir>/<filename>`.
//!
//! The copy is atomic via the standard tempfile-rename idiom:
//! write into `<dst-dir>/.pkg-install-tmp-<random>`, then `rename`
//! over `<dst>`. The tempfile MUST live in the destination
//! directory so `rename(2)` stays on a single filesystem — a
//! tempfile in `/tmp/` would cross devices and fail with `EXDEV`
//! on many setups.

use std::path::{Path, PathBuf};

use {
    reovim_dylib_loader::Kind as LoaderKind, reovim_pkg_manifest::PackageKind, tempfile::Builder,
};

use crate::{artifact::DiscoveredArtifact, error::InstallError};

pub struct Placement {
    pub dst: PathBuf,
}

pub fn place(
    artifact: &DiscoveredArtifact,
    kind: PackageKind,
    library_root: &Path,
) -> Result<Placement, InstallError> {
    let dst_dir = library_root.join(kind_subdir(kind));
    std::fs::create_dir_all(&dst_dir).map_err(|e| write_failed(&dst_dir, &e))?;
    let dst = dst_dir.join(&artifact.filename);

    // tempfile adjacent to dst so the final rename stays on the
    // same filesystem; see module docs.
    let tmp = Builder::new()
        .prefix(".pkg-install-tmp-")
        .tempfile_in(&dst_dir)
        .map_err(|e| write_failed(&dst_dir, &e))?;

    std::fs::copy(&artifact.abs_path, tmp.path())
        .map_err(|e| read_failed(&artifact.abs_path, &e))?;
    tmp.persist(&dst)
        .map_err(|e| write_failed(&dst, &e.error))?;

    Ok(Placement { dst })
}

pub const fn kind_subdir(kind: PackageKind) -> &'static str {
    match kind {
        PackageKind::Driver => LoaderKind::Driver.subdir(),
        PackageKind::Module => LoaderKind::Module.subdir(),
    }
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

#[cfg(test)]
#[path = "place_tests.rs"]
mod place_tests;
