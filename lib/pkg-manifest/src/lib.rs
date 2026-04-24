//! `pkg.toml` parser and writer for the reovim package manager.
//!
//! This crate defines the manifest types ([`Manifest`], [`Package`],
//! [`Dependency`], [`LazyTrigger`]) and the TOML round-trip that
//! reads and writes them. It is pure mechanism: no resolver, no
//! install, no loader. The resolver (#771 Phase 1) parses the
//! semver-constraint strings carried on [`Package`] and [`Dependency`];
//! install (#771 Phase 2) consumes the dependency graph.

#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(missing_docs)]

mod dependency;
mod lazy;
mod package;
mod parser;
mod writer;

use std::collections::BTreeMap;

pub use crate::{
    dependency::{Dependency, DetailedDep},
    lazy::LazyTrigger,
    package::Package,
};

/// A parsed `pkg.toml` manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// `[package]` table: name and reovim version constraint.
    pub package: Package,
    /// `[dependencies]` table: name → dependency spec.
    ///
    /// Ordered by key so serialization is deterministic.
    pub dependencies: BTreeMap<String, Dependency>,
    /// `[lazy]` table: name → lazy-load trigger.
    ///
    /// Names must appear in [`Manifest::dependencies`]; Phase 1
    /// resolver enforces that invariant. Phase 0 accepts any key.
    pub lazy: BTreeMap<String, LazyTrigger>,
}

/// Error returned by manifest parse and serialize operations.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    /// The TOML source could not be deserialized.
    #[error("manifest parse error: {0}")]
    TomlDe(#[from] toml::de::Error),
    /// The manifest value could not be serialized.
    #[error("manifest serialize error: {0}")]
    TomlSer(#[from] toml::ser::Error),
    /// A `[lazy]` entry did not specify exactly one trigger.
    ///
    /// Exactly one of `on-domain`, `on-event`, `on-capability`, or
    /// `eager = true` must be set per dependency.
    #[error(
        "invalid lazy trigger for `{dep}`: set exactly one of on-domain, on-event, on-capability, or `eager = true`"
    )]
    InvalidLazyTrigger {
        /// Dependency name whose `[lazy.<dep>]` entry was invalid.
        dep: String,
    },
}
