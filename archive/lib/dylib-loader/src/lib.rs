//! Core-tier OS-abstraction wrapper over [`libloading`].
//!
//! `reovim-dylib-loader` is the mechanism beneath the two specialized
//! safe-wrapper subsys crates (`clients/lib/subsys/driver-loader/` and
//! `server/lib/subsys/module-loader/`). It answers three questions for
//! any reovim cdylib consumer:
//!
//! 1. How do I open a shared object at runtime on this OS?
//! 2. Where on disk should I look for reovim cdylibs?
//! 3. How do I scan a set of directories without a single malformed
//!    cdylib aborting the whole discovery pass?
//!
//! The crate has no reovim-* dependencies and no knowledge of any ABI
//! vtable layout; it is pure OS abstraction plus filesystem policy.
//! Vtable validation and hot-reload live in the specialized subsys
//! loaders that sit atop this crate.

#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(missing_docs)]
#![allow(unsafe_code)]

mod error;
mod library;
mod path_resolver;
mod platform;
mod scan;

pub use self::{
    error::{LoaderError, ScanEntryError},
    library::{Library, Symbol},
    path_resolver::{
        EnvProvider, Kind, PathResolver, PathResolverBuilder, StdEnv, UnknownKind, split_paths,
    },
    platform::{
        cdylib_filename, library_extension, library_filename, pkg_name_from_cdylib_filename,
    },
    scan::{ScanEntry, ScanReport, scan_paths},
};
