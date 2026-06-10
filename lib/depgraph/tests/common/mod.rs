//! Shared helpers for integration tests.
//!
//! # Duplication note
//!
//! `TempDir` duplicates the helper in `src/tests.rs`. Integration tests
//! cannot reach `src/tests.rs` (it is compiled only under `#[cfg(test)]`
//! for unit tests), so a local copy is the standard pattern. Both copies
//! share the same algorithm; changes must be applied to both.

use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

#[allow(dead_code)]
static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A temporary directory removed on `Drop`.
///
/// Path: `std::env::temp_dir()` + `"reovim-integ-"` + process-ID +
/// `-` + monotonic counter (unique within a process; PID scopes across
/// concurrent processes).
#[allow(dead_code)]
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// Creates and returns a new unique temporary directory.
    ///
    /// # Panics
    ///
    /// Panics when the directory cannot be created.
    #[must_use]
    #[allow(dead_code)]
    pub fn new() -> Self {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("reovim-integ-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&path).expect("TempDir::new: create_dir_all failed");
        Self { path }
    }

    /// Returns the path of the temporary directory.
    #[must_use]
    #[allow(dead_code)]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Writes `content` to `base.join(rel)`, creating parent directories as needed.
///
/// # Panics
///
/// Panics on any I/O failure.
#[allow(dead_code)]
pub fn write_file(base: &Path, rel: &str, content: &str) {
    let target = base.join(rel);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|e| panic!("write_file: create_dir_all `{}`: {e}", parent.display()));
    }
    std::fs::write(&target, content)
        .unwrap_or_else(|e| panic!("write_file: write `{}`: {e}", target.display()));
}

/// Locates the workspace root by ascending from `CARGO_MANIFEST_DIR` two
/// levels (since the crate lives at `lib/depgraph/`).
///
/// # Panics
///
/// Panics when the ancestor does not exist.
#[must_use]
#[allow(dead_code)]
pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("lib/depgraph has a workspace root two levels up")
        .to_path_buf()
}
