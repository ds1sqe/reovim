//! [`LoaderHandle`] — shared, mutex-protected access to a single
//! [`ModuleLoader`] for callers that mutate it from a callback context
//! (lazy-load dispatchers, hot-reload watchers, …).
//!
//! The dispatchers fire from inside `CommandNameIndex::resolve*` and
//! `Session::set_domain_driver`, where the bootstrap-owned `&mut
//! ModuleLoader` is no longer reachable. They hold an
//! `Arc<LoaderHandle>` and call [`LoaderHandle::load_named`] to dlopen
//! a single cdylib by package name.

use std::{path::Path, sync::Arc};

use {
    parking_lot::Mutex,
    reovim_dylib_loader::{Kind, cdylib_filename},
    reovim_kernel::api::v1::{ModuleError, ModuleId},
};

use crate::loader::ModuleLoader;

/// Shared handle that lets a callback-tier component dlopen one cdylib
/// at a time on the underlying [`ModuleLoader`].
///
/// All [`load_named`](Self::load_named) calls are write-exclusive — a
/// `parking_lot::Mutex` is sufficient and a `RwLock` would add no
/// reader path.
#[derive(Clone)]
pub struct LoaderHandle {
    inner: Arc<Mutex<ModuleLoader>>,
}

impl LoaderHandle {
    /// Wrap an existing loader behind the shared handle.
    #[must_use]
    pub fn new(loader: ModuleLoader) -> Self {
        Self {
            inner: Arc::new(Mutex::new(loader)),
        }
    }

    /// Wrap an already-shared loader.
    #[must_use]
    pub const fn from_arc(inner: Arc<Mutex<ModuleLoader>>) -> Self {
        Self { inner }
    }

    /// dlopen the cdylib for `name` at
    /// `<root>/<kind.subdir()>/<cdylib_filename(name)>` and register
    /// it into the underlying loader.
    ///
    /// # Safety
    ///
    /// Forwards the
    /// [`ModuleLoader::load_dynamic`](crate::loader::ModuleLoader::load_dynamic)
    /// safety contract: the caller must guarantee the cdylib at the
    /// resolved path is ABI-compatible with the host kernel.
    ///
    /// # Errors
    ///
    /// Returns whatever `load_dynamic` returns — including
    /// [`ModuleError::LoadFailed`] for a missing cdylib or a
    /// duplicate-load attempt.
    #[allow(unsafe_code)]
    pub unsafe fn load_named(
        &self,
        name: &str,
        kind: Kind,
        root: &Path,
    ) -> Result<ModuleId, ModuleError> {
        let path = root.join(kind.subdir()).join(cdylib_filename(name));
        // SAFETY: forwarded from caller of `load_named`.
        unsafe { self.inner.lock().load_dynamic(&path) }
    }
}

#[cfg(test)]
#[path = "loader_handle_tests.rs"]
mod loader_handle_tests;
