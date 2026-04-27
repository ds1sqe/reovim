//! Immutable package-name → [`LazyTrigger`] map built from a
//! [`Lockfile`].

use std::collections::BTreeMap;

use {
    reovim_pkg_lockfile::Lockfile,
    reovim_pkg_manifest::{LazyTrigger, parse_trigger},
};

use crate::error::LazyError;

/// Registry of lazy-load triggers keyed by package name.
///
/// Built once from a [`Lockfile`] at startup. A package without a
/// `trigger` field defaults to [`LazyTrigger::Eager`], matching the
/// backward-compatible behaviour for Phase 2 lockfiles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LazyRegistry {
    by_name: BTreeMap<String, LazyTrigger>,
}

impl LazyRegistry {
    /// An empty registry — every package falls through as
    /// [`LazyTrigger::Eager`].
    ///
    /// Used by the runtime loader on hosts that have not run
    /// `pkg install` yet: with no `pkg.lock` to read, every cdylib in
    /// the library root is treated as eager, preserving the
    /// pre-package-manager dlopen-everything behavior.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            by_name: BTreeMap::new(),
        }
    }

    /// Parse every package's `trigger` field into a [`LazyTrigger`].
    ///
    /// A missing `trigger` defaults to [`LazyTrigger::Eager`].
    ///
    /// # Errors
    ///
    /// Returns [`LazyError::MalformedTrigger`] if any package's
    /// `trigger` string does not match the
    /// `on-domain:<n>` / `on-event:<n>` / `on-capability:<n>` /
    /// `eager` grammar.
    pub fn from_lockfile(lock: &Lockfile) -> Result<Self, LazyError> {
        let mut by_name = BTreeMap::new();
        for pkg in &lock.packages {
            let trigger = match &pkg.trigger {
                None => LazyTrigger::Eager,
                Some(raw) => parse_trigger(raw).map_err(|source| LazyError::MalformedTrigger {
                    pkg: pkg.name.clone(),
                    value: raw.clone(),
                    source: Box::new(source),
                })?,
            };
            by_name.insert(pkg.name.clone(), trigger);
        }
        Ok(Self { by_name })
    }

    /// Return the trigger for `name`, or [`LazyTrigger::Eager`] when
    /// the package is not in the registry.
    #[must_use]
    pub fn trigger_for(&self, name: &str) -> LazyTrigger {
        self.by_name
            .get(name)
            .cloned()
            .unwrap_or(LazyTrigger::Eager)
    }

    /// Report whether `name` has a non-eager trigger.
    #[must_use]
    pub fn is_lazy(&self, name: &str) -> bool {
        !matches!(self.trigger_for(name), LazyTrigger::Eager)
    }

    /// Iterator over `(name, trigger)` pairs in registered order.
    ///
    /// Used by the trigger dispatcher to filter packages by event
    /// without exposing the underlying map.
    pub fn entries(&self) -> impl Iterator<Item = (&str, &LazyTrigger)> {
        self.by_name.iter().map(|(n, t)| (n.as_str(), t))
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod registry_tests;
