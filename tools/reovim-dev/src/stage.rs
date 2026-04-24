//! Symlink-based staging of cdylibs from `target/debug/` into
//! `target/reovim-dev/{driver,modules}/`.
//!
//! Discovery rule (crate-name prefix, applied to the basename stem):
//!
//! - `reovim-driver-*` → `target/reovim-dev/driver/`
//! - `reovim-module-*` / `reovim-client-module-*` → `target/reovim-dev/modules/`
//! - everything else → skip with a `warn!`.
//!
//! Idempotent: an existing symlink at the destination is unlinked
//! before a fresh symlink is created, so re-running `stage` after a
//! rebuild picks up the new `.so` without manual cleanup.
//!
//! Windows fallback: `std::os::windows::fs::symlink_file` requires
//! Developer Mode or an elevated shell. If it fails with
//! `PermissionDenied`, we copy instead and log a `warn!` pointing at
//! the Developer Mode doc.

use {
    reovim_dylib_loader::library_extension,
    serde::Serialize,
    std::{
        fs, io,
        path::{Path, PathBuf},
    },
};

/// Outcome of [`stage_all`]: how many cdylibs landed in the staging
/// tree and where that tree lives.
#[derive(Debug, Default)]
pub struct StageReport {
    count: usize,
    staging_root: PathBuf,
}

/// Which staging subdirectory a cdylib is routed to.
#[derive(Debug, Clone, Copy)]
enum Kind {
    Driver,
    Module,
}

impl Kind {
    const fn subdir(self) -> &'static str {
        match self {
            Self::Driver => "driver",
            Self::Module => "modules",
        }
    }
}

impl StageReport {
    /// Number of cdylibs staged in this pass.
    #[must_use]
    pub const fn staged_count(&self) -> usize {
        self.count
    }

    /// Root directory under which cdylibs were staged.
    #[must_use]
    pub fn staging_root(&self) -> &Path {
        &self.staging_root
    }
}

/// Stage every `reovim-driver-*` / `reovim-module-*` /
/// `reovim-client-module-*` cdylib under
/// `<workspace>/target/debug/` into
/// `<workspace>/target/reovim-dev/{driver,modules}/`.
///
/// # Errors
///
/// Returns an error if `target/debug/` does not exist or the staging
/// root cannot be created. Per-file symlink/copy failures do NOT
/// abort the pass; they are logged and the offending entry is
/// omitted from the report.
pub fn stage_all(workspace: &Path) -> io::Result<StageReport> {
    let target_debug = workspace.join("target").join("debug");
    if !target_debug.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("target/debug not found under {}", workspace.display()),
        ));
    }

    let staging_root = workspace.join("target").join("reovim-dev");
    fs::create_dir_all(staging_root.join("driver"))?;
    fs::create_dir_all(staging_root.join("modules"))?;

    let mut report = StageReport {
        count: 0,
        staging_root: staging_root.clone(),
    };

    let ext = library_extension();
    for entry in fs::read_dir(&target_debug)?.flatten() {
        let src = entry.path();
        let Some(stem) = src.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if src
            .extension()
            .and_then(|s| s.to_str())
            .is_none_or(|e| !e.eq_ignore_ascii_case(ext))
        {
            continue;
        }

        let Some(kind) = classify(stem) else {
            tracing::warn!(file = %src.display(), "skipping cdylib with unknown crate-name prefix");
            continue;
        };

        let Some(dest_name) = src.file_name() else {
            continue;
        };
        let dest = staging_root.join(kind.subdir()).join(dest_name);

        match place(&src, &dest) {
            Ok(()) => report.count += 1,
            Err(e) => {
                tracing::warn!(file = %src.display(), error = %e, "failed to stage");
            }
        }
    }

    Ok(report)
}

fn classify(stem: &str) -> Option<Kind> {
    let s = stem.strip_prefix("lib").unwrap_or(stem);
    if s.starts_with("reovim_driver_") || s.starts_with("reovim-driver-") {
        Some(Kind::Driver)
    } else if s.starts_with("reovim_module_")
        || s.starts_with("reovim-module-")
        || s.starts_with("reovim_client_module_")
        || s.starts_with("reovim-client-module-")
    {
        Some(Kind::Module)
    } else {
        None
    }
}

/// Symlink `src` at `dest`, falling back to copy on Windows
/// `PermissionDenied`. Idempotent: an existing `dest` is removed
/// before the new link is created.
fn place(src: &Path, dest: &Path) -> io::Result<()> {
    if dest.exists() || dest.symlink_metadata().is_ok() {
        fs::remove_file(dest)?;
    }

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(src, dest)
    }
    #[cfg(windows)]
    {
        match std::os::windows::fs::symlink_file(src, dest) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                // TODO(#769-O4): Windows symlinks require Developer
                // Mode; fall back to copy so the dev loop works
                // unattended. Document in the Phase 1.F ABI doc.
                tracing::warn!(
                    "symlink denied on Windows — falling back to copy \
                     (enable Developer Mode to speed up iteration)"
                );
                fs::copy(src, dest).map(|_| ())
            }
            Err(e) => Err(e),
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        fs::copy(src, dest).map(|_| ())
    }
}

/// Scan the staging tree and return a JSON-serializable report of
/// each cdylib's open status. Used by `cargo reovim-dev scan`.
#[derive(Debug, Serialize)]
pub struct ScanResult {
    /// Staging root that was scanned.
    pub staging_root: String,
    /// Per-cdylib results from the `driver/` subdirectory.
    pub driver: Vec<ScanEntry>,
    /// Per-cdylib results from the `modules/` subdirectory.
    pub modules: Vec<ScanEntry>,
}

/// One candidate cdylib in a [`ScanResult`].
#[derive(Debug, Serialize)]
pub struct ScanEntry {
    /// Absolute path to the candidate.
    pub path: String,
    /// Whether `Library::open` succeeded for this candidate.
    pub opened: bool,
    /// Structured error message if `opened` is false.
    pub error: Option<String>,
}

/// Scan the staging tree at `<workspace>/target/reovim-dev/` and
/// report each cdylib's open status as JSON-serializable data.
#[must_use]
pub fn scan_staging(workspace: &Path) -> ScanResult {
    let staging_root = workspace.join("target").join("reovim-dev");
    let driver = scan_subdir(&staging_root.join("driver"));
    let modules = scan_subdir(&staging_root.join("modules"));
    ScanResult {
        staging_root: staging_root.display().to_string(),
        driver,
        modules,
    }
}

fn scan_subdir(dir: &Path) -> Vec<ScanEntry> {
    reovim_dylib_loader::scan_paths(&[dir.to_path_buf()])
        .into_entries()
        .into_iter()
        .map(|entry| {
            let (opened, error) = match entry.outcome {
                Ok(_lib) => (true, None),
                Err(e) => (false, Some(e.to_string())),
            };
            ScanEntry {
                path: entry.path.display().to_string(),
                opened,
                error,
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "stage_tests.rs"]
mod stage_tests;
