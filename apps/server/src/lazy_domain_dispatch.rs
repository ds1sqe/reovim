//! `LazyDomainDispatcher` — concrete [`DomainRegisterListener`] impl
//! that observes `Session::set_domain_driver` and dlopens packages
//! whose `pkg.lock` `on-domain = "<name>"` trigger matches.
//!
//! The dispatcher is registered on every newly-created `Session`. Per
//! Wave 3a Decision 2, the listener fires from
//! `SessionState::set_domain_driver` (`session.rs:202`) after the
//! writer-lock release.
//!
//! [`DomainRegisterListener`]: reovim_subsys_session::DomainRegisterListener

use std::{collections::HashSet, path::PathBuf, sync::Arc};

use {
    parking_lot::Mutex,
    reovim_dylib_loader::Kind,
    reovim_pkg_lazyload::{LazyRegistry, TriggerEvent, names_to_load},
    reovim_pkg_manifest::LazyTrigger,
    reovim_subsys_module_loader::loader_handle::LoaderHandle,
    reovim_subsys_session::DomainRegisterListener,
};

/// State for the domain-register lazy dispatcher.
pub struct LazyDomainDispatcher {
    registry: Arc<LazyRegistry>,
    loader: LoaderHandle,
    library_root: PathBuf,
    pending: Mutex<HashSet<String>>,
}

impl LazyDomainDispatcher {
    /// Build a dispatcher.
    ///
    /// Pre-populates the pending-set with every name in `registry`
    /// whose trigger is `on-domain` so a trigger fires at most once
    /// per process lifetime.
    #[must_use]
    pub fn new(
        registry: Arc<LazyRegistry>,
        loader: LoaderHandle,
        library_root: impl Into<PathBuf>,
    ) -> Self {
        let pending: HashSet<String> = registry
            .entries()
            .filter_map(|(name, trigger)| match trigger {
                LazyTrigger::OnDomain(_) => Some(name.to_owned()),
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

impl DomainRegisterListener for LazyDomainDispatcher {
    fn on_register(&self, domain_name: &str) {
        let triggered: Vec<String> =
            names_to_load(&self.registry, &TriggerEvent::Domain(domain_name))
                .into_iter()
                .map(str::to_owned)
                .collect();
        if triggered.is_empty() {
            return;
        }

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
            // and is treated as ABI-compatible. Failures are logged
            // and dropped.
            #[allow(unsafe_code)]
            let load = unsafe {
                self.loader
                    .load_named(&pkg, Kind::Module, &self.library_root)
            };
            if let Err(err) = load {
                tracing::warn!(package = %pkg, ?err, "lazy domain-trigger load failed");
            }
        }
    }
}

#[cfg(test)]
#[path = "lazy_domain_dispatch_tests.rs"]
mod lazy_domain_dispatch_tests;
