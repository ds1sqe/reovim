//! Generic dependency resolution via Kahn's topological sort.
//!
//! This crate provides a standalone, zero-dependency topological sort algorithm
//! that works with any key type implementing `Eq + Hash + Clone + Debug`.
//!
//! # Usage
//!
//! ```
//! use reovim_driver_depgraph::{DepEntry, resolve_dependencies};
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

pub use graph::{DepEntry, DependencyOrder, DepgraphError, resolve_dependencies};
