//! `LazyCommandDispatcher` — concrete [`CommandResolutionListener`]
//! impl that observes command-name resolution and dlopens packages
//! whose `pkg.lock` `on-event = "<name>"` trigger matches.
//!
//! The dispatcher is registered on the server's [`CommandNameIndex`]
//! at bootstrap. Per Wave 3a Decision 1, command-name dispatch is the
//! `on-event` trigger surface — there is no kernel-event subscription.
//!
//! [`CommandResolutionListener`]: reovim_subsys_command::CommandResolutionListener
//! [`CommandNameIndex`]: reovim_subsys_command::CommandNameIndex

use std::{collections::HashSet, path::PathBuf, sync::Arc};

use {
    parking_lot::Mutex,
    reovim_dylib_loader::Kind,
    reovim_pkg_lazyload::{LazyRegistry, TriggerEvent, names_to_load},
    reovim_pkg_manifest::LazyTrigger,
    reovim_subsys_command::CommandResolutionListener,
    reovim_subsys_module_loader::loader_handle::LoaderHandle,
};

/// State for the command-name lazy dispatcher.
///
/// Wraps the immutable `LazyRegistry`, the shared `LoaderHandle`, and
/// the `library_root`. The mutable pending-set tracks names already
/// scheduled so the same trigger does not double-load.
pub struct LazyCommandDispatcher {
    registry: Arc<LazyRegistry>,
    loader: LoaderHandle,
    library_root: PathBuf,
    pending: Mutex<HashSet<String>>,
}

impl LazyCommandDispatcher {
    /// Build a dispatcher.
    ///
    /// Pre-populates the pending-set with every name in `registry`
    /// whose trigger is `on-event` so a trigger fires at most once per
    /// process lifetime.
    #[must_use]
    pub fn new(
        registry: Arc<LazyRegistry>,
        loader: LoaderHandle,
        library_root: impl Into<PathBuf>,
    ) -> Self {
        let pending: HashSet<String> = registry
            .entries()
            .filter_map(|(name, trigger)| match trigger {
                LazyTrigger::OnEvent(_) => Some(name.to_owned()),
                _ => None,
            })
            .collect();
        Self {
            registry,
            loader,
            library_root: library_root.into(),
            pending: Mutex::new(pending),
        }
    }
}

impl CommandResolutionListener for LazyCommandDispatcher {
    fn on_resolve(&self, name: &str) {
        let triggered: Vec<String> = names_to_load(&self.registry, &TriggerEvent::Event(name))
            .into_iter()
            .map(str::to_owned)
            .collect();
        if triggered.is_empty() {
            return;
        }

        // Drain only those still pending; emptying a name from the
        // pending-set is what makes the trigger fire-once.
        let to_load: Vec<String> = {
            let mut pending = self.pending.lock();
            triggered
                .into_iter()
                .filter(|pkg| pending.remove(pkg))
                .collect()
        };

        for pkg in to_load {
            // SAFETY: bootstrap-side trust — every cdylib under
            // `<library_root>/modules/` was placed by `pkg install`
            // and is treated as ABI-compatible. A failed load is
            // logged and dropped (fire-and-forget).
            #[allow(unsafe_code)]
            let load = unsafe {
                self.loader
                    .load_named(&pkg, Kind::Module, &self.library_root)
            };
            if let Err(err) = load {
                tracing::warn!(package = %pkg, ?err, "lazy command-trigger load failed");
            }
        }
    }
}

#[cfg(test)]
#[path = "lazy_command_dispatch_tests.rs"]
mod lazy_command_dispatch_tests;
