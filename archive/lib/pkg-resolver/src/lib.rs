//! Semver graph resolution for the reovim package manager.
//!
//! Given a root `pkg.toml` (a [`reovim_pkg_manifest::Manifest`]) and a
//! tree of local-path dependencies, [`resolve`] walks the DAG, validates
//! every declared version constraint, detects cycles and conflicts, and
//! returns a deterministic [`Resolved`] package list. The helper
//! [`Resolved::into_lockfile`] converts the result into a
//! [`reovim_pkg_lockfile::Lockfile`] ready for `pkg.lock` emission.
//!
//! Phase 1 scope: local-path deps only. A dependency without a `path`
//! field fails with [`ResolveError::UnresolvableDependency`]; the remote
//! registry lands in a follow-on epic.

#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(missing_docs)]

mod constraint;
mod emit;
mod graph;
mod loader;

pub use crate::graph::{ResolveError, Resolved, ResolvedPackage, resolve};
