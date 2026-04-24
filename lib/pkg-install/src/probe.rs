//! Dlopen smoke test.
//!
//! Confirms that a discovered cdylib can be loaded by the OS's
//! dynamic linker. The handle is dropped immediately; vtable
//! validation is the Phase 4 doctor command's job.

use reovim_dylib_loader::{Library, LoaderError};

use crate::{artifact::DiscoveredArtifact, error::InstallError};

pub fn probe(artifact: &DiscoveredArtifact) -> Result<(), InstallError> {
    Library::open(&artifact.abs_path)
        .map(drop)
        .map_err(|e| not_loadable(&artifact.abs_path, e))
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn not_loadable(probed: &std::path::Path, err: LoaderError) -> InstallError {
    match err {
        LoaderError::LibraryOpen { path, source_text } => InstallError::NotLoadable {
            at: path,
            reason: source_text,
        },
        other @ LoaderError::SymbolNotFound { .. } => InstallError::NotLoadable {
            at: probed.to_path_buf(),
            reason: other.to_string(),
        },
    }
}

#[cfg(test)]
#[path = "probe_tests.rs"]
mod probe_tests;
