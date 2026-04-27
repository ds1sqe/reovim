//! Cdylib fixture whose `REOVIM_MODULE_API_VERSION` reports major =
//! `u32::MAX`.
//!
//! Built for the wave-3a ABI-diagnostic enrichment tests. The cdylib
//! filename (`libreovim_pkg_wrong_api_module.<ext>`) matches the reovim
//! package-naming convention so the loader can recover the package
//! name `wrong-api-module` and surface
//! [`reovim_subsys_module_loader::diagnostic::LoadDiagnostic::AbiMismatchAtPackage`]
//! via [`ModuleLoader::from_path_scan_filtered_diag`].
//!
//! The loader checks `REOVIM_MODULE_API_VERSION` before resolving any
//! other FFI symbol, so this fixture omits `reovim_module_probe`,
//! `reovim_module_entry`, etc.; dlopen will succeed but the API
//! compatibility check rejects the module before any other symbol is
//! consulted.

#![allow(unsafe_code)]

use reovim_kernel::api::v1::Version;

#[unsafe(no_mangle)]
pub static REOVIM_MODULE_API_VERSION: Version = Version::new(u32::MAX, 0, 0);
