//! Parallel cdylib discovery across a list of directories.
//!
//! [`scan_paths`] walks each input directory, filters entries by the
//! per-OS cdylib extension from [`crate::library_extension`], and
//! attempts to `dlopen` each candidate via [`crate::Library::open`] —
//! in parallel, through `rayon`. The function is infallible at the API
//! level: a missing directory, an unreadable file, or a malformed
//! cdylib becomes a structured [`ScanEntryError`] attached to the
//! offending path inside the returned [`ScanReport`]. One broken
//! cdylib never prevents the loader from surfacing the rest.

use {
    crate::{Library, error::ScanEntryError, platform::library_extension},
    rayon::prelude::*,
    std::{
        fs,
        path::{Path, PathBuf},
    },
};

/// Per-candidate outcome produced by [`scan_paths`].
///
/// Carries the source path on both the success and error sides so
/// callers can log, dedupe, or attribute failures without re-plumbing
/// context. Consumed by downstream `from_path_scan` constructors in
/// the specialized subsys loader crates.
#[derive(Debug)]
pub struct ScanEntry {
    /// Absolute or caller-relative path to the candidate cdylib.
    pub path: PathBuf,
    /// Open library, or a structured failure.
    pub outcome: Result<Library, ScanEntryError>,
}

/// Aggregated result of [`scan_paths`]. Holds one [`ScanEntry`] per
/// candidate cdylib found across all input directories.
#[derive(Debug)]
pub struct ScanReport {
    entries: Vec<ScanEntry>,
}

impl ScanReport {
    /// All per-candidate entries, in discovery order.
    #[must_use]
    pub fn entries(&self) -> &[ScanEntry] {
        &self.entries
    }

    /// Consume the report into its `Vec<ScanEntry>`. Used by
    /// `from_path_scan` callers that map each entry through further
    /// per-driver or per-module validation.
    #[must_use]
    pub fn into_entries(self) -> Vec<ScanEntry> {
        self.entries
    }

    /// Number of candidates, successful or failed.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when no candidates were found at all (every input
    /// directory missing, empty, or filtered out).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Scan a list of directories in parallel, producing a [`ScanReport`].
///
/// Per-directory and per-candidate behavior:
///
/// - A non-existent directory emits a `warn!` and contributes no
///   entries to the report.
/// - A non-directory path (file, symlink to file) emits a `warn!` and
///   contributes nothing.
/// - Entries inside each directory are filtered by the per-OS cdylib
///   extension; non-matching entries are silently skipped.
/// - Matching entries are opened in parallel; successes and failures
///   both land in the report as [`ScanEntry`] values.
///
/// Duplicate paths across input directories are NOT deduplicated at
/// this layer; the caller's [`crate::PathResolver`] has already
/// dedup'd the directory list, and physical-identity dedup (two
/// symlinks pointing at the same inode) is policy, not mechanism.
#[must_use]
pub fn scan_paths(paths: &[PathBuf]) -> ScanReport {
    let candidates = collect_candidates(paths);
    let entries: Vec<ScanEntry> = candidates
        .into_par_iter()
        .map(|path| {
            let outcome = Library::open(&path).map_err(|e| {
                let text = match e {
                    crate::LoaderError::LibraryOpen { source_text, .. }
                    | crate::LoaderError::SymbolNotFound { source_text, .. } => source_text,
                };
                classify_open_error(&path, text)
            });
            ScanEntry { path, outcome }
        })
        .collect();
    ScanReport { entries }
}

fn collect_candidates(paths: &[PathBuf]) -> Vec<PathBuf> {
    let ext = library_extension();
    let mut out = Vec::new();
    for dir in paths {
        match fs::read_dir(dir) {
            Ok(rd) => {
                for entry in rd.flatten() {
                    let candidate = entry.path();
                    if candidate
                        .extension()
                        .and_then(|s| s.to_str())
                        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
                    {
                        out.push(candidate);
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    path = %dir.display(),
                    error = %e,
                    "scan_paths: skipping inaccessible directory"
                );
            }
        }
    }
    out
}

/// Bucket a `Library::open` failure into the correct
/// [`ScanEntryError`] variant based on the upstream error text.
///
/// Takes the rendered error string directly instead of a
/// [`crate::LoaderError`]: `scan_paths` only ever sees
/// `LibraryOpen`-shaped failures (it never calls `Library::symbol`),
/// so matching on the enum variant added a dead arm the tests had to
/// synthesize. The string is the common-denominator surface.
///
/// `libloading` does not expose a typed error enum we can match on
/// across platforms, so classification reads the rendered text — the
/// same technique the reovim driver-loader stack uses at the vtable
/// layer.
///
/// Branch coverage:
/// - Permission-denied text → [`ScanEntryError::Io`]. Exercised by
///   the chmod-0 fixture in `tests/scan_nonfatal.rs`.
/// - "not a" / "file too short" text → [`ScanEntryError::NotALibrary`].
///   Exercised by the 4-byte garbage fixture.
/// - Everything else → [`ScanEntryError::DlopenFailed`]. See the
///   coverage note below.
fn classify_open_error(path: &Path, source_text: String) -> ScanEntryError {
    let lower = source_text.to_ascii_lowercase();

    if lower.contains("permission denied") {
        return ScanEntryError::Io {
            path: path.to_path_buf(),
            source: std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        };
    }

    // "not a" covers glibc / musl / macOS dyld "not a shared object /
    // dynamic library" text and Windows "not a valid win32
    // application". "file too short" covers truncated ELF files. The
    // Phase 1.F CI matrix validates these strings empirically on
    // macOS and Windows; if a runner surfaces a novel phrase we
    // extend this OR chain in that flight's landing.
    if lower.contains("not a") || lower.contains("file too short") {
        return ScanEntryError::NotALibrary {
            path: path.to_path_buf(),
            source_text,
        };
    }

    dlopen_failed_fallback(path, source_text)
}

/// Fallback arm of [`classify_open_error`]. The call site at the
/// bottom of `classify_open_error` routes here when the upstream
/// error text matches neither the permission-denied nor the
/// format-failure patterns — in practice, a cdylib that dlopens past
/// format validation but fails at symbol-resolution time (for
/// example, a missing `DT_NEEDED` dep).
///
/// Exercised by the `classify_open_error_routes_unrecognized_text_to_dlopen_failed`
/// unit test via a synthesized `LoaderError`; a real-world cdylib
/// fixture that triggers this path is heavier than the four Phase 0
/// `PoC` crates and is deferred to the Phase 1.F CI matrix.
fn dlopen_failed_fallback(path: &Path, source_text: String) -> ScanEntryError {
    ScanEntryError::DlopenFailed {
        path: path.to_path_buf(),
        source_text,
    }
}

#[cfg(test)]
#[path = "scan_tests.rs"]
mod scan_tests;
