#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![cfg_attr(coverage_nightly, allow(unused_features))]
//! Generic dependency resolution via Kahn's topological sort.
//!
//! This crate provides a standalone, zero-dependency topological sort algorithm
//! that works with any key type implementing `Eq + Hash + Clone + Debug`.
//!
//! # Usage
//!
//! ```
//! use reovim_depgraph::{DepEntry, resolve_dependencies};
//!
//! let entries = vec![
//!     DepEntry { key: "a", required: vec![], optional: vec![], provides_caps: vec![], requires_caps: vec![] },
//!     DepEntry { key: "b", required: vec!["a"], optional: vec![], provides_caps: vec![], requires_caps: vec![] },
//!     DepEntry { key: "c", required: vec!["b"], optional: vec![], provides_caps: vec![], requires_caps: vec![] },
//! ];
//!
//! let order = resolve_dependencies(&entries).unwrap();
//! assert_eq!(order.order, vec!["a", "b", "c"]);
//! ```
//!
//! # Design
//!
//! This crate has **zero dependencies** — it uses only `std` collections.
//! It is generic over the key type `K` so it can be used for server modules
//! (`ModuleId`), client extensions (`&str`), or any other dependency graph.

mod graph;
mod version;

pub use {
    graph::{DepEntry, DependencyOrder, DepgraphError, resolve_dependencies},
    version::{
        ConstraintViolation, SemVer, VersionConstraint, VersionRange, check_version_constraints,
    },
};
