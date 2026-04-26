//! Cdylib fixture whose exported vtable reports `abi_version =
//! u32::MAX`.
//!
//! Built for the wave-3a ABI-diagnostic enrichment tests. The cdylib
//! filename (`libreovim_pkg_wrong_abi_poc.<ext>`) matches the reovim
//! package-naming convention so the loader can recover the package
//! name `wrong-abi-poc` and surface
//! `LoadError::AbiMismatchAtPackage` / `ScanEntryError::AbiMismatchAtPackage`.
//!
//! The trampoline slots are NEVER invoked because validation rejects
//! the vtable on the `abi_version` mismatch before any function pointer
//! is dispatched. They exist only so the static layout matches
//! `ClientRenderVTable`.

#![allow(unsafe_code)]

use {
    reovim_client_subsys_render::{
        abi::{ClientRenderVTable, RenderTargetVTable},
        client_render::ClientRenderDriverProbe,
    },
    reovim_kernel::api::v1::Version,
    std::ffi::{c_char, c_int, c_void},
};

#[unsafe(no_mangle)]
pub static REOVIM_CLIENT_RENDER_DRIVER_VTABLE: ClientRenderVTable = ClientRenderVTable {
    abi_version: u32::MAX,
    api_version: Version::new(1, 0, 0),
    size_of_self: std::mem::size_of::<ClientRenderVTable>(),
    probe: unreachable_probe,
    construct: unreachable_construct,
    target: unreachable_target,
    shutdown: unreachable_shutdown,
    destroy: unreachable_destroy,
    destroy_error_string: unreachable_destroy_error_string,
};

const unsafe extern "C" fn unreachable_probe() -> ClientRenderDriverProbe {
    ClientRenderDriverProbe::new("", "")
}

const unsafe extern "C" fn unreachable_construct(
    _platform: *mut c_void,
    _out_instance: *mut *mut c_void,
    _out_err: *mut *mut c_char,
) -> c_int {
    -1
}

const unsafe extern "C" fn unreachable_target(
    _instance: *mut c_void,
    _out_vtable: *mut *const RenderTargetVTable,
    _out_handle: *mut *mut c_void,
    _out_err: *mut *mut c_char,
) -> c_int {
    -1
}

const unsafe extern "C" fn unreachable_shutdown(
    _instance: *mut c_void,
    _out_err: *mut *mut c_char,
) -> c_int {
    -1
}

const unsafe extern "C" fn unreachable_destroy(_instance: *mut c_void) {}

const unsafe extern "C" fn unreachable_destroy_error_string(_ptr: *mut c_char) {}
