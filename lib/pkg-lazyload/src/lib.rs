//! Lazy-load registry, scan filter, and runtime trigger dispatcher
//! for reovim cdylib packages.
//!
//! A reovim package graph mixes packages that must be loaded at
//! startup (`eager`) with packages that should only be loaded when a
//! specific domain opens, event fires, or capability is requested.
//! [`LazyRegistry`] is the immutable name→trigger map built from a
//! [`reovim_pkg_lockfile::Lockfile`]; later phases add a scan filter
//! and a runtime trigger dispatcher that consult it.

#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(missing_docs)]

mod error;
mod filter;
mod registry;
mod trigger;

pub use crate::{
    error::LazyError,
    filter::scan_eager,
    registry::LazyRegistry,
    trigger::{TriggerEvent, load_triggered, names_to_load},
};
