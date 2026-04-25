//! Runtime-side bridge that resolves `$REOVIM_LIBRARY_ROOT/pkg.lock`
//! and builds a [`LazyRegistry`] for both server and client loaders.
//!
//! Wave-2 shipped the data layer (`pkg-manifest`, `pkg-lockfile`,
//! `pkg-lazyload`) and the `pkg` CLI. Wave-3a wires that data into the
//! runtime loaders so installed cdylibs honor lockfile-declared lazy
//! triggers at startup. This crate owns the I/O step that neither the
//! pure-data `pkg-lazyload` crate nor either subsys tier can host
//! without a layer violation.
//!
//! Both `server/lib/subsys/module-loader/` and
//! `clients/lib/subsys/driver-loader/` consume [`RuntimeLoaderConfig`]
//! to filter the eager-load batch and then dispatch deferred triggers
//! at runtime.

#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(missing_docs)]

mod config;
mod error;
mod package_lookup;
mod registry_load;

pub use {
    crate::{
        config::RuntimeLoaderConfig,
        error::RuntimeLoaderError,
        package_lookup::package_name_for_path,
        registry_load::{load_registry, lockfile_path},
    },
    reovim_dylib_loader::Kind,
    reovim_pkg_lazyload::LazyRegistry,
};
