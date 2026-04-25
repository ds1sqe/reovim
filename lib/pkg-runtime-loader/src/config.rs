//! [`RuntimeLoaderConfig`] — the typed bundle threaded into each
//! runtime loader's discovery path.

use std::{path::PathBuf, sync::Arc};

use {reovim_dylib_loader::Kind, reovim_pkg_lazyload::LazyRegistry};

use crate::{error::RuntimeLoaderError, registry_load::load_registry};

/// Runtime-loader inputs derived from `$REOVIM_LIBRARY_ROOT`.
///
/// Server- and client-side loaders both consume one of these instead
/// of reading the lockfile themselves; the registry I/O is centralized
/// here so every loader sees the same trigger view.
///
/// `kind` selects which subdirectory under `library_root` the loader
/// scans (`driver/` for client drivers, `modules/` for server / client
/// modules). [`scan_dir`](Self::scan_dir) returns the resolved path.
#[derive(Debug, Clone)]
pub struct RuntimeLoaderConfig {
    /// `$REOVIM_LIBRARY_ROOT`.
    pub library_root: PathBuf,
    /// Shared lazy-load registry built from `<library_root>/pkg.lock`.
    pub registry: Arc<LazyRegistry>,
    /// Which subdirectory to scan (`driver` vs `modules`).
    pub kind: Kind,
}

impl RuntimeLoaderConfig {
    /// Read `<library_root>/pkg.lock`, build a registry, and bundle.
    ///
    /// A missing lockfile is treated as "no lazy entries declared":
    /// the resulting registry is empty so every package falls through
    /// as eager, preserving Wave-2 dlopen-everything behavior.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeLoaderError`] when the lockfile exists but is
    /// unreadable, malformed, or contains an invalid trigger.
    pub fn load(library_root: impl Into<PathBuf>, kind: Kind) -> Result<Self, RuntimeLoaderError> {
        let library_root = library_root.into();
        let registry = load_registry(&library_root)?;
        Ok(Self {
            library_root,
            registry,
            kind,
        })
    }

    /// Build from an already-constructed registry.
    ///
    /// Used by tests, by composition roots that synthesize a registry
    /// from an in-memory lockfile, and by intentionally lockfile-free
    /// code paths.
    #[must_use]
    pub fn from_registry(
        library_root: impl Into<PathBuf>,
        kind: Kind,
        registry: Arc<LazyRegistry>,
    ) -> Self {
        Self {
            library_root: library_root.into(),
            registry,
            kind,
        }
    }

    /// Subdirectory under `library_root` the loader should scan for
    /// cdylibs of [`Self::kind`].
    #[must_use]
    pub fn scan_dir(&self) -> PathBuf {
        self.library_root.join(self.kind.subdir())
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod config_tests;
