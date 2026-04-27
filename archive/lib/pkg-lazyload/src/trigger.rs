//! Runtime trigger dispatcher.
//!
//! A reovim runtime calls [`names_to_load`] when a domain opens, an
//! event fires, or a capability is requested, to discover which lazy
//! packages should now be loaded. [`load_triggered`] is the convenience
//! that also opens those cdylibs from the standard `library_root /
//! kind.subdir() / cdylib_filename(name)` layout.

use std::path::Path;

use {
    reovim_dylib_loader::{Kind, Library, LoaderError, cdylib_filename},
    reovim_pkg_manifest::LazyTrigger,
};

use crate::registry::LazyRegistry;

/// A runtime event that may unlock one or more lazy packages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerEvent<'a> {
    /// A domain with this name was opened.
    Domain(&'a str),
    /// An event with this name fired.
    Event(&'a str),
    /// A capability with this name was requested.
    Capability(&'a str),
}

/// Names of registry packages whose trigger matches `event`.
#[must_use]
pub fn names_to_load<'r>(registry: &'r LazyRegistry, event: &TriggerEvent<'_>) -> Vec<&'r str> {
    registry
        .entries()
        .filter(|(_, trigger)| matches(trigger, event))
        .map(|(name, _)| name)
        .collect()
}

fn matches(trigger: &LazyTrigger, event: &TriggerEvent<'_>) -> bool {
    match (trigger, event) {
        (LazyTrigger::OnDomain(s), TriggerEvent::Domain(arg))
        | (LazyTrigger::OnEvent(s), TriggerEvent::Event(arg))
        | (LazyTrigger::OnCapability(s), TriggerEvent::Capability(arg)) => s == arg,
        _ => false,
    }
}

/// Open every cdylib whose package matches `event`.
///
/// Returns one entry per triggered name, pairing the package name with
/// the result of opening the cdylib at
/// `library_root / kind.subdir() / cdylib_filename(name)`.
#[must_use]
pub fn load_triggered(
    library_root: &Path,
    kind: Kind,
    registry: &LazyRegistry,
    event: &TriggerEvent<'_>,
) -> Vec<(String, Result<Library, LoaderError>)> {
    names_to_load(registry, event)
        .into_iter()
        .map(|name| {
            let path = library_root.join(kind.subdir()).join(cdylib_filename(name));
            (name.to_string(), Library::open(&path))
        })
        .collect()
}

#[cfg(test)]
#[path = "trigger_tests.rs"]
mod trigger_tests;
