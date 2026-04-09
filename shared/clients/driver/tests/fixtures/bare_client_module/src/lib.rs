//! Fixture that manually exports ONLY the 6 hard-required FFI symbols
//! (`REOVIM_CLIENT_MODULE_API_VERSION`, `probe`, `entry`, `init`, `exit`,
//! `destroy`). Every optional symbol is absent, so `load_from_path`
//! succeeds but the resulting `ClientModuleHandle` must return documented
//! defaults for all 17 optional dispatch methods.
//!
//! This fixture covers T1's "optional symbols missing graceful degradation"
//! contract without requiring 17 separate fixtures.

#![allow(unsafe_code)]
#![allow(clippy::missing_const_for_fn, clippy::doc_markdown)]

use std::ffi::c_void;

use reovim_client_driver::{CLIENT_MODULE_API_VERSION, ClientModuleProbe, Version};

/// Module state — nothing interesting, just a non-zero allocation so the
/// entry pointer is non-null.
struct BareModule;

#[unsafe(no_mangle)]
pub static REOVIM_CLIENT_MODULE_API_VERSION: Version = CLIENT_MODULE_API_VERSION;

#[unsafe(no_mangle)]
pub extern "C" fn reovim_client_module_probe() -> ClientModuleProbe {
    ClientModuleProbe::new(
        "bare-client",
        "Bare Client Fixture",
        Version::new(1, 0, 0),
        CLIENT_MODULE_API_VERSION,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn reovim_client_module_entry() -> *mut c_void {
    let boxed = Box::new(BareModule);
    Box::into_raw(boxed).cast::<c_void>()
}

/// Minimal init — returns Success (0). Signature matches
/// `ClientInitFn = unsafe extern "C" fn(*mut c_void, *const c_void) -> i32`.
#[unsafe(no_mangle)]
pub extern "C" fn reovim_client_module_init(
    _module: *mut c_void,
    _ctx: *const c_void,
) -> i32 {
    0
}

/// Minimal exit — returns Ok (0). Signature matches
/// `ClientExitFn = unsafe extern "C" fn(*mut c_void) -> i32`.
#[unsafe(no_mangle)]
pub extern "C" fn reovim_client_module_exit(_module: *mut c_void) -> i32 {
    0
}

/// Destroy — drops the boxed state.
#[unsafe(no_mangle)]
pub extern "C" fn reovim_client_module_destroy(module: *mut c_void) {
    if !module.is_null() {
        // SAFETY: module was allocated by entry() as a Box<BareModule>.
        unsafe {
            let _ = Box::from_raw(module.cast::<BareModule>());
        }
    }
}

// NO optional symbols — none of the event, role-declaration, chrome
// metadata, or priority trampolines are exported. The handle must return
// documented defaults for all of them.
