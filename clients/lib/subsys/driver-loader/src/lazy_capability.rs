//! `CapabilityLazyHook` — boot-time `on-capability` trigger for
//! Wave 3a client-side lazy loading.
//!
//! Holds the runtime [`LazyRegistry`] and the resolved library root.
//! [`dispatch_capability`](Self::dispatch_capability) is called by
//! the platform runtime once per capability the platform provides;
//! each call probes the cdylib path for any matching package, picks
//! render-vs-debug via header probe, constructs the driver, and
//! parks it in an internal [`DriverStore`].
//!
//! Wave 3a's hook is fire-and-forget: a failed probe or construct is
//! logged at the call site (the platform runtime); the pending-set
//! still drains so subsequent dispatches don't retry. Sub-plan 04
//! enriches the `LoadError` with the package name on ABI mismatch.

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};

use {
    parking_lot::Mutex,
    reovim_dylib_loader::{Kind, cdylib_filename},
    reovim_pkg_lazyload::{LazyRegistry, TriggerEvent, names_to_load},
    reovim_pkg_manifest::LazyTrigger,
};

use crate::{client_debug::LoadedClientDebug, client_render::LoadedClientRender, error::LoadError};

/// Loaded-driver storage keyed by package name.
///
/// Wave 3a does not implement eviction: a driver loaded via lazy
/// dispatch is kept alive for the process lifetime. Wave 3b can
/// extend with reference-counted handles or explicit unload.
#[derive(Default)]
pub struct DriverStore {
    /// Render drivers loaded via lazy dispatch, keyed by package name.
    pub render: HashMap<String, LoadedClientRender>,
    /// Debug drivers loaded via lazy dispatch, keyed by package name.
    pub debug: HashMap<String, LoadedClientDebug>,
}

/// Boot-time `on-capability` trigger dispatcher.
///
/// One instance per platform runtime. The platform calls
/// [`dispatch_capability`](Self::dispatch_capability) once per
/// capability name in its static `PROVIDED_CAPABILITY_NAMES` list;
/// each call dlopens any package whose `pkg.lock` `on-capability`
/// trigger matches.
pub struct CapabilityLazyHook {
    registry: Arc<LazyRegistry>,
    library_root: PathBuf,
    pending: Mutex<HashSet<String>>,
    drivers: Mutex<DriverStore>,
}

impl CapabilityLazyHook {
    /// Build a hook.
    ///
    /// Pre-populates the pending-set with every name in `registry`
    /// whose trigger is `on-capability` so a trigger fires at most
    /// once per process lifetime.
    #[must_use]
    pub fn new(registry: Arc<LazyRegistry>, library_root: impl Into<PathBuf>) -> Self {
        let pending: HashSet<String> = registry
            .entries()
            .filter_map(|(name, trigger)| match trigger {
                LazyTrigger::OnCapability(_) => Some(name.to_owned()),
                _ => None,
            })
            .collect();
        Self {
            registry,
            library_root: library_root.into(),
            pending: Mutex::new(pending),
            drivers: Mutex::new(DriverStore::default()),
        }
    }

    /// Dispatch a single capability name. Loads any package whose
    /// `on-capability` trigger matches `name`.
    ///
    /// Idempotent: a second call with the same `name` is a no-op
    /// because the pending-set drained the matching packages on the
    /// first call. A package whose probe fails for both render and
    /// debug surfaces is reported once via the returned `LoadError`
    /// chain; the pending-set still drains so the platform does not
    /// retry on every subsequent dispatch.
    ///
    /// # Errors
    ///
    /// Returns the LAST `LoadError` encountered while dispatching the
    /// triggered set. Earlier errors in the same dispatch are dropped
    /// (sub-plan 04's diagnostic enrichment will surface package
    /// names so the caller can tell which package failed).
    pub fn dispatch_capability(&self, name: &str) -> Result<(), LoadError> {
        let triggered: Vec<String> = names_to_load(&self.registry, &TriggerEvent::Capability(name))
            .into_iter()
            .map(str::to_owned)
            .collect();
        if triggered.is_empty() {
            return Ok(());
        }

        let to_load: Vec<String> = {
            let mut pending = self.pending.lock();
            triggered
                .into_iter()
                .filter(|pkg| pending.remove(pkg))
                .collect()
        };

        let mut last_err: Option<LoadError> = None;
        for pkg in to_load {
            let path = self
                .library_root
                .join(Kind::Driver.subdir())
                .join(cdylib_filename(&pkg));
            match probe_and_construct(&pkg, &path) {
                Ok(loaded) => {
                    let mut store = self.drivers.lock();
                    match loaded {
                        LoadedDriver::Render(driver) => {
                            store.render.insert(pkg, driver);
                        }
                        LoadedDriver::Debug(driver) => {
                            store.debug.insert(pkg, driver);
                        }
                    }
                }
                Err(err) => {
                    last_err = Some(err);
                }
            }
        }

        last_err.map_or(Ok(()), Err)
    }

    /// Borrow the underlying driver store. Used by the platform
    /// runtime when it needs to keep loaded drivers alive past the
    /// hook's lifetime, or by tests.
    #[must_use]
    pub const fn drivers(&self) -> &Mutex<DriverStore> {
        &self.drivers
    }
}

enum LoadedDriver {
    Render(LoadedClientRender),
    Debug(LoadedClientDebug),
}

fn probe_and_construct(pkg: &str, path: &std::path::Path) -> Result<LoadedDriver, LoadError> {
    if LoadedClientRender::probe_from_path(path).is_ok() {
        return LoadedClientRender::load_from_path(path).map(LoadedDriver::Render);
    }
    if LoadedClientDebug::probe_from_path(path).is_ok() {
        return LoadedClientDebug::load_from_path(path).map(LoadedDriver::Debug);
    }
    Err(LoadError::LibraryOpen(format!(
        "cdylib for package `{pkg}` at {} is neither a render nor a debug driver",
        path.display(),
    )))
}

#[cfg(test)]
#[path = "lazy_capability_tests.rs"]
mod lazy_capability_tests;
