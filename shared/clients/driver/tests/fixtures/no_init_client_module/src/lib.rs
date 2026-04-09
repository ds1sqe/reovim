//! Fixture that manually exports API version + probe + entry, but
//! deliberately does NOT export `reovim_client_module_init`. Loading this
//! `.so` must fail with `LoadError::MissingSymbol { symbol: "reovim_client_module_init", .. }`
//! so T5 can assert the missing-symbol error variant and its message format.
//!
//! This fixture bypasses `declare_client_module!` because the macro always
//! emits init. A manual FFI surface is the cleanest way to simulate a
//! partially-implemented module.

#![allow(unsafe_code)]
#![allow(clippy::missing_const_for_fn, clippy::doc_markdown)]

use std::ffi::c_void;

use reovim_client_driver::{CLIENT_MODULE_API_VERSION, ClientModuleProbe, Version};

#[unsafe(no_mangle)]
pub static REOVIM_CLIENT_MODULE_API_VERSION: Version = CLIENT_MODULE_API_VERSION;

/// Well-formed probe — version check passes.
#[unsafe(no_mangle)]
pub extern "C" fn reovim_client_module_probe() -> ClientModuleProbe {
    ClientModuleProbe::new(
        "no-init",
        "No Init Fixture",
        Version::new(1, 0, 0),
        CLIENT_MODULE_API_VERSION,
    )
}

/// Entry returns a non-null dummy pointer — load_from_path advances to the
/// required-symbol resolution step, where `init` is missing.
#[unsafe(no_mangle)]
pub extern "C" fn reovim_client_module_entry() -> *mut c_void {
    // Allocate one byte so the pointer is non-null but owned by the leaked
    // Box (we never destroy it because init never runs).
    Box::into_raw(Box::new(0u8)).cast::<c_void>()
}

// NO `reovim_client_module_init`, `reovim_client_module_exit`, or
// `reovim_client_module_destroy` exports — load_from_path fails on the first
// missing required symbol it encounters (`init`).
