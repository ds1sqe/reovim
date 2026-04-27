//! Fixture that exports `REOVIM_CLIENT_MODULE_API_VERSION` with a MAJOR
//! version one higher than the host provides. Loading this `.so` must fail
//! with `LoadError::IncompatibleApiVersion` so T5 can assert the error
//! variant and its message format.
//!
//! This fixture does NOT use `declare_client_module!` — it only exports the
//! version symbol and a stub probe so `load_from_path` can reach the version
//! check before any other FFI call happens.

#![allow(unsafe_code)]
// Fixtures are pure FFI trampolines — many clippy suggestions do not apply
// (extern "C" functions, doc-test crates, empty `.rs` paragraphs).
#![allow(clippy::missing_const_for_fn, clippy::doc_markdown)]

use std::ffi::c_void;

use reovim_client_driver::{ClientModuleProbe, Version};

/// Hard-coded incompatible API version.
///
/// The host's `CLIENT_MODULE_API_VERSION` lives at
/// `clients/lib/driver/src/types/mod.rs` — this fixture deliberately
/// advertises a major version 99 higher so the version check rejects it
/// regardless of future host bumps.
#[unsafe(no_mangle)]
pub static REOVIM_CLIENT_MODULE_API_VERSION: Version = Version::new(99, 0, 0);

/// Stub probe — never actually called because version check fails first.
#[unsafe(no_mangle)]
pub extern "C" fn reovim_client_module_probe() -> ClientModuleProbe {
    ClientModuleProbe::new(
        "incompatible",
        "Incompatible",
        Version::new(1, 0, 0),
        Version::new(99, 0, 0),
    )
}

/// Stub entry — never called, but present so load_from_path gets past the
/// probe/entry symbol lookups if the version check is ever relaxed.
#[unsafe(no_mangle)]
pub extern "C" fn reovim_client_module_entry() -> *mut c_void {
    std::ptr::null_mut()
}
